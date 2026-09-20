#![cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]
//! The DEFAULT on-disk credential store — permissions, atomicity, concurrency,
//! migration and the administrative operations, all against a real tempdir.
//!
//! This file IS `#![cfg(feature = "oauth")]`, which is the exact inverse of
//! `tests/oauth_credential_store.rs`'s deliberate ungatedness. The format tier
//! is ungated so a platform can implement the seam without the `oauth` feature;
//! the FILE tier is gated because a home directory is not a viable store on any
//! hosting target, and `dirs` only exists behind `oauth`.
//!
//! Groups:
//! 1. Construction and basic I/O — the I/O-free constructor, round trip, refusals
//! 2. Permissions and atomicity — ported from `cargo-pmcp`'s `write_atomic`
//! 3. Concurrency — the LOST UPDATE an atomic rename alone never prevents
//! 4. The combined update — `save_with_issuer` as ONE read-modify-write
//! 5. Migration — schema 1 read without a rewrite
//! 6. Administrative operations — the same semantics as the in-memory store

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use pmcp::shared::credential_file::{
    CREDENTIAL_LOCK_STALE_SECS, CREDENTIAL_LOCK_SUFFIX, CREDENTIAL_WRITE_EVENT_TARGET,
};
use pmcp::shared::credential_store::CREDENTIAL_SCHEMA_VERSION;
use pmcp::{
    default_credential_path, CredentialKey, CredentialSnapshot, CredentialStore,
    CredentialStoreAdmin, FileCredentialStore, StoredCredentials,
};
use serde_json::Value;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const AS_ALPHA: &str = "https://as-alpha.example";
const AS_BETA: &str = "https://as-beta.example";
const MCP_ALPHA: &str = "https://mcp-alpha.example";
const MCP_BETA: &str = "https://mcp-beta.example";
const CACHE_FILE: &str = "oauth-cache.json";

/// A schema-1 document in `cargo-pmcp`'s `TokenCacheV1` shape, written as a
/// LITERAL rather than built from a core-authored struct on purpose: if either
/// side's field names drift, this fixture stops migrating and the test fails.
const LEGACY_TWO_ENTRIES: &[u8] = br#"{
  "schema_version": 1,
  "entries": {
    "https://mcp-alpha.example": {
      "access_token": "alpha-token",
      "refresh_token": "alpha-refresh",
      "expires_at": 1893456000,
      "scopes": ["mcp:read", "mcp:write"],
      "issuer": "https://as-alpha.example",
      "client_id": "alpha-client"
    },
    "https://mcp-beta.example": {
      "access_token": "beta-token",
      "issuer": "https://as-beta.example",
      "client_id": "beta-client"
    }
  }
}"#;

/// The same shape with an entry that records NO issuer — the case a migration
/// cannot re-key without guessing which authorization server issued it.
const LEGACY_ONE_UNKEYABLE: &[u8] = br#"{
  "schema_version": 1,
  "entries": {
    "https://mcp-alpha.example": {
      "access_token": "alpha-token",
      "issuer": "https://as-alpha.example",
      "client_id": "alpha-client"
    },
    "https://mcp-orphan.example": {
      "access_token": "orphan-token",
      "client_id": "orphan-client"
    }
  }
}"#;

fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("a temporary directory")
}

fn cache_path(dir: &TempDir) -> PathBuf {
    dir.path().join(CACHE_FILE)
}

fn store_in(dir: &TempDir) -> FileCredentialStore {
    FileCredentialStore::new(cache_path(dir))
}

fn lock_path(path: &Path) -> PathBuf {
    let mut name = path.to_path_buf().into_os_string();
    name.push(CREDENTIAL_LOCK_SUFFIX);
    PathBuf::from(name)
}

fn key(issuer: &str, server: &str) -> CredentialKey {
    CredentialKey::new(issuer, "", server)
}

fn creds(token: &str) -> StoredCredentials {
    StoredCredentials::new(token, "client-id")
}

fn read_document(path: &Path) -> Value {
    let bytes = fs::read(path).expect("the credential file exists");
    serde_json::from_slice(&bytes).expect("the credential file is JSON")
}

/// Force a file's modification time backwards, so a stale-lock test does not
/// have to sleep for the real staleness window.
fn age(path: &Path, by: Duration) {
    let file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("the file to age is writable");
    let when = SystemTime::now() - by;
    let times = fs::FileTimes::new().set_accessed(when).set_modified(when);
    file.set_times(times).expect("the file times are settable");
}

/// Counts completed credential-file writes by observing the store's own
/// `tracing` event.
///
/// This exists because "exactly one write" is not observable through the
/// filesystem after the fact: an atomic rename leaves the same evidence whether
/// it happened once or twice. Without a counter, removing the `save_with_issuer`
/// override would produce a byte-identical file and every other assertion would
/// still pass.
#[derive(Debug, Default)]
struct WriteCounter {
    writes: Arc<AtomicUsize>,
}

impl tracing::Subscriber for WriteCounter {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        metadata.target() == CREDENTIAL_WRITE_EVENT_TARGET
    }

    fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        if event.metadata().target() == CREDENTIAL_WRITE_EVENT_TARGET {
            self.writes.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn enter(&self, _span: &tracing::span::Id) {}

    fn exit(&self, _span: &tracing::span::Id) {}
}

/// Install the counter for the rest of the current test, on this thread.
///
/// `#[tokio::test]` without a flavor runs a current-thread runtime, so the whole
/// future stays on the thread the dispatcher was installed on.
fn count_writes() -> (Arc<AtomicUsize>, tracing::dispatcher::DefaultGuard) {
    let writes = Arc::new(AtomicUsize::new(0));
    let counter = WriteCounter {
        writes: Arc::clone(&writes),
    };
    let guard = tracing::dispatcher::set_default(&tracing::Dispatch::new(counter));
    (writes, guard)
}

/// Three credentials over two servers, mirroring the fixture
/// `InMemoryCredentialStore`'s administrative tests use, so the file store's
/// counts can be compared against the same numbers.
async fn seed_three(store: &FileCredentialStore) {
    let alpha_default = CredentialKey::new(AS_ALPHA, "", MCP_ALPHA);
    let alpha_second = CredentialKey::new(AS_ALPHA, "second-account", MCP_ALPHA);
    let beta = CredentialKey::new(AS_BETA, "", MCP_BETA);

    store
        .save_with_issuer(&alpha_default, &creds("a1"), MCP_ALPHA, AS_ALPHA)
        .await
        .expect("the first credential saves");
    store
        .save_with_issuer(&alpha_second, &creds("a2"), MCP_ALPHA, AS_ALPHA)
        .await
        .expect("the second credential saves");
    store
        .save_with_issuer(&beta, &creds("b1"), MCP_BETA, AS_BETA)
        .await
        .expect("the third credential saves");
}

// ---------------------------------------------------------------------------
// 1. Construction and basic I/O
// ---------------------------------------------------------------------------

/// D-07's I/O-free-construction rule, asserted rather than asserted-about: a
/// Lambda that injects its own store must never have a home directory touched
/// on its behalf, and the only way to know is to construct against a path whose
/// directory does not exist and watch nothing appear.
#[tokio::test]
async fn new_touches_nothing_and_a_load_does_not_either() {
    let dir = temp_dir();
    let nested = dir.path().join("does").join("not").join("exist");
    let path = nested.join(CACHE_FILE);

    let store = FileCredentialStore::new(path.clone());
    assert!(!nested.exists(), "the constructor created a directory");
    assert!(!path.exists(), "the constructor created a file");

    let loaded = store
        .load(&key(AS_ALPHA, MCP_ALPHA))
        .await
        .expect("a missing file is not an error");
    assert!(loaded.is_none(), "a missing file must read as None");
    assert!(!nested.exists(), "a load created a directory");

    store
        .save(&key(AS_ALPHA, MCP_ALPHA), &creds("token"))
        .await
        .expect("a save creates what it needs");
    assert!(path.exists(), "only a write creates the file");
}

#[tokio::test]
async fn save_then_load_round_trips_through_a_real_file() {
    let dir = temp_dir();
    let store = store_in(&dir);
    let target = key(AS_ALPHA, MCP_ALPHA);
    let stored = creds("round-trip-token")
        .with_refresh_token("refresh")
        .with_expires_at(1_893_456_000)
        .with_granted_scopes(["mcp:read"]);

    store.save(&target, &stored).await.expect("the save works");

    let reloaded = FileCredentialStore::new(cache_path(&dir))
        .load(&target)
        .await
        .expect("the load works")
        .expect("the credential is present");
    assert_eq!(reloaded.access_token(), "round-trip-token");
    assert_eq!(reloaded.refresh_token(), Some("refresh"));
    assert_eq!(reloaded.expires_at(), Some(1_893_456_000));
    assert_eq!(reloaded.granted_scopes(), ["mcp:read".to_string()]);
    assert_eq!(reloaded.client_id(), "client-id");
}

#[tokio::test]
async fn a_file_this_store_writes_carries_the_current_schema_version() {
    let dir = temp_dir();
    let store = store_in(&dir);
    store
        .save(&key(AS_ALPHA, MCP_ALPHA), &creds("token"))
        .await
        .expect("the save works");

    let document = read_document(&cache_path(&dir));
    assert_eq!(
        document["schema_version"],
        Value::from(CREDENTIAL_SCHEMA_VERSION)
    );
}

/// A corrupt file must produce an ACTIONABLE refusal that names the path — and
/// must not reproduce a single byte of what it read, because the bytes in
/// question are bearer and refresh tokens.
#[tokio::test]
async fn a_corrupt_file_names_the_path_says_what_to_do_and_echoes_no_content() {
    const CANARY: &str = "SUPER-SECRET-CANARY-VALUE";

    let dir = temp_dir();
    let path = cache_path(&dir);
    fs::write(
        &path,
        format!(r#"{{"schema_version": 2, "credentials": "{CANARY}"}}"#),
    )
    .expect("the fixture writes");

    let store = FileCredentialStore::new(path.clone());
    let message = store
        .load(&key(AS_ALPHA, MCP_ALPHA))
        .await
        .expect_err("a corrupt file is an error, not an empty store")
        .to_string();

    assert!(
        message.contains(&path.display().to_string()),
        "the refusal must name the path: {message}"
    );
    assert!(
        message.contains("delete"),
        "the refusal must say how to reset: {message}"
    );
    assert!(
        !message.contains(CANARY),
        "the refusal reproduced file content: {message}"
    );
}

// ---------------------------------------------------------------------------
// 2. Permissions and atomicity
// ---------------------------------------------------------------------------

/// Ported from `cargo-pmcp`'s `write_sets_0600_perms_on_unix`. The parent is a
/// directory THIS STORE creates (not the tempdir itself, which is already
/// 0o700), so the 0o700 half of the assertion is not vacuous.
#[cfg(unix)]
#[tokio::test]
async fn save_sets_0600_on_the_file_and_0700_on_the_parent_it_creates() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir();
    let parent = dir.path().join("dot-pmcp");
    let path = parent.join(CACHE_FILE);

    FileCredentialStore::new(path.clone())
        .save(&key(AS_ALPHA, MCP_ALPHA), &creds("token"))
        .await
        .expect("the save works");

    let file_mode = fs::metadata(&path)
        .expect("the file exists")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(file_mode, 0o600, "credential file mode");

    let parent_mode = fs::metadata(&parent)
        .expect("the parent exists")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(parent_mode, 0o700, "credential directory mode");
}

#[cfg(unix)]
#[tokio::test]
async fn a_pre_existing_loose_file_is_tightened_by_the_next_save() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir();
    let path = cache_path(&dir);
    let store = FileCredentialStore::new(path.clone());

    store
        .save(&key(AS_ALPHA, MCP_ALPHA), &creds("first"))
        .await
        .expect("the first save works");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
        .expect("the fixture can loosen the mode");
    assert_eq!(
        fs::metadata(&path)
            .expect("the file exists")
            .permissions()
            .mode()
            & 0o777,
        0o644
    );

    store
        .save(&key(AS_BETA, MCP_BETA), &creds("second"))
        .await
        .expect("the second save works");

    assert_eq!(
        fs::metadata(&path)
            .expect("the file exists")
            .permissions()
            .mode()
            & 0o777,
        0o600,
        "a save must re-tighten a file someone loosened"
    );
}

/// The write goes through a same-directory temporary and a rename, so a reader
/// never sees a half-written document. The observable is that nothing is left
/// behind: after a save the directory holds the credential file and nothing
/// else — no temporary, no lock.
#[tokio::test]
async fn a_save_leaves_neither_a_temporary_nor_a_lock_behind() {
    let dir = temp_dir();
    let store = store_in(&dir);
    store
        .save(&key(AS_ALPHA, MCP_ALPHA), &creds("token"))
        .await
        .expect("the save works");

    let entries: Vec<_> = fs::read_dir(dir.path())
        .expect("the directory is readable")
        .map(|entry| {
            entry
                .expect("the entry is readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        entries,
        vec![CACHE_FILE.to_string()],
        "leftovers: {entries:?}"
    );
}

// ---------------------------------------------------------------------------
// 3. Concurrency — the lost update
// ---------------------------------------------------------------------------

/// **What breaks without this:** two tasks in one process each read the same
/// snapshot, each add their own key, and each write their own version back.
/// The second write wins and the first credential is GONE — with no error
/// anywhere, so a test that only asserted `is_ok()` would pass. The assertion
/// therefore has to be on the FINAL key set.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_concurrent_saves_on_one_store_both_survive() {
    let dir = temp_dir();
    let store = store_in(&dir);
    let alpha = key(AS_ALPHA, MCP_ALPHA);
    let beta = key(AS_BETA, MCP_BETA);

    let (alpha_credentials, beta_credentials) = (creds("alpha"), creds("beta"));
    let (first, second) = tokio::join!(
        store.save(&alpha, &alpha_credentials),
        store.save(&beta, &beta_credentials),
    );
    first.expect("the first save works");
    second.expect("the second save works");

    let keys = store.list_keys().await.expect("the keys are readable");
    assert!(keys.contains(&alpha), "alpha was lost: {keys:?}");
    assert!(keys.contains(&beta), "beta was lost: {keys:?}");
    assert_eq!(keys.len(), 2, "{keys:?}");
}

/// **What breaks without this:** the same lost update ACROSS store instances —
/// the cross-process case, exercised with two instances rather than two
/// processes because a test cannot fork. The in-process `Mutex` cannot help
/// here (there are two of them), so this is the case that proves the advisory
/// lock file is doing real work.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_instances_over_one_path_saving_concurrently_both_survive() {
    let dir = temp_dir();
    let path = cache_path(&dir);
    let alpha = key(AS_ALPHA, MCP_ALPHA);
    let beta = key(AS_BETA, MCP_BETA);

    let first = Arc::new(FileCredentialStore::new(path.clone()));
    let second = Arc::new(FileCredentialStore::new(path.clone()));

    let alpha_task = tokio::spawn({
        let (store, target) = (Arc::clone(&first), alpha.clone());
        async move { store.save(&target, &creds("alpha")).await }
    });
    let beta_task = tokio::spawn({
        let (store, target) = (Arc::clone(&second), beta.clone());
        async move { store.save(&target, &creds("beta")).await }
    });

    alpha_task
        .await
        .expect("the alpha task joins")
        .expect("the alpha save works");
    beta_task
        .await
        .expect("the beta task joins")
        .expect("the beta save works");

    let keys = FileCredentialStore::new(path)
        .list_keys()
        .await
        .expect("the keys are readable");
    assert!(keys.contains(&alpha), "alpha was lost: {keys:?}");
    assert!(keys.contains(&beta), "beta was lost: {keys:?}");
    assert_eq!(keys.len(), 2, "{keys:?}");
}

/// **What breaks without this:** an implementation that reads the file BEFORE
/// waiting for the lock. Such a store passes both tests above by luck of
/// timing, and still loses updates in production. Here the holder's write lands
/// WHILE the waiter is blocked, so a waiter that read early necessarily writes
/// a snapshot that does not contain the holder's credential. This is the one
/// concurrency test that fails deterministically under the bug.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_waiter_reads_the_document_the_lock_holder_left_behind() {
    let dir = temp_dir();
    let path = cache_path(&dir);
    let lock = lock_path(&path);
    let alpha = key(AS_ALPHA, MCP_ALPHA);
    let beta = key(AS_BETA, MCP_BETA);

    // Stand in for a holder that is midway through its read-modify-write.
    fs::write(&lock, b"").expect("the fixture can take the lock");

    let store = Arc::new(FileCredentialStore::new(path.clone()));
    let waiting = tokio::spawn({
        let (store, target) = (Arc::clone(&store), beta.clone());
        async move { store.save(&target, &creds("beta")).await }
    });

    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        !waiting.is_finished(),
        "a save proceeded while the lock was held"
    );

    // The holder completes its own atomic write, then releases.
    let mut holder_view = CredentialSnapshot::new();
    holder_view.insert(alpha.clone(), creds("alpha"));
    fs::write(
        &path,
        holder_view.to_bytes().expect("the fixture serializes"),
    )
    .expect("the fixture writes the holder's document");
    fs::remove_file(&lock).expect("the fixture can release the lock");

    waiting
        .await
        .expect("the waiting task joins")
        .expect("the waiting save works");

    let keys = store.list_keys().await.expect("the keys are readable");
    assert!(
        keys.contains(&alpha),
        "LOST UPDATE — the waiter read before it locked: {keys:?}"
    );
    assert!(keys.contains(&beta), "{keys:?}");
    assert_eq!(keys.len(), 2, "{keys:?}");
}

/// A lock abandoned by a crashed process must not wedge the store forever.
#[tokio::test]
async fn a_stale_lock_is_broken_so_a_crash_cannot_wedge_the_store() {
    let dir = temp_dir();
    let path = cache_path(&dir);
    let lock = lock_path(&path);
    let store = FileCredentialStore::new(path.clone());

    fs::write(&lock, b"").expect("the fixture can take the lock");
    age(&lock, Duration::from_secs(CREDENTIAL_LOCK_STALE_SECS * 4));

    let started = Instant::now();
    store
        .save(&key(AS_ALPHA, MCP_ALPHA), &creds("token"))
        .await
        .expect("a stale lock does not block a save");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the stale lock was waited on rather than broken"
    );

    assert!(!lock.exists(), "the broken lock was not cleaned up");
    assert!(store
        .load(&key(AS_ALPHA, MCP_ALPHA))
        .await
        .expect("the load works")
        .is_some());
}

/// Reads do NOT take the lock: a document that was renamed into place is
/// already consistent, so making readers queue behind a writer would buy
/// nothing and would let one wedged writer stall every reader.
#[tokio::test]
async fn a_load_succeeds_while_a_lock_file_exists() {
    let dir = temp_dir();
    let path = cache_path(&dir);
    let store = FileCredentialStore::new(path.clone());
    let target = key(AS_ALPHA, MCP_ALPHA);

    store
        .save(&target, &creds("token"))
        .await
        .expect("the save works");
    fs::write(lock_path(&path), b"").expect("the fixture can take the lock");

    assert!(store
        .load(&target)
        .await
        .expect("a load must not wait on the lock")
        .is_some());
    assert_eq!(
        store
            .list_keys()
            .await
            .expect("an enumeration must not wait on the lock")
            .len(),
        1
    );
    assert_eq!(
        store
            .last_issuer(MCP_ALPHA)
            .await
            .expect("an issuer read must not wait on the lock"),
        None
    );
}

// ---------------------------------------------------------------------------
// 4. The combined update
// ---------------------------------------------------------------------------

/// `save_with_issuer` is overridden here to be ONE read-modify-write under ONE
/// lock. The trait's default calls `save` and then `record_issuer`, which
/// leaves a window in which the store names one issuer while holding another's
/// credentials.
///
/// Single-write evidence is the same-bytes comparison against a two-call
/// baseline, plus the absence of any leftover temporary: the combined call
/// produces byte-for-byte the document the two separate calls produce, and does
/// it without a second visit to the file.
#[tokio::test]
async fn save_with_issuer_is_one_write_that_makes_both_observable() {
    let combined_dir = temp_dir();
    let separate_dir = temp_dir();
    let target = key(AS_ALPHA, MCP_ALPHA);

    let combined = store_in(&combined_dir);
    combined
        .save_with_issuer(&target, &creds("token"), MCP_ALPHA, AS_ALPHA)
        .await
        .expect("the combined update works");

    assert!(combined
        .load(&target)
        .await
        .expect("the load works")
        .is_some());
    assert_eq!(
        combined
            .last_issuer(MCP_ALPHA)
            .await
            .expect("the issuer read works")
            .as_deref(),
        Some(AS_ALPHA)
    );

    let separate = store_in(&separate_dir);
    separate
        .save(&target, &creds("token"))
        .await
        .expect("the save works");
    separate
        .record_issuer(MCP_ALPHA, AS_ALPHA)
        .await
        .expect("the issuer record works");

    assert_eq!(
        fs::read(cache_path(&combined_dir)).expect("the combined file is readable"),
        fs::read(cache_path(&separate_dir)).expect("the separate file is readable"),
        "one combined update must produce exactly the two-call document"
    );

    let entries = fs::read_dir(combined_dir.path())
        .expect("the directory is readable")
        .count();
    assert_eq!(entries, 1, "the combined update left something behind");
}

/// The single-write half of the same claim, and the assertion that actually
/// fails if the override is deleted.
///
/// The two-call baseline is measured in the same test rather than assumed, so
/// the counter is proven able to see two writes before one write is claimed.
#[tokio::test]
async fn save_with_issuer_writes_the_file_exactly_once() {
    let (writes, _guard) = count_writes();
    let target = key(AS_ALPHA, MCP_ALPHA);

    let combined_dir = temp_dir();
    store_in(&combined_dir)
        .save_with_issuer(&target, &creds("token"), MCP_ALPHA, AS_ALPHA)
        .await
        .expect("the combined update works");
    assert_eq!(
        writes.load(Ordering::SeqCst),
        1,
        "save_with_issuer must be ONE read-modify-write, not save-then-record"
    );

    writes.store(0, Ordering::SeqCst);
    let separate_dir = temp_dir();
    let separate = store_in(&separate_dir);
    separate
        .save(&target, &creds("token"))
        .await
        .expect("the save works");
    separate
        .record_issuer(MCP_ALPHA, AS_ALPHA)
        .await
        .expect("the issuer record works");
    assert_eq!(
        writes.load(Ordering::SeqCst),
        2,
        "the counter cannot see a second write, so the assertion above proves nothing"
    );
}

/// A mutation that changes nothing must not rewrite the file. Byte-stability is
/// what makes that detectable, and it is why the document uses ordered maps.
#[tokio::test]
async fn a_mutation_that_changes_nothing_does_not_write() {
    let dir = temp_dir();
    let store = store_in(&dir);
    let target = key(AS_ALPHA, MCP_ALPHA);

    store
        .save(&target, &creds("token"))
        .await
        .expect("the first save works");

    let (writes, _guard) = count_writes();
    store
        .save(&target, &creds("token"))
        .await
        .expect("the identical save works");
    store
        .delete(&key(AS_BETA, MCP_BETA))
        .await
        .expect("deleting an absent key works");
    store
        .delete_by_server("https://unknown.example")
        .await
        .expect("an unknown server logout works");

    assert_eq!(
        writes.load(Ordering::SeqCst),
        0,
        "a no-op mutation churned the credential file"
    );
}

// ---------------------------------------------------------------------------
// 5. Migration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_schema_1_file_is_read_by_migrating_it_not_by_failing() {
    let dir = temp_dir();
    let path = cache_path(&dir);
    fs::write(&path, LEGACY_TWO_ENTRIES).expect("the fixture writes");
    let store = FileCredentialStore::new(path);

    let alpha = store
        .load(&key(AS_ALPHA, MCP_ALPHA))
        .await
        .expect("the migrating load works")
        .expect("the alpha entry migrated");
    assert_eq!(alpha.access_token(), "alpha-token");
    assert_eq!(alpha.refresh_token(), Some("alpha-refresh"));
    assert_eq!(alpha.client_id(), "alpha-client");

    let beta = store
        .load(&key(AS_BETA, MCP_BETA))
        .await
        .expect("the migrating load works")
        .expect("the beta entry migrated");
    assert_eq!(beta.access_token(), "beta-token");

    let report = store
        .take_migration_report()
        .await
        .expect("the report read works")
        .expect("a migration happened");
    assert_eq!(report.migrated(), 2);
    assert!(report.dropped().is_empty(), "{:?}", report.dropped());
}

/// A read must stay a read. Rewriting on load would turn `auth token` into a
/// mutation, and would hand the operator the migration as a fait accompli
/// before anything could report on it.
#[tokio::test]
async fn a_migrating_load_does_not_rewrite_the_file_but_the_next_save_does() {
    let dir = temp_dir();
    let path = cache_path(&dir);
    fs::write(&path, LEGACY_TWO_ENTRIES).expect("the fixture writes");
    let store = FileCredentialStore::new(path.clone());
    let alpha = key(AS_ALPHA, MCP_ALPHA);

    store.load(&alpha).await.expect("the migrating load works");
    assert_eq!(
        read_document(&path)["schema_version"],
        Value::from(1),
        "a load rewrote the file"
    );

    store
        .save(&key(AS_ALPHA, "https://mcp-gamma.example"), &creds("gamma"))
        .await
        .expect("the save works");
    assert_eq!(
        read_document(&path)["schema_version"],
        Value::from(CREDENTIAL_SCHEMA_VERSION),
        "the save did not persist the current version"
    );

    assert!(
        store.load(&alpha).await.expect("the load works").is_some(),
        "the rewrite dropped a migrated entry"
    );
}

#[tokio::test]
async fn a_schema_1_entry_with_no_issuer_is_dropped_and_reported() {
    let dir = temp_dir();
    let path = cache_path(&dir);
    fs::write(&path, LEGACY_ONE_UNKEYABLE).expect("the fixture writes");
    let store = FileCredentialStore::new(path);

    let keys = store.list_keys().await.expect("the keys are readable");
    assert_eq!(keys, vec![key(AS_ALPHA, MCP_ALPHA)], "{keys:?}");

    let report = store
        .take_migration_report()
        .await
        .expect("the report read works")
        .expect("a migration happened");
    assert_eq!(report.migrated(), 1);
    assert_eq!(report.dropped().len(), 1, "{:?}", report.dropped());
    assert_eq!(
        report.dropped()[0].server_key(),
        "https://mcp-orphan.example"
    );
    assert!(report.dropped()[0].reason().contains("issuer"));
}

#[tokio::test]
async fn an_unknown_future_schema_version_names_the_observed_and_the_supported() {
    let dir = temp_dir();
    let path = cache_path(&dir);
    fs::write(&path, br#"{"schema_version": 99, "credentials": {}}"#).expect("the fixture writes");

    let message = FileCredentialStore::new(path.clone())
        .load(&key(AS_ALPHA, MCP_ALPHA))
        .await
        .expect_err("an unknown version is a refusal")
        .to_string();

    assert!(
        message.contains("schema_version 99"),
        "the refusal must name the observed version: {message}"
    );
    assert!(
        message.contains(&format!("version {CREDENTIAL_SCHEMA_VERSION}")),
        "the refusal must name the supported version: {message}"
    );
    assert!(
        message.contains(&path.display().to_string()),
        "the refusal must name the path: {message}"
    );
}

// ---------------------------------------------------------------------------
// 6. Administrative operations
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_keys_is_empty_then_reflects_every_stored_credential() {
    let dir = temp_dir();
    let store = store_in(&dir);
    assert!(store
        .list_keys()
        .await
        .expect("the keys are readable")
        .is_empty());

    seed_three(&store).await;
    assert_eq!(
        store
            .list_keys()
            .await
            .expect("the keys are readable")
            .len(),
        3
    );
}

#[tokio::test]
async fn delete_by_server_removes_only_that_server_and_returns_the_count() {
    let dir = temp_dir();
    let store = store_in(&dir);
    seed_three(&store).await;

    let removed = store
        .delete_by_server(MCP_ALPHA)
        .await
        .expect("the delete works");
    assert_eq!(removed, 2);

    let keys = store.list_keys().await.expect("the keys are readable");
    assert_eq!(
        keys,
        vec![CredentialKey::new(AS_BETA, "", MCP_BETA)],
        "{keys:?}"
    );

    assert_eq!(
        store
            .last_issuer(MCP_ALPHA)
            .await
            .expect("the issuer read works"),
        None,
        "a logout must not leave behind which authorization server was visited"
    );
    assert_eq!(
        store
            .last_issuer(MCP_BETA)
            .await
            .expect("the issuer read works")
            .as_deref(),
        Some(AS_BETA),
        "a per-server logout must not forget another server's issuer"
    );
}

#[tokio::test]
async fn delete_by_server_for_an_unknown_server_returns_zero_and_is_not_an_error() {
    let dir = temp_dir();
    let store = store_in(&dir);
    seed_three(&store).await;

    assert_eq!(
        store
            .delete_by_server("https://unknown.example")
            .await
            .expect("an unknown server is not an error"),
        0
    );
    assert_eq!(
        store
            .list_keys()
            .await
            .expect("the keys are readable")
            .len(),
        3
    );
}

#[tokio::test]
async fn clear_all_returns_the_total_and_empties_the_file() {
    let dir = temp_dir();
    let store = store_in(&dir);
    seed_three(&store).await;

    assert_eq!(store.clear_all().await.expect("the wipe works"), 3);
    assert!(store
        .list_keys()
        .await
        .expect("the keys are readable")
        .is_empty());
    assert_eq!(
        store
            .last_issuer(MCP_ALPHA)
            .await
            .expect("the issuer read works"),
        None
    );
    assert_eq!(store.clear_all().await.expect("the second wipe works"), 0);
}

#[tokio::test]
async fn clear_all_on_a_missing_file_returns_zero_and_creates_no_file() {
    let dir = temp_dir();
    let path = cache_path(&dir);
    let store = FileCredentialStore::new(path.clone());

    assert_eq!(store.clear_all().await.expect("the wipe works"), 0);
    assert!(!path.exists(), "an empty wipe created the credential file");
}

#[tokio::test]
async fn deleting_a_key_that_is_not_present_is_not_an_error() {
    let dir = temp_dir();
    let store = store_in(&dir);
    store
        .delete(&key(AS_ALPHA, MCP_ALPHA))
        .await
        .expect("deleting an absent key is not an error");
    assert!(
        !cache_path(&dir).exists(),
        "a no-op delete created the file"
    );
}

#[tokio::test]
async fn delete_removes_the_entry_and_leaves_the_others() {
    let dir = temp_dir();
    let store = store_in(&dir);
    seed_three(&store).await;

    store
        .delete(&CredentialKey::new(AS_ALPHA, "", MCP_ALPHA))
        .await
        .expect("the delete works");
    assert_eq!(
        store
            .list_keys()
            .await
            .expect("the keys are readable")
            .len(),
        2
    );
}

#[tokio::test]
async fn take_migration_report_yields_once_then_none() {
    let dir = temp_dir();
    let path = cache_path(&dir);
    fs::write(&path, LEGACY_TWO_ENTRIES).expect("the fixture writes");
    let store = FileCredentialStore::new(path);

    store.list_keys().await.expect("the migrating read works");
    assert!(store
        .take_migration_report()
        .await
        .expect("the report read works")
        .is_some());
    assert!(store
        .take_migration_report()
        .await
        .expect("the second report read works")
        .is_none());
}

#[tokio::test]
async fn a_current_version_file_reports_no_migration() {
    let dir = temp_dir();
    let store = store_in(&dir);
    store
        .save(&key(AS_ALPHA, MCP_ALPHA), &creds("token"))
        .await
        .expect("the save works");
    store.list_keys().await.expect("the read works");

    assert!(store
        .take_migration_report()
        .await
        .expect("the report read works")
        .is_none());
}

// ---------------------------------------------------------------------------
// The default path
// ---------------------------------------------------------------------------

/// `default_credential_path` is a FREE FUNCTION, so the `dirs` lookup happens
/// at the call site rather than inside a constructor. That split is what keeps
/// `FileCredentialStore::new` I/O-free.
#[test]
fn default_credential_path_names_the_shared_oauth_cache() {
    let path = default_credential_path().expect("a home directory is resolvable");
    assert!(
        path.ends_with(".pmcp/oauth-cache.json"),
        "unexpected default path: {}",
        path.display()
    );
    assert!(
        path.is_absolute(),
        "the default path must be absolute: {}",
        path.display()
    );
}
