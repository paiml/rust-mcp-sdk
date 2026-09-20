//! `tasks/update` input delivery through `GenericTaskStore<B>`.
//!
//! Everything here drives `GenericTaskStore<B>` rather than a specific backend,
//! because that type is where the whole domain implementation lives: the
//! in-memory, `DynamoDB` and Redis deployments share it byte for byte and the
//! backends beneath it are dumb key-value stores. A property proven here is a
//! property of all three. Per the project's no-Docker-in-tests rule, live
//! servers are out of scope; where a *backend behaviour* has to be exercised
//! (eventual consistency, a genuine two-writer race, write volume) a
//! purpose-built double supplies it.
//!
//! # What each group proves
//!
//! | Group | Property |
//! |-------|----------|
//! | Delivery semantics | complete vs. partial, ignore-not-error, terminal refusal |
//! | Concurrency | conflict propagates; and, separately, first writer WINS |
//! | Isolation | another owner reads `NotFound`, and the message says nothing else |
//! | Durability | a record written before this feature existed still reads |
//! | Capacity | a maximum-sized delivery still fits a `DynamoDB` item |
//! | Delegation | all three of the sites that must implement this are covered |

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{json, Map, Value};

use pmcp_tasks::domain::TaskRecord;
use pmcp_tasks::error::TaskError;
use pmcp_tasks::security::TaskSecurityConfig;
use pmcp_tasks::store::backend::{make_key, StorageBackend, StorageError, VersionedRecord};
use pmcp_tasks::store::generic::GenericTaskStore;
use pmcp_tasks::store::memory::{InMemoryBackend, InMemoryTaskStore};
use pmcp_tasks::store::TaskStore;
use pmcp_tasks::types::task::TaskStatus;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const OWNER: &str = "owner-1";

/// A store over a plain in-memory backend, with anonymous access left at its
/// default (`false`) so the tests exercise the shipped configuration.
fn store() -> GenericTaskStore<InMemoryBackend> {
    GenericTaskStore::new(InMemoryBackend::new())
}

/// SERVER-authored input requests. The shape is opaque to this crate -- it sits
/// below the `serde_json::Value` seam and never interprets the request bodies --
/// so any object keyed by the server-assigned key is a faithful fixture.
fn requests_of(keys: &[&str]) -> Value {
    Value::Object(
        keys.iter()
            .map(|key| {
                (
                    (*key).to_string(),
                    json!({ "kind": "elicitation", "prompt": format!("need {key}") }),
                )
            })
            .collect::<Map<String, Value>>(),
    )
}

/// A client's answers, keyed identically to [`requests_of`].
fn responses_of(keys: &[&str]) -> Value {
    Value::Object(
        keys.iter()
            .map(|key| ((*key).to_string(), json!({ "answer": key })))
            .collect::<Map<String, Value>>(),
    )
}

/// Creates a task and pauses it awaiting `keys`, returning the task id.
async fn paused_task<B: StorageBackend>(
    store: &GenericTaskStore<B>,
    owner: &str,
    keys: &[&str],
) -> String {
    let record = store
        .create(owner, "tools/call", None)
        .await
        .expect("create should succeed");
    let task_id = record.task.task_id.clone();
    store
        .record_input_requests(&task_id, owner, requests_of(keys))
        .await
        .expect("record_input_requests should succeed");
    task_id
}

fn accepted(outcome: &Value) -> Vec<String> {
    string_list(outcome, "accepted")
}

fn ignored(outcome: &Value) -> Vec<String> {
    string_list(outcome, "ignored")
}

fn string_list(outcome: &Value, field: &str) -> Vec<String> {
    outcome[field]
        .as_array()
        .unwrap_or_else(|| panic!("{field} should be an array, got: {outcome}"))
        .iter()
        .map(|value| value.as_str().unwrap_or_default().to_string())
        .collect()
}

fn is_complete(outcome: &Value) -> bool {
    outcome["complete"]
        .as_bool()
        .unwrap_or_else(|| panic!("complete should be a bool, got: {outcome}"))
}

// ---------------------------------------------------------------------------
// 1-2. Complete vs. partial delivery
// ---------------------------------------------------------------------------

/// A delivery that answers everything outstanding persists the answers AND
/// resumes the task, and does both in a SINGLE write -- asserted by the version
/// advancing by exactly one, which is the observable signature of one
/// `put_if_version`.
#[tokio::test]
async fn deliver_inputs_completing_the_set_transitions_to_working() {
    let store = store();
    let task_id = paused_task(&store, OWNER, &["city"]).await;

    let before = store.get(&task_id, OWNER).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    let outcome = store
        .deliver_inputs(&task_id, OWNER, responses_of(&["city"]))
        .await
        .expect("delivery should be accepted");

    assert_eq!(accepted(&outcome), vec!["city".to_string()]);
    assert!(ignored(&outcome).is_empty(), "nothing should be ignored");
    assert!(is_complete(&outcome), "the outstanding set is now complete");

    let after = store.get(&task_id, OWNER).await.unwrap();
    assert_eq!(after.task.status, TaskStatus::Working, "task must resume");
    assert_eq!(
        after
            .input_responses
            .as_ref()
            .and_then(|answers| answers.get("city")),
        Some(&json!({ "answer": "city" })),
        "the answer must be persisted, not merely acknowledged"
    );
    assert!(
        after.task.last_updated_at > before.task.last_updated_at,
        "last_updated_at must advance: {} -> {}",
        before.task.last_updated_at,
        after.task.last_updated_at
    );
    assert_eq!(
        after.version,
        before.version + 1,
        "persist + transition must be ONE write, not two"
    );
}

/// A partial delivery persists what it carried and the task STAYS paused.
///
/// Both halves are asserted deliberately: a test that checked only the status
/// would pass against an implementation that dropped the responses on the
/// floor, and a test that checked only the responses would pass against one
/// that resumed a task with answers still outstanding.
#[tokio::test]
async fn deliver_inputs_partial_set_stays_input_required() {
    let store = store();
    let task_id = paused_task(&store, OWNER, &["city", "country"]).await;

    let outcome = store
        .deliver_inputs(&task_id, OWNER, responses_of(&["city"]))
        .await
        .unwrap();

    assert_eq!(accepted(&outcome), vec!["city".to_string()]);
    assert!(!is_complete(&outcome), "one key is still outstanding");

    let after = store.get(&task_id, OWNER).await.unwrap();
    assert_eq!(
        after.task.status,
        TaskStatus::InputRequired,
        "a partial delivery must NOT resume the task"
    );
    let answers = after.input_responses.as_ref().expect("answers persisted");
    assert!(answers.contains_key("city"), "the answer must be durable");
    assert!(!answers.contains_key("country"), "nothing was invented");

    // The remaining key completes it.
    let second = store
        .deliver_inputs(&task_id, OWNER, responses_of(&["country"]))
        .await
        .unwrap();
    assert!(is_complete(&second));
    assert_eq!(
        store.get(&task_id, OWNER).await.unwrap().task.status,
        TaskStatus::Working
    );
}

// ---------------------------------------------------------------------------
// 3-4. Ignore semantics
// ---------------------------------------------------------------------------

/// A key the server never issued is IGNORED and REPORTED -- not an error.
///
/// Reporting matters as much as ignoring: silently swallowing an unknown key
/// would leave the caller unable to distinguish "accepted" from "discarded".
#[tokio::test]
async fn deliver_inputs_ignores_keys_that_are_not_outstanding() {
    let store = store();
    let task_id = paused_task(&store, OWNER, &["city"]).await;

    let outcome = store
        .deliver_inputs(&task_id, OWNER, responses_of(&["city", "never-issued"]))
        .await
        .expect("an unknown key must not fail the whole delivery");

    assert_eq!(accepted(&outcome), vec!["city".to_string()]);
    assert_eq!(ignored(&outcome), vec!["never-issued".to_string()]);

    let after = store.get(&task_id, OWNER).await.unwrap();
    let answers = after.input_responses.as_ref().unwrap();
    assert!(
        !answers.contains_key("never-issued"),
        "an ignored key must not be persisted"
    );
}

/// An already-answered key is IGNORED rather than re-accepted, so a delivered
/// response can never be replayed over.
#[tokio::test]
async fn deliver_inputs_ignores_an_already_answered_key() {
    let store = store();
    let task_id = paused_task(&store, OWNER, &["city", "country"]).await;

    store
        .deliver_inputs(&task_id, OWNER, responses_of(&["city"]))
        .await
        .unwrap();

    let replay = store
        .deliver_inputs(&task_id, OWNER, json!({ "city": { "answer": "tampered" } }))
        .await
        .expect("a replay must not fail the delivery");

    assert!(accepted(&replay).is_empty(), "nothing may be re-accepted");
    assert_eq!(ignored(&replay), vec!["city".to_string()]);

    let after = store.get(&task_id, OWNER).await.unwrap();
    assert_eq!(
        after.input_responses.as_ref().unwrap().get("city"),
        Some(&json!({ "answer": "city" })),
        "the ORIGINAL answer must survive the replay"
    );
    assert_eq!(
        after.task.status,
        TaskStatus::InputRequired,
        "a delivery that changed nothing must not resume the task"
    );
}

// ---------------------------------------------------------------------------
// 5. Terminal tasks cannot be fed
// ---------------------------------------------------------------------------

async fn assert_terminal_delivery_is_refused(terminal: TaskStatus) {
    let store = store();
    let task_id = paused_task(&store, OWNER, &["city"]).await;
    store
        .update_status(&task_id, OWNER, terminal, None)
        .await
        .unwrap();

    let result = store
        .deliver_inputs(&task_id, OWNER, responses_of(&["city"]))
        .await;

    assert!(
        matches!(result, Err(TaskError::InvalidTransition { .. })),
        "a {terminal} task must not be feedable, got: {result:?}"
    );
    let after = store.get(&task_id, OWNER).await.unwrap();
    assert_eq!(after.task.status, terminal, "status must be untouched");
    assert!(
        after.input_responses.is_none(),
        "a refused delivery must persist nothing"
    );
}

#[tokio::test]
async fn deliver_inputs_on_a_completed_task_is_refused() {
    assert_terminal_delivery_is_refused(TaskStatus::Completed).await;
}

#[tokio::test]
async fn deliver_inputs_on_a_failed_task_is_refused() {
    assert_terminal_delivery_is_refused(TaskStatus::Failed).await;
}

#[tokio::test]
async fn deliver_inputs_on_a_cancelled_task_is_refused() {
    assert_terminal_delivery_is_refused(TaskStatus::Cancelled).await;
}

/// A task that is merely `working` is not awaiting input either. This is the
/// non-vacuity partner of the three terminal tests: it shows the refusal is
/// "not `input_required`", not "terminal".
#[tokio::test]
async fn deliver_inputs_on_a_working_task_is_refused() {
    let store = store();
    let record = store.create(OWNER, "tools/call", None).await.unwrap();

    let result = store
        .deliver_inputs(&record.task.task_id, OWNER, responses_of(&["city"]))
        .await;

    assert!(
        matches!(result, Err(TaskError::InvalidTransition { .. })),
        "a working task is not awaiting input, got: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// 6. Concurrency
// ---------------------------------------------------------------------------

/// A backend whose `put_if_version` fails once armed.
///
/// The plan named `generic.rs`'s in-file `CasConflictBackend`, but that double
/// lives inside a `#[cfg(test)] mod` and is therefore unreachable from an
/// integration test, which compiles as a separate crate. This is a behaviourally
/// identical local copy, with an arming switch so the fixture can be built
/// before the conflicts start.
#[derive(Debug)]
struct ArmedConflictBackend {
    inner: InMemoryBackend,
    armed: AtomicBool,
}

impl ArmedConflictBackend {
    fn new() -> Self {
        Self {
            inner: InMemoryBackend::new(),
            armed: AtomicBool::new(false),
        }
    }

    fn arm(&self) {
        self.armed.store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl StorageBackend for ArmedConflictBackend {
    async fn get(&self, key: &str) -> Result<VersionedRecord, StorageError> {
        self.inner.get(key).await
    }
    async fn put(&self, key: &str, data: &[u8]) -> Result<u64, StorageError> {
        self.inner.put(key, data).await
    }
    async fn put_if_version(
        &self,
        key: &str,
        data: &[u8],
        expected_version: u64,
    ) -> Result<u64, StorageError> {
        if self.armed.load(Ordering::SeqCst) {
            return Err(StorageError::VersionConflict {
                key: key.to_string(),
                expected: expected_version,
                actual: expected_version + 1,
            });
        }
        self.inner.put_if_version(key, data, expected_version).await
    }
    async fn delete(&self, key: &str) -> Result<bool, StorageError> {
        self.inner.delete(key).await
    }
    async fn list_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, VersionedRecord)>, StorageError> {
        self.inner.list_by_prefix(prefix).await
    }
    async fn cleanup_expired(&self) -> Result<usize, StorageError> {
        self.inner.cleanup_expired().await
    }
}

/// A lost update is impossible: when the record moved under the delivery, the
/// CAS refuses and the caller is told, rather than the delivery overwriting
/// whatever landed in between.
///
/// This double proves conflict PROPAGATION only -- it never lets a writer win.
/// `two_writers_first_writer_wins` below supplies the other half.
#[tokio::test]
async fn concurrent_deliver_inputs_first_writer_wins() {
    let store = GenericTaskStore::new(ArmedConflictBackend::new());
    let task_id = paused_task(&store, OWNER, &["city"]).await;
    let before = store.get(&task_id, OWNER).await.unwrap();

    store.backend().arm();

    let result = store
        .deliver_inputs(&task_id, OWNER, responses_of(&["city"]))
        .await;

    assert!(
        matches!(result, Err(TaskError::ConcurrentModification { .. })),
        "a version conflict must propagate, got: {result:?}"
    );

    let after = store.get(&task_id, OWNER).await.unwrap();
    assert_eq!(
        after.version, before.version,
        "the record must not have been clobbered"
    );
    assert!(
        after.input_responses.is_none(),
        "a conflicting delivery must persist nothing"
    );
    assert_eq!(after.task.status, TaskStatus::InputRequired);
}

/// A backend that holds every armed `put_if_version` at a barrier, so two
/// writers are guaranteed to have READ the same version before either WRITES.
///
/// This is what an always-conflicting double cannot do: it proves that of two
/// genuine racers exactly one lands and the other is refused, rather than that
/// conflicts are reported.
#[derive(Debug)]
struct BarrierBackend {
    inner: InMemoryBackend,
    barrier: tokio::sync::Barrier,
    armed: AtomicBool,
}

impl BarrierBackend {
    fn new(writers: usize) -> Self {
        Self {
            inner: InMemoryBackend::new(),
            barrier: tokio::sync::Barrier::new(writers),
            armed: AtomicBool::new(false),
        }
    }

    fn arm(&self) {
        self.armed.store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl StorageBackend for BarrierBackend {
    async fn get(&self, key: &str) -> Result<VersionedRecord, StorageError> {
        self.inner.get(key).await
    }
    async fn put(&self, key: &str, data: &[u8]) -> Result<u64, StorageError> {
        self.inner.put(key, data).await
    }
    async fn put_if_version(
        &self,
        key: &str,
        data: &[u8],
        expected_version: u64,
    ) -> Result<u64, StorageError> {
        if self.armed.load(Ordering::SeqCst) {
            self.barrier.wait().await;
        }
        self.inner.put_if_version(key, data, expected_version).await
    }
    async fn delete(&self, key: &str) -> Result<bool, StorageError> {
        self.inner.delete(key).await
    }
    async fn list_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, VersionedRecord)>, StorageError> {
        self.inner.list_by_prefix(prefix).await
    }
    async fn cleanup_expired(&self) -> Result<usize, StorageError> {
        self.inner.cleanup_expired().await
    }
}

#[tokio::test]
async fn two_writers_first_writer_wins() {
    let store = Arc::new(GenericTaskStore::new(BarrierBackend::new(2)));
    let task_id = paused_task(store.as_ref(), OWNER, &["city", "country"]).await;
    let before = store.get(&task_id, OWNER).await.unwrap();

    store.backend().arm();

    let writer_a = tokio::spawn({
        let store = Arc::clone(&store);
        let task_id = task_id.clone();
        async move {
            store
                .deliver_inputs(&task_id, OWNER, responses_of(&["city"]))
                .await
        }
    });
    let writer_b = tokio::spawn({
        let store = Arc::clone(&store);
        let task_id = task_id.clone();
        async move {
            store
                .deliver_inputs(&task_id, OWNER, responses_of(&["country"]))
                .await
        }
    });

    let results = [writer_a.await.unwrap(), writer_b.await.unwrap()];
    let winners = results.iter().filter(|r| r.is_ok()).count();
    let refused = results
        .iter()
        .filter(|r| matches!(r, Err(TaskError::ConcurrentModification { .. })))
        .count();

    assert_eq!(winners, 1, "exactly one writer must land: {results:?}");
    assert_eq!(refused, 1, "the loser must be TOLD, not silently dropped");

    let after = store.get(&task_id, OWNER).await.unwrap();
    assert_eq!(
        after.version,
        before.version + 1,
        "exactly one write may have landed"
    );
    let answers = after
        .input_responses
        .as_ref()
        .expect("the winner persisted");
    assert_eq!(
        answers.len(),
        1,
        "the landed record must be the winner's, intact and unmixed: {answers:?}"
    );
    assert_eq!(after.task.status, TaskStatus::InputRequired);
}

// ---------------------------------------------------------------------------
// 7. Owner isolation
// ---------------------------------------------------------------------------

/// Another owner's task is structurally unreachable (a different owner produces
/// a different key) and reports `NotFound`. The rendered message must name
/// neither the concept of ownership nor the other owner's identity, or the
/// refusal itself would disclose what it is refusing to disclose.
#[tokio::test]
async fn deliver_inputs_for_another_owner_is_not_found() {
    let store = store();
    let task_id = paused_task(&store, "owner-a", &["city"]).await;

    let result = store
        .deliver_inputs(&task_id, "owner-b", responses_of(&["city"]))
        .await;

    let error = match result {
        Err(error @ TaskError::NotFound { .. }) => error,
        other => panic!("expected NotFound, got: {other:?}"),
    };
    let rendered = error.to_string();
    assert!(
        !rendered.contains("owner"),
        "the message must not mention ownership: {rendered}"
    );
    assert!(
        !rendered.contains("owner-a"),
        "the message must not name the other owner: {rendered}"
    );

    let untouched = store.get(&task_id, "owner-a").await.unwrap();
    assert!(untouched.input_responses.is_none());
}

/// The defence-in-depth branch: a record whose stored `ownerId` disagrees with
/// the key it was found under is also `NotFound`.
///
/// The owner-prefixed key normally makes this unreachable, so the record is
/// planted directly in the backend to reach the branch at all.
#[tokio::test]
async fn deliver_inputs_for_a_record_whose_owner_disagrees_is_not_found() {
    let store = store();
    let task_id = paused_task(&store, "owner-a", &["city"]).await;

    // Copy owner-a's bytes verbatim under owner-b's key.
    let planted = store
        .backend()
        .get(&make_key("owner-a", &task_id))
        .await
        .unwrap();
    store
        .backend()
        .put(&make_key("owner-b", &task_id), &planted.data)
        .await
        .unwrap();

    let result = store
        .deliver_inputs(&task_id, "owner-b", responses_of(&["city"]))
        .await;

    assert!(
        matches!(result, Err(TaskError::NotFound { .. })),
        "a record whose ownerId disagrees must be NotFound, got: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// 8. Backend contract: eventual consistency
// ---------------------------------------------------------------------------

/// A backend that serves each key's PREVIOUS value once after a write before
/// converging -- the read-your-own-write hazard a globally replicated store
/// exhibits when the read is not explicitly strongly consistent.
/// The value a key held BEFORE its latest write. `None` means the key did not
/// exist yet, which a replica that has not received the write reports as
/// `NotFound` -- not as the converged value.
type PreviousValue = Option<(Vec<u8>, u64)>;

#[derive(Debug)]
struct EventuallyConsistentBackend {
    inner: InMemoryBackend,
    stale: Mutex<HashMap<String, PreviousValue>>,
    gets: AtomicUsize,
}

impl EventuallyConsistentBackend {
    fn new() -> Self {
        Self {
            inner: InMemoryBackend::new(),
            stale: Mutex::new(HashMap::new()),
            gets: AtomicUsize::new(0),
        }
    }

    fn get_calls(&self) -> usize {
        self.gets.load(Ordering::SeqCst)
    }

    async fn remember_previous(&self, key: &str) {
        let previous = self
            .inner
            .get(key)
            .await
            .ok()
            .map(|record| (record.data, record.version));
        self.stale
            .lock()
            .expect("stale map poisoned")
            .insert(key.to_string(), previous);
    }
}

#[async_trait]
impl StorageBackend for EventuallyConsistentBackend {
    async fn get(&self, key: &str) -> Result<VersionedRecord, StorageError> {
        self.gets.fetch_add(1, Ordering::SeqCst);
        // Three distinct cases, and collapsing the middle one is exactly how a
        // double stops being faithful: `Some(None)` means the key had NO value
        // before the latest write, so a replica that has not received that write
        // answers NOT FOUND -- it must not fall through to the converged value.
        let stale = self.stale.lock().expect("stale map poisoned").remove(key);
        match stale {
            Some(Some((data, version))) => Ok(VersionedRecord { data, version }),
            Some(None) => Err(StorageError::NotFound {
                key: key.to_string(),
            }),
            None => self.inner.get(key).await,
        }
    }
    async fn put(&self, key: &str, data: &[u8]) -> Result<u64, StorageError> {
        self.remember_previous(key).await;
        self.inner.put(key, data).await
    }
    async fn put_if_version(
        &self,
        key: &str,
        data: &[u8],
        expected_version: u64,
    ) -> Result<u64, StorageError> {
        self.remember_previous(key).await;
        self.inner.put_if_version(key, data, expected_version).await
    }
    async fn delete(&self, key: &str) -> Result<bool, StorageError> {
        self.inner.delete(key).await
    }
    async fn list_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, VersionedRecord)>, StorageError> {
        self.inner.list_by_prefix(prefix).await
    }
    async fn cleanup_expired(&self) -> Result<usize, StorageError> {
        self.inner.cleanup_expired().await
    }
}

/// The extension requires the handle a create returns to be usable straight
/// away. The store satisfies that by NOT depending on read-after-write: `create`
/// returns the record it just wrote, having issued zero reads of the new key.
///
/// The second half is the measurement, recorded rather than smoothed over: on a
/// backend whose first read after a write is stale, a follow-up `tasks/get`
/// against the very same key can miss, and converges on the next read. The
/// handle is valid throughout; what is required is a strongly-consistent read
/// (or a client retry) on such a backend. Logged as `D-114-D`.
#[tokio::test]
async fn a_created_task_is_immediately_readable_from_its_returned_handle() {
    let store = GenericTaskStore::new(EventuallyConsistentBackend::new());

    let gets_before = store.backend().get_calls();
    let record = store.create(OWNER, "tools/call", None).await.unwrap();

    assert_eq!(
        store.backend().get_calls(),
        gets_before,
        "create must not read the record back -- the handle is the write it just made"
    );
    assert!(!record.task.task_id.is_empty(), "the handle carries an id");
    assert_eq!(record.task.status, TaskStatus::Working);
    assert!(record.version > 0, "the handle carries a durable version");

    // MEASURED behaviour of a stale first read.
    let first = store.get(&record.task.task_id, OWNER).await;
    assert!(
        matches!(first, Err(TaskError::NotFound { .. })),
        "a stale first read misses; recorded as D-114-D, got: {first:?}"
    );
    let converged = store
        .get(&record.task.task_id, OWNER)
        .await
        .expect("the read converges");
    assert_eq!(converged.task.task_id, record.task.task_id);
    assert_eq!(converged.owner_id, OWNER);
}

// ---------------------------------------------------------------------------
// 9. Backend contract: item size and write amplification
// ---------------------------------------------------------------------------

/// The `inputResponses` total-size bound enforced at request ingress, above this
/// seam: 256 KiB. Mirrored here as a local constant because the ingress
/// constant is crate-private to `pmcp`; if the two ever disagree, this test's
/// fixture stops being the worst case and the headroom claim weakens.
const MAX_INPUT_RESPONSES_TOTAL_BYTES: usize = 262_144;

/// The maximum entry COUNT enforced at the same place.
const MAX_INPUT_RESPONSES: usize = 64;

/// `DynamoDB`'s hard item-size limit: 400 KB.
const DYNAMODB_MAX_ITEM_BYTES: usize = 400 * 1024;

/// A worst-case task record -- a maximum-sized delivery, plus the metadata,
/// variables, outstanding requests and terminal result that share the item --
/// still fits a `DynamoDB` item, with headroom.
///
/// This is the capacity claim the "DynamoDB works from day one" promise rests
/// on: the whole record is ONE item, so a delivery at the ingress bound must not
/// push it past the backend's own limit.
#[test]
fn a_full_input_response_set_fits_the_dynamodb_item_budget() {
    let mut record = TaskRecord::new(OWNER.to_string(), "tools/call".to_string(), Some(60_000));
    record.task.status = TaskStatus::InputRequired;
    record.task.status_message = Some("awaiting input".to_string());
    record.task.poll_interval = Some(500);

    // A maximum-sized delivery: the entry-count bound, padded to just under the
    // total-size bound.
    let pad = "x".repeat(4_040);
    let mut responses = Map::new();
    let mut requests = Map::new();
    for index in 0..MAX_INPUT_RESPONSES {
        let key = format!("input-key-{index:03}");
        responses.insert(key.clone(), json!({ "answer": pad }));
        requests.insert(key, json!({ "kind": "elicitation", "prompt": "answer me" }));
    }
    let responses_bytes = serde_json::to_vec(&responses).unwrap().len();
    assert!(
        responses_bytes <= MAX_INPUT_RESPONSES_TOTAL_BYTES,
        "the fixture must be a LEGAL delivery: {responses_bytes} > {MAX_INPUT_RESPONSES_TOTAL_BYTES}"
    );
    assert!(
        responses_bytes > MAX_INPUT_RESPONSES_TOTAL_BYTES * 9 / 10,
        "the fixture must be a WORST CASE, not a small one: {responses_bytes}"
    );

    record.input_responses = Some(responses);
    record.input_requests = Some(requests);
    record.variables.insert(
        "progress".to_string(),
        json!({ "step": 3, "note": "x".repeat(1_024) }),
    );
    record.result = Some(json!({ "content": [{ "type": "text", "text": "x".repeat(4_096) }] }));
    record.error = Some(json!({ "code": -32603, "message": "upstream timed out" }));

    let item_bytes = serde_json::to_vec(&record).unwrap().len();
    // Printed so the measurement is reproducible with `--nocapture` rather than
    // only quotable from a plan document.
    println!(
        "worst-case record: inputResponses {responses_bytes} B, whole item {item_bytes} B, \
         DynamoDB limit {DYNAMODB_MAX_ITEM_BYTES} B"
    );
    assert!(
        item_bytes < DYNAMODB_MAX_ITEM_BYTES,
        "a worst-case record must fit a DynamoDB item: {item_bytes} >= {DYNAMODB_MAX_ITEM_BYTES}"
    );
    let headroom = DYNAMODB_MAX_ITEM_BYTES - item_bytes;
    assert!(
        headroom > 64 * 1024,
        "the fit must not be marginal; headroom was only {headroom} bytes"
    );
}

/// A backend that counts writes and bytes written.
#[derive(Debug)]
struct CountingBackend {
    inner: InMemoryBackend,
    writes: AtomicUsize,
    bytes: AtomicUsize,
}

impl CountingBackend {
    fn new() -> Self {
        Self {
            inner: InMemoryBackend::new(),
            writes: AtomicUsize::new(0),
            bytes: AtomicUsize::new(0),
        }
    }

    fn record_write(&self, data: &[u8]) {
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.bytes.fetch_add(data.len(), Ordering::SeqCst);
    }

    fn writes(&self) -> usize {
        self.writes.load(Ordering::SeqCst)
    }

    fn bytes(&self) -> usize {
        self.bytes.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl StorageBackend for CountingBackend {
    async fn get(&self, key: &str) -> Result<VersionedRecord, StorageError> {
        self.inner.get(key).await
    }
    async fn put(&self, key: &str, data: &[u8]) -> Result<u64, StorageError> {
        self.record_write(data);
        self.inner.put(key, data).await
    }
    async fn put_if_version(
        &self,
        key: &str,
        data: &[u8],
        expected_version: u64,
    ) -> Result<u64, StorageError> {
        self.record_write(data);
        self.inner.put_if_version(key, data, expected_version).await
    }
    async fn delete(&self, key: &str) -> Result<bool, StorageError> {
        self.inner.delete(key).await
    }
    async fn list_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, VersionedRecord)>, StorageError> {
        self.inner.list_by_prefix(prefix).await
    }
    async fn cleanup_expired(&self) -> Result<usize, StorageError> {
        self.inner.cleanup_expired().await
    }
}

/// A many-round elicitation must not become quadratic I/O on a pay-per-write
/// backend.
///
/// Each delivery rewrites the item exactly once, so the total bytes written are
/// bounded by N times the FINAL record size. The sharp form of the guarantee is
/// the write COUNT: one write per delivery, never a read-modify-write loop or a
/// retry storm.
#[tokio::test]
async fn partial_updates_do_not_amplify_writes_superlinearly() {
    const ROUNDS: usize = 8;

    let store = GenericTaskStore::new(CountingBackend::new());
    let keys: Vec<String> = (0..ROUNDS).map(|index| format!("key-{index}")).collect();
    let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    let task_id = paused_task(&store, OWNER, &key_refs).await;

    let writes_before = store.backend().writes();
    let bytes_before = store.backend().bytes();

    for key in &key_refs {
        store
            .deliver_inputs(&task_id, OWNER, responses_of(&[key]))
            .await
            .unwrap();
    }

    let writes = store.backend().writes() - writes_before;
    let bytes = store.backend().bytes() - bytes_before;
    assert_eq!(
        writes, ROUNDS,
        "each delivery must be exactly ONE write, got {writes} for {ROUNDS} deliveries"
    );

    let final_record = store.get(&task_id, OWNER).await.unwrap();
    let final_bytes = serde_json::to_vec(&final_record).unwrap().len();
    assert!(
        bytes <= ROUNDS * final_bytes,
        "write volume must stay within N x the final record size: {bytes} > {ROUNDS} x {final_bytes}"
    );
    assert!(bytes > 0, "the measurement must not be vacuous");
    assert_eq!(final_record.task.status, TaskStatus::Working);
}

// ---------------------------------------------------------------------------
// 10. Durability of pre-existing records
// ---------------------------------------------------------------------------

/// A record exactly as a build from before the tasks extension wrote it.
///
/// This is a raw SERIALIZED fixture on purpose. Building a struct and
/// serializing it would test today's serializer against itself and would keep
/// passing if one of the new fields silently became required -- which is the
/// failure it exists to catch. Every field a pre-extension build wrote is
/// present; the three the extension added are absent, and only those.
const PRE_114_RECORD: &str = r#"{"task":{"taskId":"pre-114-task","status":"input_required","createdAt":"2026-01-01T00:00:00.000Z","lastUpdatedAt":"2026-01-01T00:00:00.000Z","ttl":null,"pollInterval":5000},"ownerId":"owner-1","variables":{"progress":42},"result":null,"requestMethod":"tools/call","expiresAt":null}"#;

#[tokio::test]
async fn a_pre_114_record_still_deserializes() {
    let store = store();
    store
        .backend()
        .put(&make_key(OWNER, "pre-114-task"), PRE_114_RECORD.as_bytes())
        .await
        .unwrap();

    let record = store
        .get("pre-114-task", OWNER)
        .await
        .expect("a durable record written before this feature must still read");

    assert_eq!(record.task.task_id, "pre-114-task");
    assert_eq!(record.task.status, TaskStatus::InputRequired);
    assert_eq!(record.variables.get("progress"), Some(&json!(42)));
    assert!(record.input_requests.is_none(), "absent means empty");
    assert!(record.input_responses.is_none(), "absent means empty");
    assert!(record.error.is_none(), "absent means empty");

    // Absent-means-empty is not merely a deserialization fact: the record is
    // still OPERABLE. Nothing is outstanding, so every key is ignored and the
    // task stays paused rather than resuming on a vacuously complete set.
    let outcome = store
        .deliver_inputs("pre-114-task", OWNER, responses_of(&["city"]))
        .await
        .expect("a pre-extension record must still accept a delivery call");
    assert!(accepted(&outcome).is_empty());
    assert_eq!(ignored(&outcome), vec!["city".to_string()]);
    assert!(
        !is_complete(&outcome),
        "an empty request set is not complete"
    );
    assert_eq!(
        store.get("pre-114-task", OWNER).await.unwrap().task.status,
        TaskStatus::InputRequired
    );

    // And it has no snapshot, because the server never asked for anything.
    assert!(matches!(
        store.task_input_snapshot("pre-114-task", OWNER).await,
        Err(TaskError::NotFound { .. })
    ));
}

/// An untouched record's serialized bytes do not grow: the three added fields
/// are omitted while `None`, so upgrading the code does not rewrite storage.
#[test]
fn an_untouched_record_does_not_grow_on_the_wire() {
    let record = TaskRecord::new(OWNER.to_string(), "tools/call".to_string(), Some(60_000));
    let bytes = serde_json::to_string(&record).unwrap();
    for added in ["inputRequests", "inputResponses", "error"] {
        assert!(
            !bytes.contains(added),
            "an untouched record must not carry {added}: {bytes}"
        );
    }
}

// ---------------------------------------------------------------------------
// 11. Anonymous owners
// ---------------------------------------------------------------------------

/// `GenericTaskStore` refuses the anonymous principal (`""`) and the
/// single-user `"local"` owner unless anonymous access is explicitly enabled,
/// and it treats the two IDENTICALLY -- `is_anonymous_owner` is one predicate
/// over both.
///
/// # Asymmetry worth knowing before reading the phase's example
///
/// `pmcp`'s in-crate `InMemoryTaskStore` has NO such check: it will happily mint
/// and feed tasks for an empty owner. That is why the phase's runnable example
/// uses the in-crate store, and why a server that swaps in this crate's store
/// must configure an owner source (OAuth) or opt into `allow_anonymous`
/// deliberately -- at which point every anonymous caller shares one bucket.
#[tokio::test]
async fn anonymous_owner_is_refused_by_default_on_this_backend() {
    let store = store();

    for anonymous in ["", "local"] {
        let delivered = store
            .deliver_inputs("any-task", anonymous, responses_of(&["city"]))
            .await;
        assert!(
            matches!(&delivered, Err(TaskError::StoreError(message)) if message.contains("anonymous access")),
            "deliver_inputs must refuse the {anonymous:?} owner, got: {delivered:?}"
        );

        let recorded = store
            .record_input_requests("any-task", anonymous, requests_of(&["city"]))
            .await;
        assert!(
            matches!(&recorded, Err(TaskError::StoreError(message)) if message.contains("anonymous access")),
            "record_input_requests must refuse the {anonymous:?} owner, got: {recorded:?}"
        );
    }

    // Non-vacuity: both are ACCEPTED once anonymous access is enabled, so the
    // refusal above is the configuration and not an unrelated failure.
    let permissive = GenericTaskStore::new(InMemoryBackend::new())
        .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));
    for anonymous in ["", "local"] {
        let task_id = paused_task(&permissive, anonymous, &["city"]).await;
        let outcome = permissive
            .deliver_inputs(&task_id, anonymous, responses_of(&["city"]))
            .await
            .expect("an enabled anonymous owner must be able to deliver");
        assert!(is_complete(&outcome));
    }
}

// ---------------------------------------------------------------------------
// 12. Delegation: all three sites
// ---------------------------------------------------------------------------

const STORE_MOD_RS: &str = include_str!("../src/store/mod.rs");
const STORE_MEMORY_RS: &str = include_str!("../src/store/memory.rs");

/// Extracts the body of the first block introduced by `header`, up to the first
/// line that is a bare `}` at column zero.
fn block_after(source: &str, header: &str) -> String {
    let start = source
        .find(header)
        .unwrap_or_else(|| panic!("block header not found (did it get renamed?): {header}"));
    let rest = &source[start + header.len()..];
    let end = rest.find("\n}\n").unwrap_or(rest.len());
    rest[..end].to_string()
}

/// Every method name declared in a block, ignoring comments and doc comments.
fn method_names(block: &str) -> Vec<String> {
    block
        .lines()
        .map(str::trim_start)
        .filter(|line| !line.starts_with("//"))
        .filter_map(|line| {
            let signature = line
                .strip_prefix("async fn ")
                .or_else(|| line.strip_prefix("fn "))?;
            signature.split('(').next().map(str::to_string)
        })
        .collect()
}

/// The forgettable-delegation tripwire.
///
/// The five input-delivery methods are the only `TaskStore` methods with a
/// DEFAULT body. A missing forwarding line therefore COMPILES and then reports
/// "store does not support task input delivery" at runtime -- on the in-memory
/// path only, while the identical `DynamoDB` and Redis paths work. That is a
/// bug no compiler and no type signature can catch, so it is caught here
/// structurally: the trait, the blanket impl and the wrapper are read at
/// build time and compared, and a method added later without a forwarding line
/// fails HERE instead of in production.
#[test]
fn every_generic_store_method_is_delegated_by_the_memory_wrapper() {
    let trait_methods = method_names(&block_after(
        STORE_MOD_RS,
        "pub trait TaskStore: Send + Sync {",
    ));
    let blanket = block_after(
        STORE_MOD_RS,
        "impl<B: StorageBackend + 'static> TaskStore for generic::GenericTaskStore<B> {",
    );
    let wrapper = block_after(STORE_MEMORY_RS, "impl TaskStore for InMemoryTaskStore {");

    assert!(
        trait_methods.len() >= 16,
        "the scan found only {} trait methods -- it is matching nothing and would pass vacuously: {trait_methods:?}",
        trait_methods.len()
    );
    for expected in [
        "deliver_inputs",
        "task_input_snapshot",
        "record_input_requests",
        "set_error",
        "get_error",
    ] {
        assert!(
            trait_methods.iter().any(|name| name == expected),
            "the scan missed the input-delivery methods it exists to guard: {trait_methods:?}"
        );
    }

    for method in &trait_methods {
        let needle = format!("fn {method}(");
        assert!(
            wrapper.contains(&needle),
            "InMemoryTaskStore does not delegate `{method}` -- it would silently inherit the \
             trait default while GenericTaskStore's implementation works"
        );
        assert!(
            blanket.contains(&needle),
            "the blanket TaskStore impl for GenericTaskStore does not forward `{method}` -- every \
             Arc<dyn TaskStore> would inherit the trait default"
        );
    }
}

/// The runtime half of the same claim: the wrapper really does deliver, through
/// a trait object, which is how a server holds a store.
#[tokio::test]
async fn the_memory_wrapper_delivers_inputs_through_a_trait_object() {
    let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());

    let record = store.create(OWNER, "tools/call", None).await.unwrap();
    let task_id = record.task.task_id;

    store
        .record_input_requests(&task_id, OWNER, requests_of(&["city"]))
        .await
        .expect("the wrapper must record requests, not report 'not supported'");

    let snapshot = store.task_input_snapshot(&task_id, OWNER).await.unwrap();
    assert_eq!(snapshot["status"], json!("input_required"));
    assert!(snapshot["inputRequests"].get("city").is_some());
    assert_eq!(snapshot["inputResponses"], json!({}));

    let outcome = store
        .deliver_inputs(&task_id, OWNER, responses_of(&["city"]))
        .await
        .expect("the wrapper must deliver, not report 'not supported'");
    assert!(is_complete(&outcome));
    assert_eq!(
        store.get(&task_id, OWNER).await.unwrap().task.status,
        TaskStatus::Working
    );
}

/// The router half (D-13 site 3): `tasks/update` reaches the store across the
/// `Value` seam.
#[tokio::test]
async fn the_router_delivers_inputs_across_the_value_seam() {
    use pmcp::server::tasks::TaskRouter;

    let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
    let record = store.create(OWNER, "tools/call", None).await.unwrap();
    let task_id = record.task.task_id;
    store
        .record_input_requests(&task_id, OWNER, requests_of(&["city"]))
        .await
        .unwrap();

    let router = pmcp_tasks::router::TaskRouterImpl::new(Arc::clone(&store));
    let outcome = router
        .handle_tasks_update(
            json!({ "taskId": task_id, "inputResponses": { "city": { "answer": "Paris" } } }),
            OWNER,
        )
        .await
        .expect("the router must reach the store");

    assert!(is_complete(&outcome));
    assert_eq!(accepted(&outcome), vec!["city".to_string()]);
    assert_eq!(
        store.get(&task_id, OWNER).await.unwrap().task.status,
        TaskStatus::Working
    );
}

/// An owner named in client-supplied `params` must have no effect: the owner is
/// the one the caller resolved and passed as an argument.
///
/// Without this the owner-prefixed key would be defeated by the simplest
/// possible attack -- naming somebody else in the request body.
#[tokio::test]
async fn the_router_ignores_an_owner_supplied_in_params() {
    use pmcp::server::tasks::TaskRouter;

    let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
    let record = store.create("owner-a", "tools/call", None).await.unwrap();
    let task_id = record.task.task_id;
    store
        .record_input_requests(&task_id, "owner-a", requests_of(&["city"]))
        .await
        .unwrap();

    let router = pmcp_tasks::router::TaskRouterImpl::new(Arc::clone(&store));
    let result = router
        .handle_tasks_update(
            json!({
                "taskId": task_id,
                "inputResponses": { "city": { "answer": "tampered" } },
                "ownerId": "owner-a",
                "owner": "owner-a",
            }),
            "owner-b",
        )
        .await;

    assert!(
        result.is_err(),
        "an owner named in params must not grant access: {result:?}"
    );
    let untouched = store.get(&task_id, "owner-a").await.unwrap();
    assert_eq!(untouched.task.status, TaskStatus::InputRequired);
}

// ---------------------------------------------------------------------------
// 13. Recording requests and persisting errors
// ---------------------------------------------------------------------------

/// Requests and the pause land in ONE write, so a task is never observable as
/// `input_required` with nothing recorded to answer.
#[tokio::test]
async fn record_input_requests_pauses_the_task_in_one_write() {
    let store = store();
    let created = store.create(OWNER, "tools/call", None).await.unwrap();
    let task_id = created.task.task_id;

    let paused = store
        .record_input_requests(&task_id, OWNER, requests_of(&["city"]))
        .await
        .unwrap();
    assert_eq!(paused["status"], json!("input_required"));

    let record = store.get(&task_id, OWNER).await.unwrap();
    assert_eq!(record.task.status, TaskStatus::InputRequired);
    assert!(record.input_requests.as_ref().unwrap().contains_key("city"));
    assert_eq!(
        record.version,
        created.version + 1,
        "requests + transition must be ONE write"
    );
}

/// A second round is refused while the first is still outstanding, and the
/// refusal comes from the shared state machine rather than a second predicate
/// that could drift away from it.
#[tokio::test]
async fn record_input_requests_while_awaiting_input_is_refused() {
    let store = store();
    let task_id = paused_task(&store, OWNER, &["city"]).await;

    let second = store
        .record_input_requests(&task_id, OWNER, requests_of(&["country"]))
        .await;

    assert!(
        matches!(second, Err(TaskError::InvalidTransition { .. })),
        "a second round must not start while one is outstanding, got: {second:?}"
    );
    let record = store.get(&task_id, OWNER).await.unwrap();
    assert_eq!(record.input_requests.as_ref().unwrap().len(), 1);
}

/// Once the previous round is answered a NEW round may be recorded and merges
/// in -- and a key that is already recorded is refused, because overwriting it
/// would orphan the answer delivered against it.
#[tokio::test]
async fn record_input_requests_starts_a_second_round_but_refuses_a_reused_key() {
    let store = store();
    let task_id = paused_task(&store, OWNER, &["city"]).await;
    store
        .deliver_inputs(&task_id, OWNER, responses_of(&["city"]))
        .await
        .unwrap();

    let reused = store
        .record_input_requests(&task_id, OWNER, requests_of(&["city"]))
        .await;
    assert!(
        matches!(&reused, Err(TaskError::StoreError(message)) if message.contains("already recorded")),
        "a reused key must be refused, got: {reused:?}"
    );

    store
        .record_input_requests(&task_id, OWNER, requests_of(&["country"]))
        .await
        .expect("a fresh key starts a second round");

    let record = store.get(&task_id, OWNER).await.unwrap();
    assert_eq!(record.task.status, TaskStatus::InputRequired);
    assert_eq!(record.input_requests.as_ref().unwrap().len(), 2);
    assert!(
        record
            .input_responses
            .as_ref()
            .unwrap()
            .contains_key("city"),
        "the first round's answer must survive a second round"
    );
}

/// The terminal JSON-RPC error round-trips verbatim and is owner-scoped.
#[tokio::test]
async fn set_error_then_get_error_round_trips_verbatim() {
    let store = store();
    let record = store.create(OWNER, "tools/call", None).await.unwrap();
    let task_id = record.task.task_id;

    assert!(
        matches!(
            store.get_error(&task_id, OWNER).await,
            Err(TaskError::NotFound { .. })
        ),
        "a task that has not failed has no error to return"
    );

    let error =
        json!({ "code": -32603, "message": "upstream timed out", "data": { "retryable": true } });
    store
        .set_error(&task_id, OWNER, error.clone())
        .await
        .unwrap();

    assert_eq!(store.get_error(&task_id, OWNER).await.unwrap(), error);
    assert!(
        matches!(
            store.get_error(&task_id, "owner-b").await,
            Err(TaskError::NotFound { .. })
        ),
        "another owner must not read the error"
    );
}

/// The snapshot reports the FULL recorded set plus what has been answered, so
/// the layer above can derive the outstanding subset without a second read.
#[tokio::test]
async fn task_input_snapshot_reports_requests_responses_and_status() {
    let store = store();
    let task_id = paused_task(&store, OWNER, &["city", "country"]).await;
    store
        .deliver_inputs(&task_id, OWNER, responses_of(&["city"]))
        .await
        .unwrap();

    let snapshot = store.task_input_snapshot(&task_id, OWNER).await.unwrap();
    assert_eq!(snapshot["status"], json!("input_required"));
    assert_eq!(
        snapshot["inputRequests"].as_object().unwrap().len(),
        2,
        "the FULL recorded set, not the outstanding subset"
    );
    assert_eq!(snapshot["inputResponses"].as_object().unwrap().len(), 1);

    assert!(
        matches!(
            store.task_input_snapshot(&task_id, "owner-b").await,
            Err(TaskError::NotFound { .. })
        ),
        "another owner must not read the snapshot"
    );
}

/// A non-object delivery is refused, and the message names the TYPE only --
/// never the rejected content, which may be client-supplied.
#[tokio::test]
async fn deliver_inputs_refuses_a_non_object_payload() {
    let store = store();
    let task_id = paused_task(&store, OWNER, &["city"]).await;

    let result = store
        .deliver_inputs(&task_id, OWNER, json!("not-an-object"))
        .await;

    match result {
        Err(TaskError::StoreError(message)) => {
            assert!(message.contains("inputResponses"), "got: {message}");
            assert!(
                !message.contains("not-an-object"),
                "the rejected content must not be echoed: {message}"
            );
        },
        other => panic!("expected StoreError, got: {other:?}"),
    }
}

/// The `pmcp` typed shapes and this crate's hand-written JSON literals are the
/// SAME wire contract, on the two sides of the `serde_json::Value` seam.
///
/// Nothing used to ENFORCE that. `TaskInputDelivery` and `TaskInputSnapshot`
/// derived no `Serialize` at all, so the trait docs' "mirroring pmcp's
/// `TaskInputDelivery` field for field" was an aspiration a reader had to
/// verify by eye — and `TaskInputSnapshot`'s snake_case field names would not
/// even have produced the literal's `inputRequests` / `inputResponses` keys.
///
/// With `Serialize` derived above the seam, the claim is checkable here: rename
/// a field on either side and this test fails, instead of the mismatch
/// surfacing at runtime as a key the other layer silently does not read.
#[tokio::test]
async fn the_emitted_json_mirrors_pmcps_typed_shapes_key_for_key() {
    fn key_set(value: &Value) -> std::collections::BTreeSet<String> {
        value
            .as_object()
            .expect("expected a JSON object")
            .keys()
            .cloned()
            .collect()
    }

    fn expected(keys: &[&str]) -> std::collections::BTreeSet<String> {
        keys.iter().map(|k| (*k).to_string()).collect()
    }

    let store = store();
    let task_id = paused_task(&store, OWNER, &["city"]).await;
    let emitted_delivery = store
        .deliver_inputs(&task_id, OWNER, responses_of(&["city"]))
        .await
        .unwrap();
    let emitted_snapshot = store.task_input_snapshot(&task_id, OWNER).await.unwrap();

    let typed_delivery =
        serde_json::to_value(pmcp::server::task_store::TaskInputDelivery::default())
            .expect("TaskInputDelivery serializes");
    assert_eq!(
        key_set(&typed_delivery),
        key_set(&emitted_delivery),
        "deliver_inputs' JSON literal and pmcp's TaskInputDelivery disagree about their keys"
    );
    assert_eq!(
        key_set(&typed_delivery),
        expected(&["accepted", "ignored", "complete"]),
        "both sides drifted together; the wire keys themselves changed"
    );

    let typed_snapshot = serde_json::to_value(pmcp::server::task_store::TaskInputSnapshot {
        input_requests: pmcp::types::mrtr::InputRequests::new(),
        input_responses: pmcp::types::mrtr::InputResponses::new(),
        status: pmcp::types::tasks::TaskStatus::default(),
    })
    .expect("TaskInputSnapshot serializes");
    assert_eq!(
        key_set(&typed_snapshot),
        key_set(&emitted_snapshot),
        "task_input_snapshot's JSON literal and pmcp's TaskInputSnapshot disagree about their keys"
    );
    assert_eq!(
        key_set(&typed_snapshot),
        expected(&["inputRequests", "inputResponses", "status"]),
        "both sides drifted together; the wire keys themselves changed"
    );
}
