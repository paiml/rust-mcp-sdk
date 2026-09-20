//! SEP-2352 credential storage — the three-part key, the record, the document
//! format, the schema 1 → 2 migration and the two traits.
//!
//! This file is deliberately NOT `#![cfg(feature = "oauth")]`. The tier under
//! test is ungated on purpose so a Cloudflare Workers / AWS Lambda platform can
//! implement the store without the `oauth` feature existing at all, and the
//! only way to keep that claim honest is to run this suite under plain
//! `--features full` as well as under `--features full,oauth`.
//!
//! Groups:
//! 1. Key shape — the two collision classes the three-part key closes
//! 2. Record — serde names, builders, redacting `Debug`
//! 3. Snapshot — addressing, enumeration, removal, issuer tracking
//! 4. Format — `schema_version` 2, byte stability, round trip
//! 5. Migration — schema 1 → 2, the drop-and-report rule, hostile bytes
//! 6. Helper — `normalize_server_key`
//! 7. `CredentialStore` — the narrow platform seam and its default impls
//! 8. `CredentialStoreAdmin` — the four `auth` subcommand semantics
//! 9. Properties

use std::collections::BTreeSet;

use pmcp::shared::credential_store::{
    normalize_server_key, DroppedEntry, CREDENTIAL_SCHEMA_VERSION,
};
use pmcp::{
    parse_credential_snapshot, CredentialKey, CredentialSnapshot, CredentialStore,
    CredentialStoreAdmin, InMemoryCredentialStore, MigrationReport, StoredCredentials,
};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const AS_ALPHA: &str = "https://as-alpha.example";
const AS_BETA: &str = "https://as-beta.example";
const MCP_ALPHA: &str = "https://mcp-alpha.example";
const MCP_BETA: &str = "https://mcp-beta.example";

fn creds(token: &str, client: &str) -> StoredCredentials {
    StoredCredentials::new(token, client)
}

/// A schema-1 document in `cargo-pmcp`'s `TokenCacheV1` shape: two entries,
/// both recording an issuer, both for the SAME authorization server and the
/// same (empty) account, differing only in the map key — which is the
/// normalized MCP server URL.
const V1_TWO_SERVERS_ONE_ISSUER: &str = r#"{
  "schema_version": 1,
  "entries": {
    "https://mcp-alpha.example": {
      "access_token": "at-alpha",
      "refresh_token": "rt-alpha",
      "expires_at": 1700000000,
      "scopes": ["mcp:read", "mcp:write"],
      "issuer": "https://as-alpha.example",
      "client_id": "cid-alpha"
    },
    "https://mcp-beta.example": {
      "access_token": "at-beta",
      "scopes": [],
      "issuer": "https://as-alpha.example",
      "client_id": "cid-beta"
    }
  }
}"#;

/// A schema-1 document whose single entry records NO issuer — the case that
/// cannot be re-keyed without guessing which authorization server issued it.
const V1_NO_ISSUER: &str = r#"{
  "schema_version": 1,
  "entries": {
    "https://mcp-alpha.example": {
      "access_token": "at-orphan",
      "scopes": ["mcp:read"],
      "client_id": "cid-orphan"
    }
  }
}"#;

// ---------------------------------------------------------------------------
// 1. Key shape
// ---------------------------------------------------------------------------

#[test]
fn the_three_key_components_round_trip_verbatim() {
    let key = CredentialKey::new(AS_ALPHA, "sub-abc|123", MCP_ALPHA);
    assert_eq!(key.issuer(), AS_ALPHA);
    assert_eq!(key.account(), "sub-abc|123");
    assert_eq!(key.server(), MCP_ALPHA);
}

/// **D-116-R1.** Two MCP servers can share ONE authorization server and ONE
/// user account while holding different registrations, different client IDs
/// and different granted scopes. Under a two-part `(issuer, account)` key they
/// collide: `logout` on one deletes the other's credentials and a migration can
/// overwrite one with the other. RFC 8707's `resource` parameter would have
/// bound the audience and mitigated this; it is deferred, so the key carries
/// the binding. This test asserts the LIVE path; the migration path is asserted
/// separately by `two_schema_1_servers_sharing_one_issuer_stay_independent`.
#[test]
fn two_keys_differing_only_in_server_are_distinct() {
    let alpha = CredentialKey::new(AS_ALPHA, "acct", MCP_ALPHA);
    let beta = CredentialKey::new(AS_ALPHA, "acct", MCP_BETA);

    assert_ne!(alpha, beta, "same issuer + same account must not collide");

    // They hash differently — a HashMap-backed implementor cannot merge them.
    let hashed: BTreeSet<CredentialKey> = [alpha.clone(), beta.clone()].into_iter().collect();
    assert_eq!(hashed.len(), 2);

    // And they address different snapshot entries.
    let mut snapshot = CredentialSnapshot::new();
    snapshot.insert(alpha.clone(), creds("at-alpha", "cid-alpha"));
    snapshot.insert(beta.clone(), creds("at-beta", "cid-beta"));
    assert_eq!(
        snapshot.get(&alpha).map(StoredCredentials::access_token),
        Some("at-alpha")
    );
    assert_eq!(
        snapshot.get(&beta).map(StoredCredentials::access_token),
        Some("at-beta")
    );
}

/// **SEP-2352's first MUST** — "clients MUST NOT reuse client credentials from
/// a different authorization server". It holds by key shape, with no
/// enforcement branch anywhere in the module.
#[test]
fn two_keys_differing_only_in_issuer_are_distinct() {
    let alpha = CredentialKey::new(AS_ALPHA, "acct", MCP_ALPHA);
    let beta = CredentialKey::new(AS_BETA, "acct", MCP_ALPHA);
    assert_ne!(alpha, beta);

    let mut snapshot = CredentialSnapshot::new();
    snapshot.insert(alpha.clone(), creds("at-alpha", "cid-alpha"));
    assert!(
        snapshot.get(&beta).is_none(),
        "a different authorization server must be a cache MISS"
    );
}

#[test]
fn two_keys_differing_only_in_account_are_distinct() {
    let alpha = CredentialKey::new(AS_ALPHA, "sub-one", MCP_ALPHA);
    let beta = CredentialKey::new(AS_ALPHA, "sub-two", MCP_ALPHA);
    assert_ne!(alpha, beta);

    let mut snapshot = CredentialSnapshot::new();
    snapshot.insert(alpha, creds("at-alpha", "cid-alpha"));
    assert!(snapshot.get(&beta).is_none());
}

#[test]
fn an_empty_account_is_the_valid_single_user_cli_case() {
    let key = CredentialKey::new(AS_ALPHA, "", MCP_ALPHA);
    assert_eq!(key.account(), "");

    let mut snapshot = CredentialSnapshot::new();
    snapshot.insert(key.clone(), creds("at", "cid"));
    assert!(snapshot.get(&key).is_some());
}

/// The account scope is caller-supplied and NEVER interpreted by the SDK: a
/// Cognito `sub`, a tenant id, or a composite the caller invented. A value that
/// contains a separator a naive implementation might have split on round-trips
/// byte-identically.
#[test]
fn the_account_scope_is_stored_verbatim_and_never_parsed() {
    for account in [
        "sub-abc|123",
        "tenant/42:user",
        "  leading and trailing  ",
        "issuer=https://as.example",
        "{\"json\":\"like\"}",
    ] {
        let key = CredentialKey::new(AS_ALPHA, account, MCP_ALPHA);
        assert_eq!(key.account(), account, "account must not be interpreted");
    }
}

/// The document format sorts by key, so `Ord` must be total and stable or the
/// byte-stability guarantee has nothing to stand on.
#[test]
fn key_ordering_is_total_and_stable() {
    let mut keys = vec![
        CredentialKey::new(AS_BETA, "b", MCP_ALPHA),
        CredentialKey::new(AS_ALPHA, "b", MCP_BETA),
        CredentialKey::new(AS_ALPHA, "a", MCP_BETA),
        CredentialKey::new(AS_ALPHA, "a", MCP_ALPHA),
    ];
    keys.sort();
    let once = keys.clone();
    keys.sort();
    assert_eq!(once, keys, "sorting a sorted vec must not reorder it");
}

// ---------------------------------------------------------------------------
// 2. Record
// ---------------------------------------------------------------------------

#[test]
fn stored_credentials_serde_uses_the_expected_snake_case_names() {
    let record = StoredCredentials::new("at", "cid")
        .with_refresh_token("rt")
        .with_expires_at(1_700_000_000)
        .with_granted_scopes(["mcp:read", "mcp:write"])
        .with_registered_application_type("native");

    let json = serde_json::to_value(&record).expect("serialize");
    let object = json.as_object().expect("object");

    assert_eq!(
        object.get("access_token").and_then(|v| v.as_str()),
        Some("at")
    );
    assert_eq!(
        object.get("refresh_token").and_then(|v| v.as_str()),
        Some("rt")
    );
    assert_eq!(
        object.get("expires_at").and_then(serde_json::Value::as_u64),
        Some(1_700_000_000)
    );
    assert_eq!(
        object.get("client_id").and_then(|v| v.as_str()),
        Some("cid")
    );
    assert_eq!(
        object
            .get("registered_application_type")
            .and_then(|v| v.as_str()),
        Some("native")
    );
    assert!(object.contains_key("scopes"));

    let back: StoredCredentials = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, record);
}

#[test]
fn an_absent_refresh_token_deserializes_to_none_and_reserializes_as_absent() {
    let record = StoredCredentials::new("at", "cid");
    assert!(record.refresh_token().is_none());

    let json = serde_json::to_value(&record).expect("serialize");
    let object = json.as_object().expect("object");
    assert!(!object.contains_key("refresh_token"));
    assert!(!object.contains_key("registered_application_type"));
    assert!(!object.contains_key("expires_at"));

    let back: StoredCredentials = serde_json::from_value(json).expect("deserialize");
    assert!(back.refresh_token().is_none());
    assert!(back.registered_application_type().is_none());
}

/// This is the GRANTED scope from the token response — 116-12 refreshes with
/// exactly it, so order and content must survive verbatim.
#[test]
fn granted_scopes_round_trip_verbatim_and_in_order() {
    let record =
        StoredCredentials::new("at", "cid").with_granted_scopes(["z:last", "a:first", "m:mid"]);
    assert_eq!(record.granted_scopes(), ["z:last", "a:first", "m:mid"]);

    let json = serde_json::to_string(&record).expect("serialize");
    let back: StoredCredentials = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.granted_scopes(), ["z:last", "a:first", "m:mid"]);
}

/// **T-116-15.** A derived `Debug` would put both bearer tokens into any log
/// line or panic message that formats a record. The sentinels are chosen so
/// they are not substrings of any FIELD NAME — a value of `"token"` would make
/// this assertion fail against a correct implementation, because the rendering
/// necessarily contains `access_token:`.
#[test]
fn debug_on_stored_credentials_redacts_both_tokens() {
    let record = StoredCredentials::new("bearer-s3cr3t-value", "cid-public")
        .with_refresh_token("r3fr3sh-s3cr3t-value");
    let rendered = format!("{record:?}");

    assert!(
        !rendered.contains("bearer-s3cr3t-value"),
        "access token leaked into Debug: {rendered}"
    );
    assert!(
        !rendered.contains("r3fr3sh-s3cr3t-value"),
        "refresh token leaked into Debug: {rendered}"
    );
    // The non-secret parts stay legible so the rendering is still useful.
    assert!(rendered.contains("cid-public"), "{rendered}");
    assert!(rendered.contains("access_token"), "{rendered}");
}

#[test]
fn debug_distinguishes_a_present_refresh_token_from_an_absent_one() {
    let without = format!("{:?}", StoredCredentials::new("at", "cid"));
    let with = format!(
        "{:?}",
        StoredCredentials::new("at", "cid").with_refresh_token("rt")
    );
    assert_ne!(without, with, "presence must remain observable");
}

// ---------------------------------------------------------------------------
// 3. Snapshot
// ---------------------------------------------------------------------------

#[test]
fn insert_then_get_returns_the_same_credentials() {
    let key = CredentialKey::new(AS_ALPHA, "acct", MCP_ALPHA);
    let record = StoredCredentials::new("at", "cid").with_granted_scopes(["mcp:read"]);

    let mut snapshot = CredentialSnapshot::new();
    snapshot.insert(key.clone(), record.clone());
    assert_eq!(snapshot.get(&key), Some(&record));
}

#[test]
fn get_on_a_key_differing_in_any_component_returns_none() {
    let key = CredentialKey::new(AS_ALPHA, "acct", MCP_ALPHA);
    let mut snapshot = CredentialSnapshot::new();
    snapshot.insert(key, creds("at", "cid"));

    for miss in [
        CredentialKey::new(AS_BETA, "acct", MCP_ALPHA),
        CredentialKey::new(AS_ALPHA, "other", MCP_ALPHA),
        CredentialKey::new(AS_ALPHA, "acct", MCP_BETA),
    ] {
        assert!(snapshot.get(&miss).is_none(), "unexpected hit for {miss:?}");
    }
}

#[test]
fn remove_returns_true_then_false() {
    let key = CredentialKey::new(AS_ALPHA, "acct", MCP_ALPHA);
    let mut snapshot = CredentialSnapshot::new();
    snapshot.insert(key.clone(), creds("at", "cid"));

    assert!(snapshot.remove(&key));
    assert!(!snapshot.remove(&key));
    assert!(snapshot.get(&key).is_none());
}

#[test]
fn keys_reflects_insertions_and_removals() {
    let alpha = CredentialKey::new(AS_ALPHA, "acct", MCP_ALPHA);
    let beta = CredentialKey::new(AS_BETA, "acct", MCP_BETA);

    let mut snapshot = CredentialSnapshot::new();
    assert!(snapshot.keys().is_empty());

    snapshot.insert(alpha.clone(), creds("at-alpha", "cid-alpha"));
    snapshot.insert(beta.clone(), creds("at-beta", "cid-beta"));
    let listed: BTreeSet<CredentialKey> = snapshot.keys().into_iter().collect();
    assert_eq!(listed, [alpha.clone(), beta.clone()].into_iter().collect());

    assert!(snapshot.remove(&alpha));
    assert_eq!(snapshot.keys(), vec![beta]);
}

#[test]
fn keys_for_server_selects_exactly_that_server_across_issuers_and_accounts() {
    let mut snapshot = CredentialSnapshot::new();
    let wanted = [
        CredentialKey::new(AS_ALPHA, "one", MCP_ALPHA),
        CredentialKey::new(AS_BETA, "two", MCP_ALPHA),
        CredentialKey::new(AS_ALPHA, "", MCP_ALPHA),
    ];
    for key in wanted.clone() {
        snapshot.insert(key, creds("at", "cid"));
    }
    snapshot.insert(
        CredentialKey::new(AS_ALPHA, "one", MCP_BETA),
        creds("at", "cid"),
    );

    let selected: BTreeSet<CredentialKey> =
        snapshot.keys_for_server(MCP_ALPHA).into_iter().collect();
    assert_eq!(selected, wanted.into_iter().collect());
    assert_eq!(snapshot.keys_for_server("https://nothing.example").len(), 0);
}

#[test]
fn clear_returns_the_number_removed_and_empties_the_snapshot() {
    let mut snapshot = CredentialSnapshot::new();
    snapshot.insert(
        CredentialKey::new(AS_ALPHA, "one", MCP_ALPHA),
        creds("at", "cid"),
    );
    snapshot.insert(
        CredentialKey::new(AS_BETA, "two", MCP_BETA),
        creds("at", "cid"),
    );

    assert_eq!(snapshot.clear(), 2);
    assert!(snapshot.keys().is_empty());
    assert_eq!(snapshot.clear(), 0);
}

#[test]
fn record_issuer_and_last_issuer_round_trip() {
    let mut snapshot = CredentialSnapshot::new();
    assert!(snapshot.last_issuer(MCP_ALPHA).is_none());

    snapshot.record_issuer(MCP_ALPHA, AS_ALPHA);
    assert_eq!(snapshot.last_issuer(MCP_ALPHA), Some(AS_ALPHA));

    // D-18: an authorization-server substitution overwrites the record, which
    // is what makes the change detectable by the caller that reads it first.
    snapshot.record_issuer(MCP_ALPHA, AS_BETA);
    assert_eq!(snapshot.last_issuer(MCP_ALPHA), Some(AS_BETA));
    assert!(snapshot.last_issuer(MCP_BETA).is_none());
}

// ---------------------------------------------------------------------------
// 4. Format
// ---------------------------------------------------------------------------

fn populated_snapshot() -> CredentialSnapshot {
    let mut snapshot = CredentialSnapshot::new();
    snapshot.insert(
        CredentialKey::new(AS_ALPHA, "", MCP_ALPHA),
        StoredCredentials::new("at-alpha", "cid-alpha")
            .with_refresh_token("rt-alpha")
            .with_expires_at(1_700_000_000)
            .with_granted_scopes(["mcp:read"]),
    );
    snapshot.insert(
        CredentialKey::new(AS_BETA, "sub-2", MCP_BETA),
        StoredCredentials::new("at-beta", "cid-beta"),
    );
    snapshot.record_issuer(MCP_ALPHA, AS_ALPHA);
    snapshot
}

#[test]
fn to_bytes_emits_the_current_schema_version() {
    let bytes = populated_snapshot().to_bytes().expect("serialize");
    let text = String::from_utf8(bytes).expect("utf8");
    assert!(
        text.contains("\"schema_version\": 2"),
        "expected schema_version 2 in:\n{text}"
    );
    assert_eq!(CREDENTIAL_SCHEMA_VERSION, 2);
}

/// Byte stability is what makes a `git diff` of a credential file meaningful
/// and what stops an atomic write churning the file on every save.
#[test]
fn to_bytes_is_byte_stable_across_calls() {
    let snapshot = populated_snapshot();
    let first = snapshot.to_bytes().expect("serialize");
    let second = snapshot.to_bytes().expect("serialize");
    assert_eq!(first, second);
}

#[test]
fn parse_round_trips_a_serialized_snapshot_and_reports_a_noop() {
    let original = populated_snapshot();
    let bytes = original.to_bytes().expect("serialize");
    let (parsed, report) = parse_credential_snapshot(&bytes).expect("parse");

    assert_eq!(parsed, original);
    assert!(report.is_noop());
    assert_eq!(report.migrated(), 0);
    assert!(report.dropped().is_empty());
    assert_eq!(parsed.last_issuer(MCP_ALPHA), Some(AS_ALPHA));
}

// ---------------------------------------------------------------------------
// 5. Migration
// ---------------------------------------------------------------------------

#[test]
fn a_schema_1_document_migrates_every_entry_that_records_an_issuer() {
    let (snapshot, report) =
        parse_credential_snapshot(V1_TWO_SERVERS_ONE_ISSUER.as_bytes()).expect("parse");

    assert_eq!(report.migrated(), 2);
    assert!(report.dropped().is_empty());
    assert!(!report.is_noop());

    let alpha = CredentialKey::new(AS_ALPHA, "", MCP_ALPHA);
    let record = snapshot.get(&alpha).expect("alpha migrated");
    assert_eq!(record.access_token(), "at-alpha");
    assert_eq!(record.refresh_token(), Some("rt-alpha"));
    assert_eq!(record.expires_at(), Some(1_700_000_000));
    assert_eq!(record.granted_scopes(), ["mcp:read", "mcp:write"]);
    assert_eq!(record.client_id(), "cid-alpha");
}

/// The schema-1 map key IS the normalized MCP server URL, so widening the key
/// from two components to three is a LOSSLESS migration populated from data the
/// v1 format already recorded — not a guess.
#[test]
fn the_schema_1_map_key_becomes_the_server_component() {
    let (snapshot, _) =
        parse_credential_snapshot(V1_TWO_SERVERS_ONE_ISSUER.as_bytes()).expect("parse");

    let servers: BTreeSet<String> = snapshot
        .keys()
        .iter()
        .map(|key| key.server().to_string())
        .collect();
    assert_eq!(
        servers,
        [MCP_ALPHA.to_string(), MCP_BETA.to_string()]
            .into_iter()
            .collect()
    );
    assert_eq!(snapshot.last_issuer(MCP_ALPHA), Some(AS_ALPHA));
    assert_eq!(snapshot.last_issuer(MCP_BETA), Some(AS_ALPHA));
}

/// **D-116-R1 on the MIGRATION path.** Migration is precisely where a two-part
/// key could have silently overwritten one server's credentials with another's,
/// because both v1 entries carry the same issuer and the same (empty) account.
#[test]
fn two_schema_1_servers_sharing_one_issuer_stay_independent() {
    let (snapshot, report) =
        parse_credential_snapshot(V1_TWO_SERVERS_ONE_ISSUER.as_bytes()).expect("parse");

    assert_eq!(report.migrated(), 2, "neither entry may be overwritten");

    let alpha = CredentialKey::new(AS_ALPHA, "", MCP_ALPHA);
    let beta = CredentialKey::new(AS_ALPHA, "", MCP_BETA);
    assert_eq!(
        snapshot.get(&alpha).map(StoredCredentials::access_token),
        Some("at-alpha")
    );
    assert_eq!(
        snapshot.get(&beta).map(StoredCredentials::access_token),
        Some("at-beta")
    );
    assert_eq!(
        snapshot.get(&alpha).map(StoredCredentials::client_id),
        Some("cid-alpha")
    );
    assert_eq!(
        snapshot.get(&beta).map(StoredCredentials::client_id),
        Some("cid-beta")
    );
}

/// **T-116-16.** An entry with no issuer cannot be re-keyed without GUESSING
/// which authorization server issued it — precisely what SEP-2352 forbids. It
/// is dropped and REPORTED, never assigned to an issuer.
#[test]
fn a_schema_1_entry_without_an_issuer_is_dropped_and_reported() {
    let (snapshot, report) = parse_credential_snapshot(V1_NO_ISSUER.as_bytes()).expect("parse");

    assert_eq!(report.migrated(), 0);
    assert!(
        snapshot.keys().is_empty(),
        "an unkeyable entry must not be reachable under any key"
    );
    assert!(snapshot.last_issuer(MCP_ALPHA).is_none());

    let dropped: &[DroppedEntry] = report.dropped();
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].server_key(), MCP_ALPHA);
    assert!(
        dropped[0].reason().contains("issuer"),
        "the reason must name the missing issuer: {}",
        dropped[0].reason()
    );
}

#[test]
fn an_unknown_future_schema_version_is_an_error_naming_both_versions() {
    let err = parse_credential_snapshot(br#"{"schema_version": 3}"#)
        .expect_err("an unknown version must not parse");
    let message = err.to_string();

    assert!(message.contains('3'), "observed version missing: {message}");
    assert!(
        message.contains('2'),
        "supported version missing: {message}"
    );
    assert!(
        message.to_ascii_lowercase().contains("upgrade"),
        "the refusal must say what to do: {message}"
    );
}

#[test]
fn corrupt_bytes_are_an_error_that_echoes_no_input() {
    let canary = "CANARYSECRETDONOTECHO";
    let hostile = format!("not json at all {canary} }}{{");
    let err = parse_credential_snapshot(hostile.as_bytes()).expect_err("corrupt bytes must fail");
    let message = err.to_string();

    assert!(
        !message.contains(canary),
        "the refusal reproduced peer-controlled input: {message}"
    );
    assert!(!message.is_empty());
}

#[test]
fn empty_input_is_an_error_not_an_empty_snapshot() {
    assert!(
        parse_credential_snapshot(&[]).is_err(),
        "empty input must not be read as an empty store"
    );
}

#[test]
fn a_schema_1_document_with_no_entries_migrates_to_an_empty_snapshot() {
    let (snapshot, report) =
        parse_credential_snapshot(br#"{"schema_version": 1, "entries": {}}"#).expect("parse");
    assert!(snapshot.keys().is_empty());
    assert_eq!(report.migrated(), 0);
    assert!(report.dropped().is_empty());
}

// ---------------------------------------------------------------------------
// 6. Helper
// ---------------------------------------------------------------------------

#[test]
fn normalize_server_key_maps_trailing_slash_and_case_variants_to_one_key() {
    let canonical = normalize_server_key("https://mcp.example").expect("normalize");
    for variant in [
        "https://mcp.example/",
        "https://MCP.Example",
        "https://MCP.EXAMPLE/",
        "https://mcp.example/some/path",
        "https://mcp.example:443/",
    ] {
        assert_eq!(
            normalize_server_key(variant).expect("normalize"),
            canonical,
            "variant {variant} must normalize to one key"
        );
    }
}

#[test]
fn normalize_server_key_keeps_a_non_default_port() {
    assert_eq!(
        normalize_server_key("https://mcp.example:8443/x").expect("normalize"),
        "https://mcp.example:8443"
    );
}

#[test]
fn normalize_server_key_rejects_a_url_with_no_host() {
    assert!(normalize_server_key("not a url").is_err());
    assert!(normalize_server_key("https://").is_err());
}

// ---------------------------------------------------------------------------
// 7. CredentialStore — the narrow platform seam
// ---------------------------------------------------------------------------

#[tokio::test]
async fn save_then_load_returns_the_same_credentials() {
    let store = InMemoryCredentialStore::new();
    let key = CredentialKey::new(AS_ALPHA, "acct", MCP_ALPHA);
    let record = StoredCredentials::new("at", "cid").with_granted_scopes(["mcp:read"]);

    store.save(&key, &record).await.expect("save");
    assert_eq!(store.load(&key).await.expect("load"), Some(record));
}

/// SEP-2352 at the TRAIT level: a server that switches authorization server
/// simply misses the cache, so the client re-registers.
#[tokio::test]
async fn load_with_a_different_issuer_is_a_miss() {
    let store = InMemoryCredentialStore::new();
    store
        .save(
            &CredentialKey::new(AS_ALPHA, "acct", MCP_ALPHA),
            &creds("at", "cid"),
        )
        .await
        .expect("save");

    let other = CredentialKey::new(AS_BETA, "acct", MCP_ALPHA);
    assert_eq!(store.load(&other).await.expect("load"), None);
}

/// **D-116-R1 at the trait level.**
#[tokio::test]
async fn load_with_a_different_server_is_a_miss() {
    let store = InMemoryCredentialStore::new();
    store
        .save(
            &CredentialKey::new(AS_ALPHA, "acct", MCP_ALPHA),
            &creds("at", "cid"),
        )
        .await
        .expect("save");

    let other = CredentialKey::new(AS_ALPHA, "acct", MCP_BETA);
    assert_eq!(store.load(&other).await.expect("load"), None);
}

#[tokio::test]
async fn load_with_a_different_account_is_a_miss() {
    let store = InMemoryCredentialStore::new();
    store
        .save(
            &CredentialKey::new(AS_ALPHA, "acct", MCP_ALPHA),
            &creds("at", "cid"),
        )
        .await
        .expect("save");

    let other = CredentialKey::new(AS_ALPHA, "other", MCP_ALPHA);
    assert_eq!(store.load(&other).await.expect("load"), None);
}

#[tokio::test]
async fn delete_removes_the_entry_and_a_missing_key_is_not_an_error() {
    let store = InMemoryCredentialStore::new();
    let key = CredentialKey::new(AS_ALPHA, "acct", MCP_ALPHA);
    store.save(&key, &creds("at", "cid")).await.expect("save");

    store.delete(&key).await.expect("delete");
    assert_eq!(store.load(&key).await.expect("load"), None);

    // The friendly missing-key no-op `auth logout <url>` depends on.
    store
        .delete(&key)
        .await
        .expect("deleting a missing key is Ok");
}

#[tokio::test]
async fn record_issuer_then_last_issuer_returns_the_recorded_issuer() {
    let store = InMemoryCredentialStore::new();
    assert_eq!(store.last_issuer(MCP_ALPHA).await.expect("read"), None);

    store
        .record_issuer(MCP_ALPHA, AS_ALPHA)
        .await
        .expect("record");
    assert_eq!(
        store.last_issuer(MCP_ALPHA).await.expect("read"),
        Some(AS_ALPHA.to_string())
    );
}

/// Saving credentials and recording the server's last-seen issuer as two
/// separate operations leaves a window in which the store claims one issuer
/// while holding another's credentials. One call closes it.
#[tokio::test]
async fn save_with_issuer_makes_both_observable_in_one_call() {
    let store = InMemoryCredentialStore::new();
    let key = CredentialKey::new(AS_ALPHA, "acct", MCP_ALPHA);
    let record = creds("at", "cid");

    store
        .save_with_issuer(&key, &record, MCP_ALPHA, AS_ALPHA)
        .await
        .expect("save_with_issuer");

    assert_eq!(store.load(&key).await.expect("load"), Some(record));
    assert_eq!(
        store.last_issuer(MCP_ALPHA).await.expect("read"),
        Some(AS_ALPHA.to_string())
    );
}

/// A minimal platform implementor that overrides ONLY `load`/`save`/`delete`
/// must compile and behave: the two D-18 tracking methods degrade to their
/// defaults, and the default `save_with_issuer` still makes `load` observable
/// rather than erroring.
#[tokio::test]
async fn a_minimal_implementor_gets_working_defaults() {
    let store = minimal::MinimalStore::default();
    let key = CredentialKey::new(AS_ALPHA, "acct", MCP_ALPHA);
    let record = creds("at", "cid");

    assert_eq!(store.last_issuer(MCP_ALPHA).await.expect("default"), None);
    store
        .record_issuer(MCP_ALPHA, AS_ALPHA)
        .await
        .expect("default record_issuer is Ok");
    assert_eq!(
        store.last_issuer(MCP_ALPHA).await.expect("default"),
        None,
        "the default record_issuer must not pretend to have stored anything"
    );

    store
        .save_with_issuer(&key, &record, MCP_ALPHA, AS_ALPHA)
        .await
        .expect("the default save_with_issuer degrades to save, it does not error");
    assert_eq!(store.load(&key).await.expect("load"), Some(record));
}

mod minimal {
    use std::collections::BTreeMap;

    use async_trait::async_trait;
    use parking_lot::Mutex;
    use pmcp::{CredentialKey, CredentialStore, Result, StoredCredentials};

    /// The smallest thing that can be a `CredentialStore`: three methods.
    #[derive(Debug, Default)]
    pub struct MinimalStore {
        entries: Mutex<BTreeMap<CredentialKey, StoredCredentials>>,
    }

    #[async_trait]
    impl CredentialStore for MinimalStore {
        async fn load(&self, key: &CredentialKey) -> Result<Option<StoredCredentials>> {
            Ok(self.entries.lock().get(key).cloned())
        }

        async fn save(&self, key: &CredentialKey, credentials: &StoredCredentials) -> Result<()> {
            self.entries.lock().insert(key.clone(), credentials.clone());
            Ok(())
        }

        async fn delete(&self, key: &CredentialKey) -> Result<()> {
            self.entries.lock().remove(key);
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// 8. CredentialStoreAdmin — the four `auth` subcommand semantics
// ---------------------------------------------------------------------------

async fn three_credential_store() -> InMemoryCredentialStore {
    let store = InMemoryCredentialStore::new();
    for (issuer, account, server, token) in [
        (AS_ALPHA, "", MCP_ALPHA, "at-1"),
        (AS_BETA, "sub-2", MCP_ALPHA, "at-2"),
        (AS_ALPHA, "", MCP_BETA, "at-3"),
    ] {
        store
            .save(
                &CredentialKey::new(issuer, account, server),
                &creds(token, "cid"),
            )
            .await
            .expect("save");
    }
    store
}

#[tokio::test]
async fn list_keys_is_empty_then_returns_exactly_the_saved_keys() {
    let store = InMemoryCredentialStore::new();
    assert_eq!(store.list_keys().await.expect("list").len(), 0);

    let store = three_credential_store().await;
    let listed = store.list_keys().await.expect("list");
    assert_eq!(listed.len(), 3);

    let unique: BTreeSet<CredentialKey> = listed.into_iter().collect();
    assert_eq!(
        unique,
        [
            CredentialKey::new(AS_ALPHA, "", MCP_ALPHA),
            CredentialKey::new(AS_BETA, "sub-2", MCP_ALPHA),
            CredentialKey::new(AS_ALPHA, "", MCP_BETA),
        ]
        .into_iter()
        .collect()
    );
}

#[tokio::test]
async fn list_keys_ordering_is_deterministic_across_calls() {
    let store = three_credential_store().await;
    let first = store.list_keys().await.expect("list");
    let second = store.list_keys().await.expect("list");
    assert_eq!(first, second);
}

/// `auth logout <url>` and its count message. Two credentials for the named
/// server go, the third — a DIFFERENT server sharing an authorization server —
/// stays. **T-116-13b.**
#[tokio::test]
async fn delete_by_server_removes_only_that_server_and_returns_the_count() {
    let store = three_credential_store().await;

    assert_eq!(store.delete_by_server(MCP_ALPHA).await.expect("delete"), 2);

    let remaining = store.list_keys().await.expect("list");
    assert_eq!(remaining, vec![CredentialKey::new(AS_ALPHA, "", MCP_BETA)]);
    assert_eq!(
        store
            .load(&CredentialKey::new(AS_ALPHA, "", MCP_BETA))
            .await
            .expect("load")
            .map(|record| record.access_token().to_string()),
        Some("at-3".to_string())
    );
}

/// The friendly missing-key no-op — `auth logout <unknown-url>` prints
/// "nothing to do", it does not fail.
#[tokio::test]
async fn delete_by_server_for_an_unknown_server_returns_zero_and_is_not_an_error() {
    let store = three_credential_store().await;
    assert_eq!(
        store
            .delete_by_server("https://never-seen.example")
            .await
            .expect("delete"),
        0
    );
    assert_eq!(store.list_keys().await.expect("list").len(), 3);
}

/// `auth logout --all` and its count message.
#[tokio::test]
async fn clear_all_returns_the_total_and_empties_the_store() {
    let store = three_credential_store().await;
    assert_eq!(store.clear_all().await.expect("clear"), 3);
    assert_eq!(store.list_keys().await.expect("list").len(), 0);
}

#[tokio::test]
async fn clear_all_on_an_empty_store_returns_zero() {
    let store = InMemoryCredentialStore::new();
    assert_eq!(store.clear_all().await.expect("clear"), 0);
}

#[tokio::test]
async fn take_migration_report_is_none_on_a_store_that_never_migrated() {
    let store = InMemoryCredentialStore::new();
    assert!(store.take_migration_report().await.expect("take").is_none());

    let bytes = populated_snapshot().to_bytes().expect("serialize");
    let seeded = InMemoryCredentialStore::from_bytes(&bytes).expect("seed");
    assert!(
        seeded
            .take_migration_report()
            .await
            .expect("take")
            .is_none(),
        "reading a current-version document is not a migration"
    );
}

/// This is how 116-13 surfaces a dropped login to the operator — **T-116-16b**.
#[tokio::test]
async fn take_migration_report_yields_once_then_none() {
    let store = InMemoryCredentialStore::from_bytes(V1_NO_ISSUER.as_bytes()).expect("seed");

    let report: MigrationReport = store
        .take_migration_report()
        .await
        .expect("take")
        .expect("a migration that dropped an entry must report it");
    assert_eq!(report.migrated(), 0);
    assert_eq!(report.dropped().len(), 1);

    assert!(
        store.take_migration_report().await.expect("take").is_none(),
        "taking the report must clear it"
    );
}

#[tokio::test]
async fn a_seeded_store_serves_the_migrated_credentials() {
    let store =
        InMemoryCredentialStore::from_bytes(V1_TWO_SERVERS_ONE_ISSUER.as_bytes()).expect("seed");

    let record = store
        .load(&CredentialKey::new(AS_ALPHA, "", MCP_BETA))
        .await
        .expect("load")
        .expect("migrated");
    assert_eq!(record.access_token(), "at-beta");
    assert_eq!(record.client_id(), "cid-beta");
    assert_eq!(
        store.last_issuer(MCP_BETA).await.expect("read"),
        Some(AS_ALPHA.to_string())
    );
}

// ---------------------------------------------------------------------------
// 9. Properties
// ---------------------------------------------------------------------------

proptest! {
    /// The parser sits directly on peer- or disk-supplied bytes and is fuzzed
    /// over arbitrary input in 116-08. It must be TOTAL: every byte sequence
    /// yields an `Ok` or an `Err`, never a panic.
    #[test]
    fn parse_credential_snapshot_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        let _ = parse_credential_snapshot(&bytes);
    }

    /// Truncations and single-byte mutations of a VALID document are far more
    /// likely to reach the deserializer's interior than random bytes are.
    #[test]
    fn parse_credential_snapshot_never_panics_on_mutated_documents(
        cut in 0usize..400,
        byte in any::<u8>(),
        at in 0usize..400,
    ) {
        let mut bytes = populated_snapshot().to_bytes().expect("serialize");
        if at < bytes.len() {
            bytes[at] = byte;
        }
        bytes.truncate(cut.min(bytes.len()));
        let _ = parse_credential_snapshot(&bytes);
    }

    /// Ported from `cargo-pmcp`'s `normalize_cache_key` property. Well-formed
    /// DNS hostnames only: the generator deliberately excludes hyphens so it
    /// cannot emit IDNA-reserved labels the normalizer rightly rejects.
    #[test]
    fn normalize_server_key_is_idempotent(
        scheme in prop_oneof![Just("http"), Just("https")],
        host in "[a-z][a-z0-9]{0,10}(\\.[a-z][a-z0-9]{0,10}){0,2}\\.example",
        port_opt in prop::option::of(1025u16..60000),
        path in "/[a-z]{0,10}",
    ) {
        let port_part = port_opt.map(|p| format!(":{p}")).unwrap_or_default();
        let raw = format!("{scheme}://{host}{port_part}{path}");
        let once = normalize_server_key(&raw).expect("normalize raw");
        let twice = normalize_server_key(&once).expect("normalize once");
        prop_assert_eq!(once, twice);
    }

    /// The key never interprets any component: whatever goes in comes out.
    #[test]
    fn every_key_component_round_trips(
        issuer in "[!-~]{0,32}",
        account in "[!-~]{0,32}",
        server in "[!-~]{0,32}",
    ) {
        let key = CredentialKey::new(&issuer, &account, &server);
        prop_assert_eq!(key.issuer(), issuer.as_str());
        prop_assert_eq!(key.account(), account.as_str());
        prop_assert_eq!(key.server(), server.as_str());
    }
}
