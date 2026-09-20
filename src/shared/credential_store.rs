//! Target-agnostic OAuth credential storage: the key, the record, the document
//! format, the schema 1 → 2 migration, the platform seam and its administrative
//! sibling.
//!
//! # Why this tier is ungated
//!
//! A file under the user's home directory is not a viable credential store for
//! ANY hosting target — the home directory is unwritable on AWS Lambda and
//! per-container on Cloudflare Workers and Cloud Run. So credential storage
//! lands behind a trait, and everything a platform needs in order to implement
//! that trait lives here: outside the `oauth` feature and outside any target
//! gate. This module has no `#[cfg]` attribute other than the one over its own
//! unit tests, performs no I/O of any kind, and imports nothing beyond this
//! crate's error type, `serde`, `async_trait`, `parking_lot` and `url`.
//!
//! The practical consequence: a platform that keeps the same JSON document in
//! `DynamoDB` or a KV store gets byte-identical parsing, migration and
//! reporting behaviour to the CLI, because
//! [`parse_credential_snapshot`](crate::shared::credential_store::parse_credential_snapshot)
//! and
//! [`CredentialSnapshot::to_bytes`](crate::shared::credential_store::CredentialSnapshot::to_bytes)
//! are the ONLY places the on-disk shape is known. A gated file implementation
//! reduces to lock, read, parse, mutate, serialize, write.
//!
//! # The key is three-part
//!
//! [`CredentialKey`](crate::shared::credential_store::CredentialKey) is
//! `(issuer, account, server)`. The issuer component is SEP-2352's requirement
//! — "clients MUST NOT reuse client credentials from a different authorization
//! server" holds by construction, with no enforcement branch, because a
//! different authorization server is simply a different key.
//!
//! The `server` component closes a second collision that the two-part form
//! leaves open: two MCP servers can share one authorization server and one user
//! account while holding DIFFERENT registrations, different client IDs and
//! different granted scopes. Under a two-part key they collide — a logout on
//! one deletes the other's credentials, and a migration can overwrite one with
//! the other. RFC 8707's `resource` parameter would have bound the audience and
//! mitigated this; it is deferred by owner decision, so the key carries the
//! binding instead.
//!
//! The `account` component is caller-supplied and NEVER interpreted by this
//! crate: a Cognito `sub`, a tenant id, or empty for the single-user CLI.
//!
//! # Why the public structs have private fields
//!
//! `OAuthConfig`, `DcrRequest` and `OidcDiscoveryMetadata` are all-public-field
//! structs that are not `#[non_exhaustive]`, so adding a field to any of them is
//! a MAJOR semver event. The types here use private fields with constructors and
//! accessors precisely so they stay extensible at minor forever — which is what
//! let `registered_application_type` and `granted_scopes` be added without a
//! semver event, and what lets the next such field be added the same way. Do not
//! "simplify" them to public fields.
//!
//! # Examples
//!
//! ```
//! use pmcp::{CredentialKey, CredentialSnapshot, StoredCredentials};
//!
//! let mut snapshot = CredentialSnapshot::new();
//! let key = CredentialKey::new("https://as.example", "", "https://mcp.example");
//! snapshot.insert(key.clone(), StoredCredentials::new("access-token", "client-id"));
//!
//! // A different authorization server is a cache MISS, by key shape alone.
//! let other = CredentialKey::new("https://evil.example", "", "https://mcp.example");
//! assert!(snapshot.get(&other).is_none());
//! assert!(snapshot.get(&key).is_some());
//! # Ok::<(), pmcp::Error>(())
//! ```

use std::collections::BTreeMap;
use std::fmt;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{Error, Result};

/// The document schema version this build reads and writes.
pub const CREDENTIAL_SCHEMA_VERSION: u32 = 2;

/// The schema version of `cargo-pmcp`'s pre-existing multi-server token cache.
const LEGACY_SCHEMA_VERSION: u32 = 1;

/// What a redacted secret renders as in [`StoredCredentials`]'s `Debug`.
const REDACTED: &str = "<redacted>";

/// Why an entry could not be re-keyed during a schema 1 → 2 migration.
const MISSING_ISSUER_REASON: &str =
    "entry records no issuer; it cannot be re-keyed without guessing which \
     authorization server issued it, so it was dropped rather than misattributed";

/// account → credentials, for one issuer and one server.
type AccountMap = BTreeMap<String, StoredCredentials>;
/// server → [`AccountMap`], for one issuer.
type ServerMap = BTreeMap<String, AccountMap>;
/// issuer → [`ServerMap`] — the whole credential tree.
type IssuerMap = BTreeMap<String, ServerMap>;

// ---------------------------------------------------------------------------
// Key
// ---------------------------------------------------------------------------

/// The address of one stored credential: `(issuer, account, server)`.
///
/// All three components are stored verbatim and compared byte-for-byte. No
/// normalization of any kind is applied here — the `server` component is
/// expected to already be the value [`normalize_server_key`] produced, so that
/// trailing-slash and host-case variants of one MCP server URL do not become
/// two keys.
///
/// # Why three components and not two
///
/// - **issuer** — SEP-2352: credentials obtained from one authorization server
///   MUST NOT be reused with another. Including the issuer makes that true by
///   construction rather than by an enforcement branch somebody can forget.
/// - **server** — two MCP servers can share one authorization server AND one
///   account while holding different registrations, different client IDs and
///   different granted scopes. Without this component they collide: a logout on
///   one deletes the other's credentials, and a schema migration can overwrite
///   one with the other. RFC 8707's `resource` parameter would have bound the
///   audience and mitigated this; it is deferred, so the key carries it.
/// - **account** — caller-supplied and never interpreted: a Cognito `sub`, a
///   tenant id, or the empty string for the single-user CLI.
///
/// # Examples
///
/// ```
/// use pmcp::CredentialKey;
///
/// let key = CredentialKey::new("https://as.example", "sub-abc|123", "https://mcp.example");
/// assert_eq!(key.issuer(), "https://as.example");
/// assert_eq!(key.account(), "sub-abc|123");
/// assert_eq!(key.server(), "https://mcp.example");
///
/// // Two MCP servers sharing one authorization server and one account do NOT collide.
/// let other = CredentialKey::new("https://as.example", "sub-abc|123", "https://other.example");
/// assert_ne!(key, other);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CredentialKey {
    issuer: String,
    account: String,
    server: String,
}

impl CredentialKey {
    /// Build a key from its three components. None of them is validated,
    /// normalized or interpreted.
    pub fn new<I, A, S>(issuer: I, account: A, server: S) -> Self
    where
        I: Into<String>,
        A: Into<String>,
        S: Into<String>,
    {
        Self {
            issuer: issuer.into(),
            account: account.into(),
            server: server.into(),
        }
    }

    /// The authorization server's `issuer` identifier.
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// The caller-supplied account scope. Empty for the single-user CLI.
    pub fn account(&self) -> &str {
        &self.account
    }

    /// The normalized MCP server key. See [`normalize_server_key`].
    pub fn server(&self) -> &str {
        &self.server
    }
}

// ---------------------------------------------------------------------------
// Record
// ---------------------------------------------------------------------------

/// The credentials held for one [`CredentialKey`].
///
/// Field names on the wire are the `snake_case` names `cargo-pmcp`'s existing
/// multi-server cache already uses — `access_token`, `refresh_token`,
/// `expires_at`, `scopes`, `client_id` — plus the new optional
/// `registered_application_type`. That is not cosmetic: the pre-existing file is
/// the migration source AND the surviving path, so a field-name divergence here
/// would silently drop data.
///
/// `Debug` is implemented BY HAND. A derived one would put both bearer tokens
/// into every log line and panic message that formats a record.
///
/// # Examples
///
/// ```
/// use pmcp::StoredCredentials;
///
/// let record = StoredCredentials::new("access-token", "client-id")
///     .with_refresh_token("refresh-token")
///     .with_granted_scopes(["mcp:read", "mcp:write"]);
///
/// assert_eq!(record.granted_scopes(), ["mcp:read", "mcp:write"]);
/// // Neither token survives into the Debug rendering.
/// assert!(!format!("{record:?}").contains("access-token"));
/// ```
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCredentials {
    access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    scopes: Vec<String>,
    client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    registered_application_type: Option<String>,
}

impl StoredCredentials {
    /// Build a record from the two values every successful flow produces.
    pub fn new(access_token: impl Into<String>, client_id: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            refresh_token: None,
            expires_at: None,
            scopes: Vec::new(),
            client_id: client_id.into(),
            registered_application_type: None,
        }
    }

    /// Attach the refresh token, when the authorization server issued one.
    pub fn with_refresh_token(mut self, refresh_token: impl Into<String>) -> Self {
        self.refresh_token = Some(refresh_token.into());
        self
    }

    /// Attach the absolute expiry, in Unix seconds.
    pub fn with_expires_at(mut self, expires_at: u64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Attach the GRANTED scopes from the token response, in order.
    pub fn with_granted_scopes<S, I>(mut self, scopes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.scopes = scopes.into_iter().map(Into::into).collect();
        self
    }

    /// Attach the `application_type` the authorization server actually
    /// registered, which may differ from the one that was requested.
    pub fn with_registered_application_type(mut self, application_type: impl Into<String>) -> Self {
        self.registered_application_type = Some(application_type.into());
        self
    }

    /// The bearer access token. Sensitive — never log it.
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    /// The refresh token, when one was issued. Sensitive — never log it.
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }

    /// Absolute expiry in Unix seconds, when the response carried one.
    pub fn expires_at(&self) -> Option<u64> {
        self.expires_at
    }

    /// The GRANTED scopes, verbatim and in the order the response listed them.
    pub fn granted_scopes(&self) -> &[String] {
        &self.scopes
    }

    /// The effective client id — issued by dynamic registration, or supplied.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// The `application_type` the authorization server registered, if observed.
    pub fn registered_application_type(&self) -> Option<&str> {
        self.registered_application_type.as_deref()
    }
}

impl fmt::Debug for StoredCredentials {
    /// Redacts both bearer tokens while keeping the shape legible. Presence of
    /// a refresh token stays observable, because "can this record refresh?" is
    /// the question a reader is usually asking.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredCredentials")
            .field("access_token", &REDACTED)
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| REDACTED),
            )
            .field("expires_at", &self.expires_at)
            .field("scopes", &self.scopes)
            .field("client_id", &self.client_id)
            .field(
                "registered_application_type",
                &self.registered_application_type,
            )
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Document
// ---------------------------------------------------------------------------

/// The whole credential document, in memory.
///
/// The credential tree is nested issuer → server → account rather than keyed by
/// a concatenated composite string, so no separator has to be invented that an
/// issuer URL could itself contain. Every level is a `BTreeMap`, which is what
/// makes [`CredentialSnapshot::to_bytes`] byte-stable across writes.
///
/// A second, flat map records the last-seen issuer per normalized server key.
/// Issuer-keyed storage makes an authorization-server substitution SAFE but
/// invisible; recording the previous issuer is what lets a caller notice and
/// warn.
///
/// # Examples
///
/// ```
/// use pmcp::{CredentialKey, CredentialSnapshot, StoredCredentials};
///
/// let mut snapshot = CredentialSnapshot::new();
/// let key = CredentialKey::new("https://as.example", "", "https://mcp.example");
/// snapshot.insert(key.clone(), StoredCredentials::new("at", "cid"));
/// snapshot.record_issuer("https://mcp.example", "https://as.example");
///
/// // Serializing the same snapshot twice produces identical bytes.
/// assert_eq!(snapshot.to_bytes()?, snapshot.to_bytes()?);
/// assert_eq!(snapshot.keys(), vec![key]);
/// # Ok::<(), pmcp::Error>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CredentialSnapshot {
    credentials: IssuerMap,
    issuers: BTreeMap<String, String>,
}

impl CredentialSnapshot {
    /// An empty snapshot.
    pub fn new() -> Self {
        Self::default()
    }

    /// The credentials stored under `key`, if any.
    pub fn get(&self, key: &CredentialKey) -> Option<&StoredCredentials> {
        self.credentials
            .get(&key.issuer)?
            .get(&key.server)?
            .get(&key.account)
    }

    /// Store `credentials` under `key`, replacing anything already there.
    pub fn insert(&mut self, key: CredentialKey, credentials: StoredCredentials) {
        self.credentials
            .entry(key.issuer)
            .or_default()
            .entry(key.server)
            .or_default()
            .insert(key.account, credentials);
    }

    /// Remove `key`, returning whether anything was there.
    ///
    /// Emptied inner maps are pruned, so [`CredentialSnapshot::keys`] stays
    /// accurate and the serialized bytes do not accumulate empty objects.
    pub fn remove(&mut self, key: &CredentialKey) -> bool {
        let Some(by_server) = self.credentials.get_mut(&key.issuer) else {
            return false;
        };
        let Some(by_account) = by_server.get_mut(&key.server) else {
            return false;
        };
        let removed = by_account.remove(&key.account).is_some();
        if by_account.is_empty() {
            by_server.remove(&key.server);
        }
        if by_server.is_empty() {
            self.credentials.remove(&key.issuer);
        }
        removed
    }

    /// Every stored key, in a deterministic order.
    pub fn keys(&self) -> Vec<CredentialKey> {
        let mut out = Vec::new();
        for (issuer, by_server) in &self.credentials {
            for (server, by_account) in by_server {
                for account in by_account.keys() {
                    out.push(CredentialKey::new(issuer, account, server));
                }
            }
        }
        out
    }

    /// Every stored key whose `server` component equals `server_key`, across
    /// all issuers and accounts. This is what a per-server logout operates on.
    pub fn keys_for_server(&self, server_key: &str) -> Vec<CredentialKey> {
        // The tree is keyed issuer -> server -> account, so the server is a
        // direct lookup rather than a scan. Building every key in the store and
        // discarding the non-matching ones cost three String allocations per
        // stored credential to find the handful under one server.
        let mut out = Vec::new();
        for (issuer, by_server) in &self.credentials {
            if let Some(by_account) = by_server.get(server_key) {
                for account in by_account.keys() {
                    out.push(CredentialKey::new(issuer, account, server_key));
                }
            }
        }
        out
    }

    /// Remove everything, returning how many credentials were removed.
    ///
    /// The last-seen-issuer records go too: after a full logout the store must
    /// not retain a list of which authorization servers the user visited.
    pub fn clear(&mut self) -> usize {
        let removed = self.credential_count();
        self.credentials.clear();
        self.issuers.clear();
        removed
    }

    /// The issuer last seen for `server_key`, if one was ever recorded.
    pub fn last_issuer(&self, server_key: &str) -> Option<&str> {
        self.issuers.get(server_key).map(String::as_str)
    }

    /// Record the issuer currently in use for `server_key`, replacing any
    /// previous value.
    pub fn record_issuer(&mut self, server_key: &str, issuer: &str) {
        self.issuers
            .insert(server_key.to_owned(), issuer.to_owned());
    }

    /// Serialize to the current document format.
    ///
    /// Byte-stable: serializing the same snapshot twice produces identical
    /// bytes, because every map is ordered. That is what makes a diff of a
    /// credential file meaningful and stops an atomic write churning the file
    /// on every save.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let document = DocumentRef {
            schema_version: CREDENTIAL_SCHEMA_VERSION,
            credentials: &self.credentials,
            issuers: &self.issuers,
        };
        serde_json::to_vec_pretty(&document)
            .map_err(|e| Error::internal(format!("failed to serialize credentials: {e}")))
    }

    /// Forget the last-seen issuer for one server, without touching any other.
    ///
    /// `pub(crate)` rather than private so the gated file store in
    /// `crate::shared::credential_file` gives `delete_by_server` the SAME
    /// semantics as [`InMemoryCredentialStore`] instead of reimplementing them —
    /// a per-server logout must not leave behind a record of which
    /// authorization server the user visited. Deliberately not `pub`: the
    /// operation only makes sense as part of a delete, and exposing it would
    /// invite a caller to desynchronize the two maps.
    pub(crate) fn forget_issuer(&mut self, server_key: &str) {
        self.issuers.remove(server_key);
    }

    /// How many credentials are stored, across every issuer and server.
    fn credential_count(&self) -> usize {
        self.credentials
            .values()
            .map(|by_server| by_server.values().map(BTreeMap::len).sum::<usize>())
            .sum()
    }
}

/// Borrowed serialization view of the current document format.
#[derive(Serialize)]
struct DocumentRef<'a> {
    schema_version: u32,
    credentials: &'a IssuerMap,
    issuers: &'a BTreeMap<String, String>,
}

/// Owned deserialization view of the current document format.
#[derive(Deserialize)]
struct Document {
    #[serde(default)]
    credentials: IssuerMap,
    #[serde(default)]
    issuers: BTreeMap<String, String>,
}

/// Reads nothing but the version, so dispatch happens before the rest of a
/// hostile document is interpreted.
#[derive(Deserialize)]
struct VersionProbe {
    schema_version: u32,
}

/// `cargo-pmcp`'s schema-1 multi-server cache, mirrored for migration only.
#[derive(Deserialize)]
struct LegacyCache {
    #[serde(default)]
    entries: BTreeMap<String, LegacyEntry>,
}

/// One schema-1 entry. The map key that addresses it is the normalized MCP
/// server URL, which is why widening the key is lossless.
#[derive(Deserialize)]
struct LegacyEntry {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_at: Option<u64>,
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(default)]
    issuer: Option<String>,
    #[serde(default)]
    client_id: String,
}

impl LegacyEntry {
    /// Carry every schema-1 field across. `registered_application_type` did not
    /// exist in schema 1, so it starts absent.
    fn into_stored(self) -> StoredCredentials {
        StoredCredentials {
            access_token: self.access_token,
            refresh_token: self.refresh_token,
            expires_at: self.expires_at,
            scopes: self.scopes,
            client_id: self.client_id,
            registered_application_type: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Migration reporting
// ---------------------------------------------------------------------------

/// What a parse did, so a caller can tell an operator about it.
///
/// Deliberately RETURNED rather than logged: the caller decides between a
/// `tracing` warning and a line of CLI output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct MigrationReport {
    migrated: usize,
    dropped: Vec<DroppedEntry>,
}

impl MigrationReport {
    /// How many entries were re-keyed into the current format.
    pub fn migrated(&self) -> usize {
        self.migrated
    }

    /// Entries that could not be carried across, each naming why.
    pub fn dropped(&self) -> &[DroppedEntry] {
        &self.dropped
    }

    /// Whether the parse changed nothing — the current-version case.
    pub fn is_noop(&self) -> bool {
        self.migrated == 0 && self.dropped.is_empty()
    }
}

/// One entry a migration could not carry across.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DroppedEntry {
    server_key: String,
    reason: String,
}

impl DroppedEntry {
    /// The server key the dropped entry was stored under.
    pub fn server_key(&self) -> &str {
        &self.server_key
    }

    /// Why it could not be carried across, in operator-readable prose.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a credential document, migrating it to the current schema if needed.
///
/// This function is TOTAL: it never panics, for any byte sequence. It contains
/// no indexing and no fallible-unwrapping call, and it is fuzzed over arbitrary
/// bytes. Refusals name the rule that was violated and never reproduce any byte
/// of the input, because a credential document is exactly the kind of thing
/// whose contents must not reach a log.
///
/// # Migration
///
/// A schema-1 document (`cargo-pmcp`'s multi-server cache) is re-keyed in
/// memory: each entry that records an issuer becomes
/// `CredentialKey::new(issuer, "", <the schema-1 map key>)`, and the map key
/// also becomes that server's last-seen issuer record. An entry that records NO
/// issuer cannot be re-keyed without guessing which authorization server issued
/// it — precisely what SEP-2352 forbids — so it is dropped and reported.
///
/// # Examples
///
/// ```
/// use pmcp::{parse_credential_snapshot, CredentialKey};
///
/// let legacy = br#"{
///   "schema_version": 1,
///   "entries": {
///     "https://mcp.example": {
///       "access_token": "at",
///       "issuer": "https://as.example",
///       "client_id": "cid"
///     }
///   }
/// }"#;
///
/// let (snapshot, report) = parse_credential_snapshot(legacy)?;
/// assert_eq!(report.migrated(), 1);
/// assert!(report.dropped().is_empty());
///
/// let key = CredentialKey::new("https://as.example", "", "https://mcp.example");
/// assert_eq!(snapshot.get(&key).map(|c| c.client_id()), Some("cid"));
/// # Ok::<(), pmcp::Error>(())
/// ```
pub fn parse_credential_snapshot(bytes: &[u8]) -> Result<(CredentialSnapshot, MigrationReport)> {
    let probe: VersionProbe = serde_json::from_slice(bytes).map_err(|e| malformed_document(&e))?;
    match probe.schema_version {
        CREDENTIAL_SCHEMA_VERSION => parse_current(bytes),
        LEGACY_SCHEMA_VERSION => migrate_legacy(bytes),
        observed => Err(unsupported_schema_version(observed)),
    }
}

/// Read a document already at the current schema version.
fn parse_current(bytes: &[u8]) -> Result<(CredentialSnapshot, MigrationReport)> {
    let document: Document = serde_json::from_slice(bytes).map_err(|e| malformed_document(&e))?;
    // `Document::credentials` is already an `IssuerMap` — the same type and the
    // same nesting the snapshot stores — so it is MOVED rather than walked and
    // re-inserted. Rebuilding it entry-by-entry re-cloned the issuer and server
    // strings once per account, on a path every load/list/mutation runs through.
    let snapshot = CredentialSnapshot {
        credentials: document.credentials,
        issuers: document.issuers,
    };
    Ok((snapshot, MigrationReport::default()))
}

/// Re-key a schema-1 document into the current format.
fn migrate_legacy(bytes: &[u8]) -> Result<(CredentialSnapshot, MigrationReport)> {
    let cache: LegacyCache = serde_json::from_slice(bytes).map_err(|e| malformed_document(&e))?;
    let mut snapshot = CredentialSnapshot::new();
    let mut migrated = 0usize;
    let mut dropped = Vec::new();

    for (server_key, mut entry) in cache.entries {
        let recorded = entry.issuer.take().filter(|value| !value.is_empty());
        let Some(issuer) = recorded else {
            dropped.push(DroppedEntry {
                server_key,
                reason: MISSING_ISSUER_REASON.to_owned(),
            });
            continue;
        };
        snapshot.record_issuer(&server_key, &issuer);
        snapshot.insert(
            CredentialKey::new(issuer, "", server_key),
            entry.into_stored(),
        );
        migrated += 1;
    }

    Ok((snapshot, MigrationReport { migrated, dropped }))
}

/// A refusal that names the classification and the position, and reproduces no
/// byte of the input.
fn malformed_document(err: &serde_json::Error) -> Error {
    Error::validation(format!(
        "credential document is malformed: {:?} error at line {}, column {}",
        err.classify(),
        err.line(),
        err.column()
    ))
}

/// A refusal that names BOTH the observed and the supported version, and says
/// what to do about it.
fn unsupported_schema_version(observed: u32) -> Error {
    Error::validation(format!(
        "credential document schema_version {observed} is not supported by this build, \
         which reads version {CREDENTIAL_SCHEMA_VERSION}; upgrade pmcp to read it"
    ))
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Reduce an MCP server URL to one stable key: `scheme://host[:port]`, with the
/// host lowercased, the path and query dropped, and default ports removed.
///
/// This is the value the `server` component of a [`CredentialKey`] is expected
/// to carry, so that trailing-slash and host-case variants of one MCP server
/// URL do not become two credentials. It is idempotent.
///
/// Refusals do not reproduce the input, which may carry userinfo.
///
/// # Examples
///
/// ```
/// use pmcp::shared::credential_store::normalize_server_key;
///
/// assert_eq!(normalize_server_key("https://MCP.Example/api/")?, "https://mcp.example");
/// assert_eq!(normalize_server_key("https://mcp.example:443")?, "https://mcp.example");
/// assert_eq!(normalize_server_key("https://mcp.example:8443/x")?, "https://mcp.example:8443");
/// # Ok::<(), pmcp::Error>(())
/// ```
pub fn normalize_server_key(server_url: &str) -> Result<String> {
    let parsed = Url::parse(server_url)
        .map_err(|e| Error::validation(format!("invalid MCP server URL ({e})")))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| Error::validation("MCP server URL has no host"))?
        .to_ascii_lowercase();

    let mut key = format!("{}://{}", parsed.scheme(), host);
    if let Some(port) = parsed.port() {
        let is_default = (parsed.scheme() == "https" && port == 443)
            || (parsed.scheme() == "http" && port == 80);
        if !is_default {
            key.push_str(&format!(":{port}"));
        }
    }
    Ok(key)
}

// ---------------------------------------------------------------------------
// The platform seam
// ---------------------------------------------------------------------------

/// The narrow seam an OAuth flow needs from credential storage.
///
/// This is the trait a HOSTING PLATFORM implements — for `DynamoDB`, a KV
/// store, a secrets manager — and the one `OAuthHelper` holds. It is
/// deliberately small: three required methods plus three that default. A store
/// that vends one user's credentials for one server can implement it in a few
/// lines and has no business being asked to enumerate or wipe anything; the
/// administrative operations a command-line tool needs live on
/// [`CredentialStoreAdmin`] instead.
///
/// # What is deliberately NOT here
///
/// Token refresh. A trait that owned refresh would need an HTTP client, which
/// would break both I/O-free construction and this tier's target-cleanliness in
/// one move. Refresh stays with the OAuth helper, which READS this store for the
/// client id and the granted scopes it needs.
///
/// Construction is likewise I/O-free by contract: an implementor takes every
/// value it needs as a constructor parameter and reads no environment, no disk
/// and no network while being built.
///
/// # Examples
///
/// ```
/// use pmcp::{CredentialKey, CredentialStore, InMemoryCredentialStore, StoredCredentials};
///
/// # async fn demo() -> pmcp::Result<()> {
/// let store = InMemoryCredentialStore::new();
/// let key = CredentialKey::new("https://as.example", "", "https://mcp.example");
///
/// store.save(&key, &StoredCredentials::new("at", "cid")).await?;
/// assert!(store.load(&key).await?.is_some());
///
/// store.delete(&key).await?;
/// assert!(store.load(&key).await?.is_none());
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait CredentialStore: Send + Sync + fmt::Debug {
    /// Load the credentials stored under `key`, if any.
    async fn load(&self, key: &CredentialKey) -> Result<Option<StoredCredentials>>;

    /// Store `credentials` under `key`, replacing anything already there.
    async fn save(&self, key: &CredentialKey, credentials: &StoredCredentials) -> Result<()>;

    /// Remove `key`. Removing a key that is not present is NOT an error.
    async fn delete(&self, key: &CredentialKey) -> Result<()>;

    /// Save credentials and record the server's issuer in ONE operation.
    ///
    /// Doing the two separately leaves a window in which a crash or a lost
    /// update makes the store claim one issuer while holding another's
    /// credentials. The DEFAULT implementation here is NOT atomic — it simply
    /// calls [`CredentialStore::save`] and then
    /// [`CredentialStore::record_issuer`]. An implementor whose storage can do
    /// both under one lock or in one transaction SHOULD override it.
    async fn save_with_issuer(
        &self,
        key: &CredentialKey,
        credentials: &StoredCredentials,
        server_key: &str,
        issuer: &str,
    ) -> Result<()> {
        self.save(key, credentials).await?;
        self.record_issuer(server_key, issuer).await
    }

    /// The issuer last seen for `server_key`.
    ///
    /// Defaults to `Ok(None)` so an implementor that does not want last-seen
    /// issuer tracking is not broken by it. A store that returns `None` here
    /// simply never triggers an issuer-change warning.
    async fn last_issuer(&self, _server_key: &str) -> Result<Option<String>> {
        Ok(None)
    }

    /// Record the issuer currently in use for `server_key`.
    ///
    /// Defaults to `Ok(())` — a successful no-op — for the same reason
    /// [`CredentialStore::last_issuer`] defaults to `None`.
    async fn record_issuer(&self, _server_key: &str, _issuer: &str) -> Result<()> {
        Ok(())
    }
}

/// The operations an administrative tool needs, kept OFF the platform seam.
///
/// [`CredentialStore`] is what an OAuth flow needs and what a hosting platform
/// implements. This trait is what a command-line tool needs in order to list,
/// remove by server, wipe with an accurate count, and report on a migration —
/// and a minimal platform store has no business implementing any of it. Giving
/// those methods default bodies on the narrow seam would be worse than omitting
/// them: a default `Ok(0)` is a lie that a tool would then print as a count.
///
/// So: `OAuthHelper` holds a [`CredentialStore`]; an administrative tool holds
/// something that also implements this. A type that implements only
/// [`CredentialStore`] does NOT satisfy this trait, which is the point — the
/// platform seam stays narrow.
///
/// ```compile_fail
/// use pmcp::CredentialStore;
///
/// // The narrow bound cannot reach the administrative operations.
/// async fn wipe<S: CredentialStore>(store: &S) -> pmcp::Result<usize> {
///     store.clear_all().await
/// }
/// ```
#[async_trait]
pub trait CredentialStoreAdmin: CredentialStore {
    /// Every stored key, in a deterministic order.
    async fn list_keys(&self) -> Result<Vec<CredentialKey>>;

    /// Remove every credential whose key names `server_key`, returning how many
    /// were removed. Removing zero is NOT an error.
    async fn delete_by_server(&self, server_key: &str) -> Result<usize>;

    /// Remove everything, returning how many credentials were removed.
    async fn clear_all(&self) -> Result<usize>;

    /// The report from the most recent load that performed a migration, if
    /// there was one. Taking it CLEARS it, so an operator is told once.
    async fn take_migration_report(&self) -> Result<Option<MigrationReport>>;
}

// ---------------------------------------------------------------------------
// In-memory implementation
// ---------------------------------------------------------------------------

/// An in-memory [`CredentialStore`] and [`CredentialStoreAdmin`].
///
/// Everything is delegated to a [`CredentialSnapshot`] behind a lock, so this
/// store and any document-backed store share ONE set of semantics and cannot
/// drift apart.
///
/// # Examples
///
/// ```
/// use pmcp::{CredentialStoreAdmin, InMemoryCredentialStore};
///
/// # async fn demo() -> pmcp::Result<()> {
/// let legacy = br#"{"schema_version": 1, "entries": {}}"#;
/// let store = InMemoryCredentialStore::from_bytes(legacy)?;
/// assert_eq!(store.list_keys().await?.len(), 0);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default)]
pub struct InMemoryCredentialStore {
    snapshot: RwLock<CredentialSnapshot>,
    migration_report: RwLock<Option<MigrationReport>>,
}

impl InMemoryCredentialStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed a store from a serialized credential document, migrating it if
    /// needed. Any resulting migration report is retained until it is taken via
    /// [`CredentialStoreAdmin::take_migration_report`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let (snapshot, report) = parse_credential_snapshot(bytes)?;
        let retained = if report.is_noop() { None } else { Some(report) };
        Ok(Self {
            snapshot: RwLock::new(snapshot),
            migration_report: RwLock::new(retained),
        })
    }
}

#[async_trait]
impl CredentialStore for InMemoryCredentialStore {
    async fn load(&self, key: &CredentialKey) -> Result<Option<StoredCredentials>> {
        Ok(self.snapshot.read().get(key).cloned())
    }

    async fn save(&self, key: &CredentialKey, credentials: &StoredCredentials) -> Result<()> {
        self.snapshot
            .write()
            .insert(key.clone(), credentials.clone());
        Ok(())
    }

    async fn delete(&self, key: &CredentialKey) -> Result<()> {
        self.snapshot.write().remove(key);
        Ok(())
    }

    /// Overridden to be atomic: both mutations happen under one write lock, so
    /// no reader ever observes the credentials without the issuer record.
    async fn save_with_issuer(
        &self,
        key: &CredentialKey,
        credentials: &StoredCredentials,
        server_key: &str,
        issuer: &str,
    ) -> Result<()> {
        let mut snapshot = self.snapshot.write();
        snapshot.insert(key.clone(), credentials.clone());
        snapshot.record_issuer(server_key, issuer);
        Ok(())
    }

    async fn last_issuer(&self, server_key: &str) -> Result<Option<String>> {
        Ok(self
            .snapshot
            .read()
            .last_issuer(server_key)
            .map(str::to_owned))
    }

    async fn record_issuer(&self, server_key: &str, issuer: &str) -> Result<()> {
        self.snapshot.write().record_issuer(server_key, issuer);
        Ok(())
    }
}

#[async_trait]
impl CredentialStoreAdmin for InMemoryCredentialStore {
    async fn list_keys(&self) -> Result<Vec<CredentialKey>> {
        Ok(self.snapshot.read().keys())
    }

    async fn delete_by_server(&self, server_key: &str) -> Result<usize> {
        let mut snapshot = self.snapshot.write();
        let mut removed = 0usize;
        for key in snapshot.keys_for_server(server_key) {
            if snapshot.remove(&key) {
                removed += 1;
            }
        }
        snapshot.forget_issuer(server_key);
        Ok(removed)
    }

    async fn clear_all(&self) -> Result<usize> {
        Ok(self.snapshot.write().clear())
    }

    async fn take_migration_report(&self) -> Result<Option<MigrationReport>> {
        Ok(self.migration_report.write().take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_redaction_marker_is_not_a_field_name_substring() {
        // Guards the redaction test's sentinel discipline: if the marker ever
        // became a field name, an "absence" assertion would pass vacuously.
        for field in [
            "access_token",
            "refresh_token",
            "expires_at",
            "scopes",
            "client_id",
            "registered_application_type",
        ] {
            assert!(!field.contains(REDACTED));
            assert!(!REDACTED.contains(field));
        }
    }

    #[test]
    fn a_legacy_entry_carries_every_field_across() {
        let entry = LegacyEntry {
            access_token: "at".to_owned(),
            refresh_token: Some("rt".to_owned()),
            expires_at: Some(42),
            scopes: vec!["a".to_owned()],
            issuer: Some("https://as.example".to_owned()),
            client_id: "cid".to_owned(),
        };
        let stored = entry.into_stored();
        assert_eq!(stored.access_token(), "at");
        assert_eq!(stored.refresh_token(), Some("rt"));
        assert_eq!(stored.expires_at(), Some(42));
        assert_eq!(stored.granted_scopes(), ["a"]);
        assert_eq!(stored.client_id(), "cid");
        assert!(stored.registered_application_type().is_none());
    }

    #[test]
    fn an_emptied_issuer_map_is_pruned_so_keys_stays_accurate() {
        let key = CredentialKey::new("https://as.example", "", "https://mcp.example");
        let mut snapshot = CredentialSnapshot::new();
        snapshot.insert(key.clone(), StoredCredentials::new("at", "cid"));
        assert!(snapshot.remove(&key));
        assert!(snapshot.credentials.is_empty(), "empty maps must be pruned");
        assert_eq!(snapshot.credential_count(), 0);
    }

    #[test]
    fn forget_issuer_touches_only_the_named_server() {
        let mut snapshot = CredentialSnapshot::new();
        snapshot.record_issuer("https://a.example", "https://as.example");
        snapshot.record_issuer("https://b.example", "https://as.example");
        snapshot.forget_issuer("https://a.example");
        assert!(snapshot.last_issuer("https://a.example").is_none());
        assert!(snapshot.last_issuer("https://b.example").is_some());
    }

    #[test]
    fn an_unsupported_version_refusal_names_both_versions() {
        let message = unsupported_schema_version(7).to_string();
        assert!(message.contains('7'), "{message}");
        assert!(message.contains('2'), "{message}");
    }
}
