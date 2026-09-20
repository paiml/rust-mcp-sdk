//! End-to-end tests for the `cargo pmcp auth` subcommand group + cache fallback.
//!
//! # What changed in Phase 116 Plan 13
//!
//! `cargo-pmcp` no longer carries its own credential-cache implementation. The
//! format, the migration and the storage all live in the SDK
//! (`pmcp::shared::credential_store` + `pmcp::shared::credential_file`), and the
//! five `auth` subcommands are thin wrappers over `CredentialStore` /
//! `CredentialStoreAdmin`. These tests therefore seed the SAME document a real
//! login writes — through `FileCredentialStore` — rather than through a
//! crate-local writer that could drift from it.
//!
//! Covers:
//! - URL normalization edge cases and the idempotence property, through the seam
//! - a credential round-trip through the shared on-disk store
//! - all FOUR `auth logout` semantics, asserted against the CURRENT message text
//! - the schema 1 -> 2 migration: keyable entries survive, unkeyable ones are
//!   dropped and BOTH counts are reported, and two servers sharing one issuer
//!   stay independently addressable
//! - D-116-R1: `auth logout <A>` leaves `auth token <B>` working when A and B
//!   share one authorization server and one account
//! - `auth token` stdout discipline (stdout is EXACTLY the token)
//! - precedence: explicit `--api-key` wins over cached OAuth credentials
//!
//! # Every test runs against a temporary `HOME`
//!
//! The document under test is real credential storage. Every case below points
//! `HOME` at a `tempfile::tempdir()`, so no case can read, rewrite or delete the
//! developer's own `~/.pmcp/oauth-cache.json`.

use cargo_pmcp::test_support::cache::{
    default_multi_cache_path, is_near_expiry, normalize_server_key, CLI_ACCOUNT_SCOPE,
    REFRESH_WINDOW_SECS,
};
use pmcp::shared::credential_store::{CredentialKey, CredentialStore, StoredCredentials};
use pmcp::FileCredentialStore;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// Seconds since the Unix epoch.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The credential document inside a temporary `HOME`.
fn cache_path_in(home: &Path) -> PathBuf {
    home.join(".pmcp").join("oauth-cache.json")
}

/// Seed one credential through the SAME store a real login writes through.
///
/// Using `FileCredentialStore` rather than a hand-rolled writer is deliberate:
/// a fixture that serializes the document itself can drift from the production
/// format and keep passing while the CLI is broken.
async fn seed(home: &Path, issuer: &str, server: &str, access_token: &str) {
    let store = FileCredentialStore::new(cache_path_in(home));
    let key = CredentialKey::new(issuer, CLI_ACCOUNT_SCOPE, server);
    let credentials = StoredCredentials::new(access_token, "cid")
        .with_refresh_token("rt")
        .with_expires_at(now_secs() + 3600)
        .with_granted_scopes(["openid".to_string()]);
    store
        .save_with_issuer(&key, &credentials, server, issuer)
        .await
        .expect("seed the credential store");
}

/// Seed one credential whose access token expired an hour ago.
///
/// Deliberately carries NO refresh token: the renewal the SDK attempts cannot
/// succeed, which is what makes the "never print the stale token" assertion
/// meaningful rather than incidental.
async fn seed_expired(home: &Path, issuer: &str, server: &str, access_token: &str) {
    let store = FileCredentialStore::new(cache_path_in(home));
    let key = CredentialKey::new(issuer, CLI_ACCOUNT_SCOPE, server);
    let credentials = StoredCredentials::new(access_token, "cid")
        .with_expires_at(now_secs().saturating_sub(3600));
    store
        .save_with_issuer(&key, &credentials, server, issuer)
        .await
        .expect("seed the credential store");
}

/// A literal document in `cargo-pmcp` 0.18's ORIGINAL on-disk shape, carrying
/// two issuer-bearing entries.
///
/// One literal shared by every migration case below, so the fixture a
/// destruction test protects and the fixture a survival test reads cannot drift
/// apart.
const LEGACY_TWO_ENTRY_DOCUMENT: &str = r#"{
  "schema_version": 1,
  "entries": {
    "https://a.example": {
      "access_token": "LEGACY-TOKEN-A",
      "refresh_token": "rt-a",
      "scopes": ["openid"],
      "issuer": "https://as-a.example",
      "client_id": "cid-a"
    },
    "https://b.example": {
      "access_token": "LEGACY-TOKEN-B",
      "scopes": ["openid", "profile"],
      "issuer": "https://as-b.example",
      "client_id": "cid-b"
    }
  }
}"#;

/// Write a literal document in `cargo-pmcp` 0.18's ORIGINAL on-disk shape.
///
/// A literal fixture, not a re-serialization: the point of the migration test is
/// that a file written by an already-installed binary is read, and a fixture
/// produced by today's code could not prove that.
fn seed_legacy_document(home: &Path, body: &str) {
    let path = cache_path_in(home);
    std::fs::create_dir_all(path.parent().expect("cache path has a parent")).unwrap();
    std::fs::write(&path, body).unwrap();
}

/// Run `cargo-pmcp <args>` with `HOME` pointed at `home`.
fn run_cli(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-pmcp"))
        .args(args)
        .env("HOME", home)
        .output()
        .expect("run cargo-pmcp binary")
}

/// `(stdout, stderr)` as owned strings.
fn streams(output: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

// ---------------------------------------------------------------------------
// Group A — URL normalization through the seam
// ---------------------------------------------------------------------------

#[test]
fn normalize_covers_url_edge_cases() {
    // IDN / mixed case / trailing slash / default port / custom port
    assert_eq!(
        normalize_server_key("HTTPS://API.Example.Com/").unwrap(),
        "https://api.example.com"
    );
    assert_eq!(
        normalize_server_key("https://api.example.com:443").unwrap(),
        "https://api.example.com"
    );
    assert_eq!(
        normalize_server_key("http://api.example.com:8080/x/y/z").unwrap(),
        "http://api.example.com:8080"
    );
}

/// The idempotence property the crate-local `normalize_cache_key` carried, now
/// asserted against the SDK function that replaced it. It is the only thing
/// stopping `https://x/` and `https://x` becoming two separate logins.
#[test]
fn normalize_is_idempotent_and_folds_slash_and_case_variants() {
    for raw in [
        "https://mcp.example.com",
        "https://mcp.example.com/",
        "HTTPS://MCP.Example.Com/v1/api",
        "https://mcp.example.com:443/",
    ] {
        let once = normalize_server_key(raw).unwrap();
        let twice = normalize_server_key(&once).unwrap();
        assert_eq!(once, twice, "not idempotent for {raw}");
        assert_eq!(once, "https://mcp.example.com", "wrong fold for {raw}");
    }
}

#[test]
fn normalize_errors_on_an_invalid_url() {
    assert!(normalize_server_key("not a url").is_err());
}

// ---------------------------------------------------------------------------
// Group B — the shared on-disk store
// ---------------------------------------------------------------------------

#[tokio::test]
async fn store_roundtrip_through_the_shared_credential_file() {
    let dir = tempfile::tempdir().unwrap();
    seed(
        dir.path(),
        "https://issuer.example",
        "https://mockito.example",
        "integration-test-token",
    )
    .await;

    let store = FileCredentialStore::new(cache_path_in(dir.path()));
    let key = CredentialKey::new(
        "https://issuer.example",
        CLI_ACCOUNT_SCOPE,
        "https://mockito.example",
    );
    let back = store.load(&key).await.unwrap().expect("credential present");
    assert_eq!(back.access_token(), "integration-test-token");
    assert_eq!(
        store
            .last_issuer("https://mockito.example")
            .await
            .unwrap()
            .as_deref(),
        Some("https://issuer.example")
    );
}

#[test]
fn is_near_expiry_window_is_60s() {
    let now = now_secs();
    let near = StoredCredentials::new("at", "c").with_expires_at(now + 30);
    assert!(is_near_expiry(&near, REFRESH_WINDOW_SECS));

    let far = StoredCredentials::new("at", "c").with_expires_at(now + 3600);
    assert!(!is_near_expiry(&far, REFRESH_WINDOW_SECS));

    // No expiry recorded is treated as long-lived, exactly as before the port.
    let unknown = StoredCredentials::new("at", "c");
    assert!(!is_near_expiry(&unknown, REFRESH_WINDOW_SECS));
}

#[test]
fn default_multi_cache_path_ends_in_oauth_cache_json() {
    let p = default_multi_cache_path();
    let s = p.to_string_lossy().to_string();
    assert!(s.ends_with(".pmcp/oauth-cache.json") || s.ends_with(".pmcp\\oauth-cache.json"));
}

// ---------------------------------------------------------------------------
// Group C — the FOUR `auth logout` semantics, against the CURRENT message text
//
// These four are load-bearing: `logout` is the only subcommand that destroys
// credentials, so a wording or count change must be caught rather than reviewed.
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn logout_no_args_errors_via_cli() {
    let temp = tempfile::tempdir().unwrap();
    let output = run_cli(temp.path(), &["auth", "logout"]);
    assert!(!output.status.success(), "expected non-zero exit");
    let (_, stderr) = streams(&output);
    assert!(
        stderr.contains("specify a server URL or --all"),
        "unexpected stderr: {stderr}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn logout_all_clears_every_entry_and_reports_the_count_via_cli() {
    let temp = tempfile::tempdir().unwrap();
    seed(
        temp.path(),
        "https://issuer.example",
        "https://a.example",
        "token-a",
    )
    .await;
    seed(
        temp.path(),
        "https://issuer.example",
        "https://b.example",
        "token-b",
    )
    .await;

    let output = run_cli(temp.path(), &["auth", "logout", "--all"]);
    let (stdout, stderr) = streams(&output);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        stdout.contains("Logged out of 2 cached server(s)."),
        "unexpected stdout: {stdout}"
    );

    let after = run_cli(temp.path(), &["auth", "status"]);
    let (status_out, _) = streams(&after);
    assert!(
        status_out.contains("No cached credentials"),
        "store not emptied: {status_out}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn logout_by_url_removes_exactly_that_server_via_cli() {
    let temp = tempfile::tempdir().unwrap();
    seed(
        temp.path(),
        "https://issuer.example",
        "https://a.example",
        "token-a",
    )
    .await;

    let output = run_cli(temp.path(), &["auth", "logout", "https://a.example/"]);
    let (stdout, stderr) = streams(&output);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        stdout.contains("Logged out of https://a.example."),
        "unexpected stdout: {stdout}"
    );
}

#[cfg(unix)]
#[test]
fn logout_for_an_unknown_server_is_a_friendly_no_op_via_cli() {
    let temp = tempfile::tempdir().unwrap();
    let output = run_cli(
        temp.path(),
        &["auth", "logout", "https://never-seen.example"],
    );
    let (stdout, stderr) = streams(&output);
    assert!(
        output.status.success(),
        "a missing key must NOT be an error; stderr: {stderr}"
    );
    assert!(
        stdout.contains("No cached credentials for https://never-seen.example (nothing to do)."),
        "unexpected stdout: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// Group D — the schema 1 -> 2 migration
// ---------------------------------------------------------------------------

/// A literal document in the ORIGINAL shape, carrying two issuer-bearing
/// entries. Both must be reachable afterwards, keyed by the original map key as
/// the SERVER component.
#[cfg(unix)]
#[test]
fn a_previous_format_file_with_two_entries_migrates_and_both_stay_reachable() {
    let temp = tempfile::tempdir().unwrap();
    seed_legacy_document(temp.path(), LEGACY_TWO_ENTRY_DOCUMENT);

    let a = run_cli(temp.path(), &["auth", "token", "https://a.example"]);
    let (a_out, a_err) = streams(&a);
    assert!(a.status.success(), "stderr: {a_err}");
    assert_eq!(a_out.trim_end(), "LEGACY-TOKEN-A");
    assert!(
        a_err.contains("Migrated 2"),
        "the migrated count must be reported: {a_err}"
    );

    let b = run_cli(temp.path(), &["auth", "token", "https://b.example"]);
    let (b_out, b_err) = streams(&b);
    assert!(b.status.success(), "stderr: {b_err}");
    assert_eq!(b_out.trim_end(), "LEGACY-TOKEN-B");

    let status = run_cli(temp.path(), &["auth", "status"]);
    let (status_out, _) = streams(&status);
    for needle in [
        "https://a.example",
        "https://b.example",
        "https://as-a.example",
        "https://as-b.example",
    ] {
        assert!(
            status_out.contains(needle),
            "missing {needle}: {status_out}"
        );
    }
}

/// An entry with NO issuer cannot be re-keyed without guessing which
/// authorization server issued it, which is exactly what SEP-2352 forbids. It
/// is dropped, the drop names the server URL and the required re-login, and
/// BOTH counts are reported.
#[cfg(unix)]
#[test]
fn a_previous_format_entry_with_no_issuer_is_dropped_and_both_counts_are_reported() {
    let temp = tempfile::tempdir().unwrap();
    seed_legacy_document(
        temp.path(),
        r#"{
  "schema_version": 1,
  "entries": {
    "https://keyable.example": {
      "access_token": "KEEP-ME",
      "issuer": "https://as.example",
      "client_id": "cid"
    },
    "https://unkeyable.example": {
      "access_token": "CANNOT-BE-KEYED",
      "client_id": "cid"
    }
  }
}"#,
    );

    let output = run_cli(temp.path(), &["auth", "status"]);
    let (stdout, stderr) = streams(&output);
    assert!(output.status.success(), "stderr: {stderr}");

    assert!(
        stderr.contains("Migrated 1"),
        "the migrated count must be reported: {stderr}"
    );
    assert!(
        stderr.contains("Dropped 1"),
        "the dropped count must be reported: {stderr}"
    );
    assert!(
        stderr.contains("https://unkeyable.example"),
        "the drop must name the server URL: {stderr}"
    );
    assert!(
        stderr.contains("cargo pmcp auth login https://unkeyable.example"),
        "the drop must name the required re-login: {stderr}"
    );

    assert!(
        stdout.contains("https://keyable.example"),
        "the keyable entry must survive: {stdout}"
    );
    assert!(
        !stdout.contains("https://unkeyable.example"),
        "the unkeyable entry must NOT be silently re-keyed: {stdout}"
    );
    assert!(
        !stdout.contains("CANNOT-BE-KEYED") && !stderr.contains("CANNOT-BE-KEYED"),
        "no token may be printed by status"
    );
}

/// Two servers that shared ONE authorization server in the previous format both
/// survive and stay independently addressable — the case a two-part
/// `(issuer, account)` key would have collapsed during migration.
#[cfg(unix)]
#[test]
fn two_previous_format_servers_sharing_one_issuer_stay_independently_addressable() {
    let temp = tempfile::tempdir().unwrap();
    seed_legacy_document(
        temp.path(),
        r#"{
  "schema_version": 1,
  "entries": {
    "https://first.example": {
      "access_token": "TOKEN-FIRST",
      "issuer": "https://shared-as.example",
      "client_id": "cid-1"
    },
    "https://second.example": {
      "access_token": "TOKEN-SECOND",
      "issuer": "https://shared-as.example",
      "client_id": "cid-2"
    }
  }
}"#,
    );

    let first = run_cli(temp.path(), &["auth", "token", "https://first.example"]);
    let (first_out, first_err) = streams(&first);
    assert!(first.status.success(), "stderr: {first_err}");
    assert_eq!(first_out.trim_end(), "TOKEN-FIRST");

    let second = run_cli(temp.path(), &["auth", "token", "https://second.example"]);
    let (second_out, second_err) = streams(&second);
    assert!(second.status.success(), "stderr: {second_err}");
    assert_eq!(second_out.trim_end(), "TOKEN-SECOND");
}

// ---------------------------------------------------------------------------
// Group D2 — the migration does not DESTROY an existing credential file
//
// The two tests above prove entries survive a migration. Neither proves the
// FILE survives, and this is real credential data on a developer's machine: a
// port that read the previous format correctly and then truncated the document
// would pass every assertion in Group D. These two close that gap from both
// sides — a read must not touch the file at all, and the first write must carry
// every surviving login across.
// ---------------------------------------------------------------------------

/// A read-only command migrates IN MEMORY and leaves the document alone.
///
/// This is the property that makes the upgrade safe to try: an operator can run
/// `auth status` under a new `cargo-pmcp`, dislike what they see, and downgrade,
/// with their credential file byte-identical to how they left it.
#[cfg(unix)]
#[test]
fn a_previous_format_file_is_left_byte_identical_by_a_read_only_command() {
    let temp = tempfile::tempdir().unwrap();
    seed_legacy_document(temp.path(), LEGACY_TWO_ENTRY_DOCUMENT);
    let path = cache_path_in(temp.path());
    let before = std::fs::read(&path).unwrap();

    for args in [
        vec!["auth", "status"],
        vec!["auth", "token", "https://a.example"],
        vec!["auth", "status", "https://b.example"],
        // A logout naming a server the document does not carry removes nothing,
        // so it must not rewrite the document either.
        vec!["auth", "logout", "https://never-seen.example"],
    ] {
        let output = run_cli(temp.path(), &args);
        let (_, stderr) = streams(&output);
        assert!(output.status.success(), "{args:?} failed; stderr: {stderr}");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "{args:?} rewrote the credential file"
        );
    }
}

/// The first command that WRITES upgrades the document, and every login that
/// survived the migration is still there afterwards.
///
/// The third assertion is the one that proves the upgrade actually landed
/// without this test having to know the on-disk format: a second invocation
/// reports NO migration, which can only be true if the document now reads as
/// current.
#[cfg(unix)]
#[test]
fn the_first_write_upgrades_the_document_without_losing_a_surviving_login() {
    let temp = tempfile::tempdir().unwrap();
    seed_legacy_document(temp.path(), LEGACY_TWO_ENTRY_DOCUMENT);
    let path = cache_path_in(temp.path());
    let before = std::fs::read(&path).unwrap();

    let logout = run_cli(temp.path(), &["auth", "logout", "https://a.example"]);
    let (logout_out, logout_err) = streams(&logout);
    assert!(logout.status.success(), "stderr: {logout_err}");
    assert!(
        logout_out.contains("Logged out of https://a.example."),
        "unexpected stdout: {logout_out}"
    );
    assert_ne!(
        std::fs::read(&path).unwrap(),
        before,
        "a write was expected to upgrade the document"
    );

    let b = run_cli(temp.path(), &["auth", "token", "https://b.example"]);
    let (b_out, b_err) = streams(&b);
    assert!(
        b.status.success(),
        "the untouched login did not survive the upgrade; stderr: {b_err}"
    );
    assert_eq!(b_out.trim_end(), "LEGACY-TOKEN-B");
    assert!(
        !b_err.contains("Migrated"),
        "the document is still being migrated on every read, so it was never upgraded: {b_err}"
    );
}

// ---------------------------------------------------------------------------
// Group E — D-116-R1: a logout is scoped to ONE MCP server
// ---------------------------------------------------------------------------

/// Two MCP servers sharing one authorization server AND one account. Logging out
/// of A must leave B's `auth token` working. Under the two-part key this phase
/// replaced, the two shared one entry and this logout deleted both.
#[cfg(unix)]
#[tokio::test]
async fn logout_of_one_server_leaves_a_second_sharing_one_issuer_working_d_116_r1() {
    let temp = tempfile::tempdir().unwrap();
    seed(
        temp.path(),
        "https://shared-as.example",
        "https://a.example",
        "TOKEN-A",
    )
    .await;
    seed(
        temp.path(),
        "https://shared-as.example",
        "https://b.example",
        "TOKEN-B",
    )
    .await;

    let logout = run_cli(temp.path(), &["auth", "logout", "https://a.example"]);
    let (logout_out, logout_err) = streams(&logout);
    assert!(logout.status.success(), "stderr: {logout_err}");
    assert!(
        logout_out.contains("Logged out of https://a.example."),
        "unexpected stdout: {logout_out}"
    );

    let b = run_cli(temp.path(), &["auth", "token", "https://b.example"]);
    let (b_out, b_err) = streams(&b);
    assert!(
        b.status.success(),
        "B's credentials were destroyed by A's logout; stderr: {b_err}"
    );
    assert_eq!(b_out.trim_end(), "TOKEN-B");

    let a = run_cli(temp.path(), &["auth", "token", "https://a.example"]);
    assert!(!a.status.success(), "A's credentials must be gone");
}

// ---------------------------------------------------------------------------
// Group F — `auth status` and `auth token` output discipline
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[tokio::test]
async fn status_lists_every_stored_login_via_cli() {
    let temp = tempfile::tempdir().unwrap();
    seed(
        temp.path(),
        "https://as-one.example",
        "https://one.example",
        "TOKEN-ONE",
    )
    .await;
    seed(
        temp.path(),
        "https://as-two.example",
        "https://two.example",
        "TOKEN-TWO",
    )
    .await;

    let output = run_cli(temp.path(), &["auth", "status"]);
    let (stdout, stderr) = streams(&output);
    assert!(output.status.success(), "stderr: {stderr}");
    for needle in [
        "https://one.example",
        "https://two.example",
        "https://as-one.example",
        "https://as-two.example",
        "openid",
    ] {
        assert!(stdout.contains(needle), "missing {needle}: {stdout}");
    }
    // A status table must never print a bearer token.
    assert!(
        !stdout.contains("TOKEN-ONE") && !stdout.contains("TOKEN-TWO"),
        "status leaked an access token: {stdout}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn auth_token_prints_only_token_to_stdout() {
    // Prime a store in a temp HOME, then invoke `cargo pmcp auth token <url>`
    // and verify stdout is EXACTLY the token + newline (no banner/status).
    let temp = tempfile::tempdir().unwrap();
    seed(
        temp.path(),
        "https://issuer.example",
        "https://mockito.example",
        "SECRET-TOKEN-VALUE-42",
    )
    .await;

    let output = run_cli(temp.path(), &["auth", "token", "https://mockito.example"]);
    let (stdout, stderr) = streams(&output);
    assert!(output.status.success(), "stderr: {stderr}");
    assert_eq!(stdout.trim_end(), "SECRET-TOKEN-VALUE-42");
}

/// A renewal that fails must not fall back to printing the expired token.
///
/// The credential is expired and carries no refresh token, and its recorded
/// authorization server is a loopback port nothing listens on, so the SDK's
/// `RefreshOnly` path cannot succeed. `auth token` is consumed as
/// `TOKEN=$(cargo pmcp auth token URL)`; emitting the stale value would hand a
/// script a bearer that produces a 401 it has no way to interpret, instead of a
/// non-zero exit it does.
#[cfg(unix)]
#[tokio::test]
async fn an_expired_token_is_never_printed_when_the_renewal_fails() {
    let temp = tempfile::tempdir().unwrap();
    seed_expired(
        temp.path(),
        "http://127.0.0.1:1",
        "https://stale.example",
        "EXPIRED-TOKEN-MUST-NOT-BE-PRINTED",
    )
    .await;

    let output = run_cli(temp.path(), &["auth", "token", "https://stale.example"]);
    let (stdout, stderr) = streams(&output);
    assert!(
        !output.status.success(),
        "a failed renewal must be a non-zero exit; stdout: {stdout}"
    );
    assert!(
        stdout.trim().is_empty(),
        "the expired token reached stdout: {stdout}"
    );
    assert!(
        !stderr.contains("EXPIRED-TOKEN-MUST-NOT-BE-PRINTED"),
        "the expired token reached stderr: {stderr}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn api_key_flag_overrides_cached_oauth_token() {
    // Seeds a fake OAuth entry in a temp store, then invokes a server-connecting
    // command with `--api-key <FORCED>` against a mockito server that only
    // accepts the forced key. Succeeds when the outgoing Authorization header
    // contains "Bearer forced-key-123" (not the cached OAuth bearer).
    let temp = tempfile::tempdir().unwrap();

    // Spin up a mockito server that asserts the inbound Authorization header.
    let mut server = mockito::Server::new_async().await;
    let base_url = server.url();
    let mock = server
        .mock("POST", mockito::Matcher::Any)
        .match_header("authorization", "Bearer forced-key-123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#)
        .expect_at_least(1)
        .create_async()
        .await;

    // Seed the store with a DIFFERENT (cached OAuth) token for the same URL.
    // If the store were consulted (wrongly) it would send a different header
    // and mockito would return the default 501, failing the test.
    seed(
        temp.path(),
        "https://issuer.example",
        &normalize_server_key(&base_url).unwrap(),
        "CACHED-OAUTH-SHOULD-NOT-BE-USED",
    )
    .await;

    // Invoke `cargo pmcp test conformance <mockito_url> --api-key forced-key-123`.
    // conformance is the minimal command that consumes `AuthFlags::resolve()`
    // and flows through resolve_auth_middleware.
    let output = run_cli(
        temp.path(),
        &[
            "test",
            "conformance",
            &base_url,
            "--api-key",
            "forced-key-123",
        ],
    );

    // The test conformance command may exit non-zero on protocol assertion
    // failures from the dummy responses — we only care that the outgoing
    // header matched our mock. Assert via the mock's hit count.
    let _ = output; // status intentionally ignored
    mock.assert_async().await;
}
