//! The DEFAULT on-disk credential store: thin, gated I/O over the ungated
//! format.
//!
//! # What this module deliberately does not know
//!
//! Anything about the credential document's shape. Every field name, every
//! version number and the whole schema 1 → 2 migration live in
//! [`crate::shared::credential_store::parse_credential_snapshot`] and
//! [`crate::shared::credential_store::CredentialSnapshot::to_bytes`], which are
//! ungated, I/O-free and fuzzable. What is left here is lock, read, parse,
//! mutate, serialize, rename. If a `serde` derive or a JSON field name ever
//! appears in this file, the split that makes a platform store and the CLI agree
//! about what a stored credential means has been broken — put the change in the
//! other module instead.
//!
//! # Why this is a separate module rather than a gated half of the other one
//!
//! [`crate::shared::credential_store`] carries no `#[cfg]` attribute other than
//! the one over its own unit tests, which is what makes its wasm32 cleanliness
//! reviewable at a glance and what a CI job now enforces. Everything in THIS
//! module needs a filesystem and the `oauth` feature's `dirs` dependency, so it
//! is gated whole rather than sprinkled through the pure tier.
//!
//! # Concurrency, and the limits of it
//!
//! An atomic rename prevents a TORN file. It prevents nothing about two
//! writers: process A reads the document, process B reads the same document, A
//! writes its version, B writes its version — and A's credential is gone with no
//! error anywhere. So the unit of work here is a SERIALIZED read-modify-write,
//! not merely an atomic write. See
//! [`FileCredentialStore`](crate::shared::credential_file::FileCredentialStore)
//! for what that buys and, just as importantly, what it does not.
//!
//! # Examples
//!
//! ```
//! use pmcp::{
//!     default_credential_path, CredentialKey, CredentialStore, FileCredentialStore,
//!     StoredCredentials,
//! };
//!
//! # async fn demo() -> pmcp::Result<()> {
//! // The `dirs` lookup happens HERE, at the call site — never in the constructor.
//! let store = FileCredentialStore::new(default_credential_path()?);
//! let key = CredentialKey::new("https://as.example", "", "https://mcp.example");
//!
//! store
//!     .save_with_issuer(
//!         &key,
//!         &StoredCredentials::new("access-token", "client-id"),
//!         "https://mcp.example",
//!         "https://as.example",
//!     )
//!     .await?;
//! assert!(store.load(&key).await?.is_some());
//! # Ok(())
//! # }
//! ```

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use parking_lot::Mutex as ReportLock;
use tokio::sync::Mutex as WriterLock;

use crate::error::{Error, Result};
use crate::shared::credential_store::{
    parse_credential_snapshot, CredentialKey, CredentialSnapshot, CredentialStore,
    CredentialStoreAdmin, MigrationReport, StoredCredentials,
};

/// What is appended to the credential file's path to name its advisory lock.
///
/// Documented and exported so an operator who finds a stray file next to their
/// credential file knows what it is and that deleting it is safe once no `pmcp`
/// process is running.
pub const CREDENTIAL_LOCK_SUFFIX: &str = ".lock";

/// How old an advisory lock may get before it is treated as abandoned.
///
/// A process that crashes between taking the lock and releasing it would
/// otherwise wedge every future login. A lock whose modification time is at
/// least this many seconds in the past is broken, with a `tracing` warning
/// naming the lock and its age.
pub const CREDENTIAL_LOCK_STALE_SECS: u64 = 30;

/// The `tracing` target every completed credential-file write is emitted on.
///
/// One `DEBUG` event per atomic write, carrying the path and the byte count and
/// never any file content. Exported so an operator can turn exactly this on
/// (`RUST_LOG=pmcp::credential_file::write=debug`) without raising the log level
/// of anything else, and so a test can COUNT writes — which is the only way to
/// prove that a combined update is one read-modify-write rather than two.
pub const CREDENTIAL_WRITE_EVENT_TARGET: &str = "pmcp::credential_file::write";

/// How long to wait between attempts to take the advisory lock.
const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// How long a single operation will wait for the lock before refusing.
///
/// Deliberately LONGER than [`CREDENTIAL_LOCK_STALE_SECS`]: a lock abandoned by
/// a crashed process is then always broken within one call, instead of leaving
/// the user to retry until the staleness window happens to have elapsed.
const LOCK_WAIT_LIMIT: Duration = Duration::from_secs(45);

/// The file mode a credential file and its lock are created with.
#[cfg(unix)]
const PRIVATE_FILE_MODE: u32 = 0o600;

/// The file mode the credential directory is created with.
#[cfg(unix)]
const PRIVATE_DIR_MODE: u32 = 0o700;

// ---------------------------------------------------------------------------
// The store
// ---------------------------------------------------------------------------

/// The default on-disk [`CredentialStore`] and [`CredentialStoreAdmin`].
///
/// Every mutation is a SERIALIZED read-modify-write: take the lock, read and
/// parse the current document, mutate it, write it atomically, release. No
/// mutating method reads the file independently, which is what stops two
/// writers discarding one another's credentials.
///
/// # Construction performs no I/O
///
/// [`FileCredentialStore::new`] takes the path as a parameter and does nothing
/// else — no home-directory lookup, no directory creation, no read. The
/// `dirs` lookup lives in the free function
/// [`default_credential_path`], which the CALLER invokes. That split is not a
/// style preference: a hosting platform that injects its own store must never
/// have a home directory touched on its behalf, and a constructor that reads the
/// environment makes that impossible to guarantee.
///
/// # What the locking does and does not buy
///
/// Serialization has two layers: a `tokio::sync::Mutex` so two tasks in one
/// process cannot interleave, and an advisory lock file created with
/// `create_new` (atomic on every platform this crate supports, and requiring no
/// additional dependency) so two processes cannot.
///
/// Both limits are stated here rather than implied away:
///
/// * The lock is ADVISORY and therefore cooperative. A different program that
///   writes the credential file directly will still clobber it; nothing in a
///   user-writable file can prevent that.
/// * A lock older than [`CREDENTIAL_LOCK_STALE_SECS`] is broken on the
///   assumption that its owner crashed. A process that stalls for longer than
///   that window can therefore have its lock broken under it.
///
/// Both are acceptable for a per-user credential file on a developer machine.
/// NEITHER would be acceptable for a multi-writer server — which is exactly why
/// [`CredentialStore`] is a trait, and why a hosting platform implements its own
/// store over a database that offers real transactions.
///
/// # Forward-compatibility trap
///
/// An already-installed `cargo-pmcp` at 0.18.0 hard-errors on any document whose
/// schema version is not the single version it knows, so once this store writes
/// [`CREDENTIAL_SCHEMA_VERSION`](crate::shared::credential_store::CREDENTIAL_SCHEMA_VERSION)
/// that older binary fails rather than degrading. The message it already prints
/// says to upgrade `cargo-pmcp`, which is the correct action; nothing in this
/// repository can change a binary that is already installed. Reading a document
/// this store has written therefore requires a `cargo-pmcp` built against this
/// release or later.
///
/// # Examples
///
/// ```
/// use pmcp::{CredentialKey, CredentialStore, FileCredentialStore, StoredCredentials};
///
/// # async fn demo() -> pmcp::Result<()> {
/// let store = FileCredentialStore::new("/tmp/pmcp-demo/oauth-cache.json".into());
/// let key = CredentialKey::new("https://as.example", "", "https://mcp.example");
///
/// // A missing file reads as an absent credential, not as an error.
/// assert!(store.load(&key).await?.is_none());
///
/// store.save(&key, &StoredCredentials::new("access-token", "client-id")).await?;
/// assert!(store.load(&key).await?.is_some());
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct FileCredentialStore {
    /// The credential document's path.
    path: PathBuf,
    /// The advisory lock's path — `path` plus [`CREDENTIAL_LOCK_SUFFIX`].
    lock_path: PathBuf,
    /// In-process serialization, so two tasks sharing this store cannot
    /// interleave a read-modify-write even before the lock file is reached.
    writer: WriterLock<()>,
    /// The report from the most recent read that migrated the document, held
    /// until it is taken.
    migration_report: ReportLock<Option<MigrationReport>>,
}

impl FileCredentialStore {
    /// A store over `path`. Performs NO filesystem and NO environment access.
    ///
    /// Constructing against a path whose directory does not exist succeeds;
    /// only a write creates directories.
    pub fn new(path: PathBuf) -> Self {
        let mut lock_name = path.clone().into_os_string();
        lock_name.push(CREDENTIAL_LOCK_SUFFIX);
        Self {
            lock_path: PathBuf::from(lock_name),
            path,
            writer: WriterLock::new(()),
            migration_report: ReportLock::new(None),
        }
    }

    /// The credential document's path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The advisory lock's path, so a tool can name it when it reports that a
    /// login is waiting on another process.
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    /// Read and parse the current document, treating a missing file as an empty
    /// one and retaining any migration report the parse produced.
    ///
    /// Takes NO lock: a document that was renamed into place is already
    /// consistent, so making readers queue behind a writer would buy nothing and
    /// would let one wedged writer stall every reader.
    fn read_snapshot(&self) -> Result<CredentialSnapshot> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(CredentialSnapshot::new()),
            Err(err) => return Err(self.unreadable(&err)),
        };

        let (snapshot, report) =
            parse_credential_snapshot(&bytes).map_err(|err| self.unusable(&err))?;
        if !report.is_noop() {
            *self.migration_report.lock() = Some(report);
        }
        Ok(snapshot)
    }

    /// The one path every mutation takes: acquire, read, mutate, write, release.
    ///
    /// Writing only when the closure actually changed the document keeps a
    /// no-op operation — deleting an absent key, wiping an empty store — from
    /// creating or churning the file.
    async fn with_snapshot_mut<F, T>(&self, mutate: F) -> Result<T>
    where
        F: FnOnce(&mut CredentialSnapshot) -> T + Send,
        T: Send,
    {
        let _serialized = self.writer.lock().await;
        let _lock = acquire_lock(&self.lock_path).await?;

        let mut snapshot = self.read_snapshot()?;
        let before = snapshot.clone();
        let outcome = mutate(&mut snapshot);
        if snapshot != before {
            write_atomic(&self.path, &snapshot.to_bytes()?)?;
        }
        Ok(outcome)
    }

    /// A refusal for a file that could not be read at all.
    fn unreadable(&self, err: &std::io::Error) -> Error {
        Error::internal(format!(
            "failed to read the credential file {}: {err}",
            self.path.display()
        ))
    }

    /// A refusal for a file that was read but could not be understood.
    ///
    /// Names the path and says what to do about it. Reproduces no file content:
    /// the wrapped refusal is built from `serde`'s CLASSIFICATION and position
    /// rather than its message, precisely because the bytes in question are
    /// bearer and refresh tokens.
    fn unusable(&self, err: &Error) -> Error {
        let path = self.path.display();
        Error::validation(format!(
            "the credential file {path} could not be understood: {err}. \
             No file content is reproduced here; if it cannot be repaired, \
             delete {path} and log in again."
        ))
    }
}

#[async_trait]
impl CredentialStore for FileCredentialStore {
    async fn load(&self, key: &CredentialKey) -> Result<Option<StoredCredentials>> {
        Ok(self.read_snapshot()?.get(key).cloned())
    }

    async fn save(&self, key: &CredentialKey, credentials: &StoredCredentials) -> Result<()> {
        self.with_snapshot_mut(|snapshot| snapshot.insert(key.clone(), credentials.clone()))
            .await
    }

    async fn delete(&self, key: &CredentialKey) -> Result<()> {
        self.with_snapshot_mut(|snapshot| {
            snapshot.remove(key);
        })
        .await
    }

    /// Overridden to be ONE read-modify-write under ONE lock.
    ///
    /// The trait's default calls `save` and then `record_issuer`, which is two
    /// lock acquisitions and a window in which the store names one issuer while
    /// holding another's credentials.
    async fn save_with_issuer(
        &self,
        key: &CredentialKey,
        credentials: &StoredCredentials,
        server_key: &str,
        issuer: &str,
    ) -> Result<()> {
        self.with_snapshot_mut(|snapshot| {
            snapshot.insert(key.clone(), credentials.clone());
            snapshot.record_issuer(server_key, issuer);
        })
        .await
    }

    async fn last_issuer(&self, server_key: &str) -> Result<Option<String>> {
        Ok(self
            .read_snapshot()?
            .last_issuer(server_key)
            .map(str::to_owned))
    }

    async fn record_issuer(&self, server_key: &str, issuer: &str) -> Result<()> {
        self.with_snapshot_mut(|snapshot| snapshot.record_issuer(server_key, issuer))
            .await
    }
}

#[async_trait]
impl CredentialStoreAdmin for FileCredentialStore {
    async fn list_keys(&self) -> Result<Vec<CredentialKey>> {
        Ok(self.read_snapshot()?.keys())
    }

    async fn delete_by_server(&self, server_key: &str) -> Result<usize> {
        self.with_snapshot_mut(|snapshot| {
            let mut removed = 0usize;
            for key in snapshot.keys_for_server(server_key) {
                if snapshot.remove(&key) {
                    removed += 1;
                }
            }
            snapshot.forget_issuer(server_key);
            removed
        })
        .await
    }

    async fn clear_all(&self) -> Result<usize> {
        self.with_snapshot_mut(CredentialSnapshot::clear).await
    }

    async fn take_migration_report(&self) -> Result<Option<MigrationReport>> {
        Ok(self.migration_report.lock().take())
    }
}

// ---------------------------------------------------------------------------
// The default location
// ---------------------------------------------------------------------------

/// Resolve the shared credential file at `~/.pmcp/oauth-cache.json`.
///
/// A FREE FUNCTION rather than a constructor, so the home-directory lookup
/// happens at the CALL SITE. [`FileCredentialStore::new`] then takes the
/// resolved path as a parameter and touches nothing, which is what lets a
/// hosting platform substitute its own store with no environment access
/// anywhere in the construction path.
///
/// The location is shared with `cargo-pmcp`'s multi-server token cache on
/// purpose: an existing login is migrated in place rather than abandoned.
///
/// # Examples
///
/// ```
/// use pmcp::{default_credential_path, FileCredentialStore};
///
/// # fn demo() -> pmcp::Result<()> {
/// let path = default_credential_path()?;
/// assert!(path.ends_with(".pmcp/oauth-cache.json"));
///
/// let store = FileCredentialStore::new(path);
/// # let _ = store;
/// # Ok(())
/// # }
/// ```
pub fn default_credential_path() -> Result<PathBuf> {
    let mut path = dirs::home_dir().ok_or_else(|| {
        Error::internal(
            "could not determine the current user's home directory; \
             pass an explicit path to FileCredentialStore::new instead",
        )
    })?;
    path.push(".pmcp");
    path.push("oauth-cache.json");
    Ok(path)
}

// ---------------------------------------------------------------------------
// Atomic writing
// ---------------------------------------------------------------------------

/// Write `bytes` to `path` atomically, with the permissions a credential file
/// needs.
///
/// Ported from `cargo-pmcp`'s proven sequence: create the parent, restrict the
/// parent, write a SAME-DIRECTORY temporary, restrict the temporary, then
/// rename it into place. Same-directory matters — a rename is only atomic
/// within one filesystem. The temporary is removed if anything fails on the way,
/// so a failed write leaves no readable fragment behind.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        Error::validation(format!(
            "the credential path has no parent directory: {}",
            path.display()
        ))
    })?;
    create_private_dir(parent)?;

    let temporary = temporary_sibling(path)?;
    let _cleanup = RemoveOnDrop::new(temporary.clone());

    let mut file = create_private_file(&temporary)?;
    file.write_all(bytes)
        .map_err(|err| io_failure("write", &temporary, &err))?;
    file.flush()
        .map_err(|err| io_failure("flush", &temporary, &err))?;
    file.sync_all()
        .map_err(|err| io_failure("synchronize", &temporary, &err))?;
    restrict_file(&file, &temporary)?;
    drop(file);

    fs::rename(&temporary, path)
        .map_err(|err| io_failure("atomically rename", &temporary, &err))?;

    tracing::debug!(
        target: CREDENTIAL_WRITE_EVENT_TARGET,
        path = %path.display(),
        bytes = bytes.len(),
        "wrote the credential file atomically"
    );
    Ok(())
}

/// A unique name beside `path`, in the SAME directory so the rename is atomic.
fn temporary_sibling(path: &Path) -> Result<PathBuf> {
    let name = path.file_name().ok_or_else(|| {
        Error::validation(format!(
            "the credential path names no file: {}",
            path.display()
        ))
    })?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());

    let mut candidate = OsString::from(name);
    candidate.push(format!(".{}.{nanos}.tmp", std::process::id()));
    Ok(path.with_file_name(candidate))
}

/// Create `dir` and every missing ancestor, then restrict it to its owner.
///
/// Restricting is best effort, exactly as in `cargo-pmcp`: a pre-existing
/// directory whose mode the user cannot change must not stop a login.
fn create_private_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).map_err(|err| io_failure("create the directory", dir, &err))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        drop(fs::set_permissions(
            dir,
            fs::Permissions::from_mode(PRIVATE_DIR_MODE),
        ));
    }
    Ok(())
}

/// Create `path`, failing if it already exists, owner-readable only.
///
/// `create_new` is the primitive both the temporary file and the advisory lock
/// rest on: it is atomic on every platform this crate supports and needs no
/// additional dependency.
fn open_exclusive(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(PRIVATE_FILE_MODE);
    }
    options.open(path)
}

/// [`open_exclusive`] with this crate's error type.
fn create_private_file(path: &Path) -> Result<File> {
    open_exclusive(path).map_err(|err| io_failure("create", path, &err))
}

/// Restrict an open file to its owner, so a generous umask cannot widen it.
#[cfg(unix)]
fn restrict_file(file: &File, path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))
        .map_err(|err| io_failure("restrict the permissions of", path, &err))
}

/// No-op where the platform has no unix permission bits.
#[cfg(not(unix))]
fn restrict_file(_file: &File, _path: &Path) -> Result<()> {
    Ok(())
}

/// A failure that names the action and the path, and never the file's content.
fn io_failure(action: &str, path: &Path, err: &std::io::Error) -> Error {
    Error::internal(format!("failed to {action} {}: {err}", path.display()))
}

// ---------------------------------------------------------------------------
// The advisory lock
// ---------------------------------------------------------------------------

/// Removes a path when it goes out of scope.
///
/// Used for BOTH the advisory lock and the write temporary, so a `?` on the way
/// out of either cannot leak a file. Failing to remove is not actionable: for
/// the temporary it means the rename already consumed it, and for the lock it
/// means another process had already broken a lock this one let go stale.
#[derive(Debug)]
struct RemoveOnDrop {
    path: PathBuf,
}

impl RemoveOnDrop {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        drop(fs::remove_file(&self.path));
    }
}

/// Take the advisory lock, waiting for another holder and breaking an abandoned
/// one.
///
/// Creates the parent directory first, because the lock has to live somewhere
/// even when the credential file does not exist yet — which is why a mutation
/// on a fresh machine creates `~/.pmcp` while a read never does.
async fn acquire_lock(lock_path: &Path) -> Result<RemoveOnDrop> {
    let parent = lock_path.parent().ok_or_else(|| {
        Error::validation(format!(
            "the credential lock path has no parent directory: {}",
            lock_path.display()
        ))
    })?;
    create_private_dir(parent)?;

    let deadline = Instant::now() + LOCK_WAIT_LIMIT;
    loop {
        match open_exclusive(lock_path) {
            Ok(file) => {
                drop(file);
                return Ok(RemoveOnDrop::new(lock_path.to_path_buf()));
            },
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {},
            Err(err) => return Err(io_failure("create the lock file", lock_path, &err)),
        }

        if break_stale_lock(lock_path) {
            continue;
        }
        if Instant::now() >= deadline {
            return Err(Error::internal(format!(
                "gave up waiting {} seconds for the credential lock {}; \
                 another process is holding it. If no such process is running, \
                 the file is safe to delete.",
                LOCK_WAIT_LIMIT.as_secs(),
                lock_path.display()
            )));
        }
        tokio::time::sleep(LOCK_POLL_INTERVAL).await;
    }
}

/// Break a lock whose owner is presumed to have crashed.
///
/// Returns whether the lock was removed. Every failure to inspect it is treated
/// as "not stale", so an unreadable lock is waited on rather than stolen.
fn break_stale_lock(lock_path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(lock_path) else {
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    let Ok(age) = SystemTime::now().duration_since(modified) else {
        return false;
    };
    if age.as_secs() < CREDENTIAL_LOCK_STALE_SECS {
        return false;
    }

    tracing::warn!(
        lock = %lock_path.display(),
        age_secs = age.as_secs(),
        stale_after_secs = CREDENTIAL_LOCK_STALE_SECS,
        "breaking an abandoned credential lock; the process that took it did not release it"
    );
    fs::remove_file(lock_path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_path_is_the_credential_path_plus_the_documented_suffix() {
        let store = FileCredentialStore::new(PathBuf::from("/x/oauth-cache.json"));
        assert_eq!(store.path(), Path::new("/x/oauth-cache.json"));
        assert_eq!(
            store.lock_path(),
            Path::new("/x/oauth-cache.json.lock"),
            "an operator who finds the file must be able to derive it from the suffix"
        );
    }

    #[test]
    fn the_wait_limit_exceeds_the_staleness_window() {
        // Otherwise a lock abandoned by a crashed process could never be broken
        // within a single call: the caller would time out first, every time.
        assert!(
            LOCK_WAIT_LIMIT.as_secs() > CREDENTIAL_LOCK_STALE_SECS,
            "wait {} must exceed staleness {}",
            LOCK_WAIT_LIMIT.as_secs(),
            CREDENTIAL_LOCK_STALE_SECS
        );
    }

    #[test]
    fn a_temporary_sibling_stays_in_the_same_directory() {
        let path = Path::new("/x/y/oauth-cache.json");
        let temporary = temporary_sibling(path).expect("the path names a file");
        assert_eq!(
            temporary.parent(),
            path.parent(),
            "a rename is only atomic within one filesystem"
        );
        assert_ne!(temporary, path);
        assert!(
            temporary.to_string_lossy().ends_with(".tmp"),
            "{}",
            temporary.display()
        );
    }

    #[test]
    fn a_path_that_names_no_file_is_refused_rather_than_guessed_at() {
        let message = temporary_sibling(Path::new("/"))
            .expect_err("a root path names no file")
            .to_string();
        assert!(message.contains("names no file"), "{message}");
    }

    #[test]
    fn a_missing_lock_is_not_stale() {
        assert!(!break_stale_lock(Path::new(
            "/nonexistent/pmcp/oauth-cache.json.lock"
        )));
    }
}
