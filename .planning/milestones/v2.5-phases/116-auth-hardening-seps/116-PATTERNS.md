# Phase 116: Auth Hardening SEPs - Pattern Map

**Mapped:** 2026-08-02
**Files analyzed:** 28 (14 new, 14 modified)
**Analogs found:** 26 / 28

> **Read this first.** RESEARCH.md's central architectural claim is confirmed by direct
> inspection: **this phase invents almost no new patterns.** `src/shared/pkce.rs` already IS the
> ungated wasm-clean primitive tier D-05/D-06 asks for; `src/error/mod.rs` already IS the marker
> pattern D-03 asks for; `cargo-pmcp/.../cache.rs` already IS the atomic-0o600-credential-file
> pattern D-19 asks for; `tests/oauth_dcr_integration.rs` already IS the mockito mock-AS harness
> with an `expect(0)` negative control. The phase's real work is **consolidation and wiring**.
> Every entry below names a concrete in-repo file + line range to copy from.

## File Classification

### New files

| New File | Role | Data Flow | Closest Analog | Match Quality |
|----------|------|-----------|----------------|---------------|
| `src/shared/oauth_validation.rs` *(name = planner's discretion)* | utility (pure library, ungated) | transform | `src/shared/pkce.rs` | **exact** |
| `src/shared/credential_store.rs` *(may be same module)* — trait + `InMemoryStore` | store / provider trait | CRUD | `src/shared/event_store.rs:17-117` (`EventStore` + `InMemoryEventStore`) | **exact** |
| File-backed `CredentialStore` impl (`oauth` + `!wasm32`) | store impl | file-I/O | `cargo-pmcp/src/commands/auth_cmd/cache.rs:91-123` (`write_atomic`) | **exact** |
| `tests/oauth_iss_validation.rs` | test (unit + property, pure fn) | transform | `tests/pkce_helper.rs` | **exact** |
| `tests/oauth_iss_integration.rs` | test (integration, mock AS) | request-response | `tests/oauth_dcr_integration.rs` | **exact** |
| `tests/oauth_discovery_validation.rs` (Pitfall 1 anchor check) | test (integration, negative control) | request-response | `tests/oauth_dcr_integration.rs:117-172` (`expect(0)` guard) | **exact** |
| `tests/oauth_discovery_urls.rs` (SEP-2351 candidates + probe order) | test (unit/property + integration) | transform + request-response | `tests/pkce_helper.rs` + `tests/oauth_dcr_integration.rs` | role-match (split) |
| `tests/oauth_state_csrf.rs` (D-12) | test (unit + integration) | transform | `tests/oauth_dcr_integration.rs`; logic from `examples/web-channel-client/client/src/lib.rs:247-251` | **exact** |
| `tests/oauth_credential_store.rs` (D-07/D-16) | test (unit + property) | CRUD | `cargo-pmcp/.../cache.rs:268-469` (`mod tests` + `mod proptests`) | **exact** |
| `tests/oauth_refresh.rs` (D-14 + SEP-2207) | test (integration, mock AS) | request-response | `tests/oauth_dcr_integration.rs` | **exact** |
| `fuzz/fuzz_targets/oauth_validation.rs` *(or extend `auth_flows.rs`)* | fuzz target | transform | `fuzz/fuzz_targets/pkce_helper.rs` | **exact** |
| `examples/cNN_oauth_iss_validation.rs` (ALWAYS: runnable example) | example | transform | `examples/c08_oauth_dcr.rs` | **exact** |
| cargo-pmcp v1→v2 migration test (crate-local) | test (unit) | file-I/O | `cargo-pmcp/.../cache.rs:297-341` (`read_rejects_wrong_schema_version`, `write_then_read_roundtrip`) | **exact** |
| wasm32 build fence (Makefile target or CI step) | config / build gate | batch | `Makefile:58-62` (`wasm-build`) | **exact** |

### Modified files

| Modified File | Role | Data Flow | What lands | Closest Analog | Match Quality |
|---------------|------|-----------|-----------|----------------|---------------|
| `src/error/mod.rs` | error model | — | `ISS_MISMATCH_MARKER` / `STATE_MISMATCH_MARKER` + ctors + `is_*` (D-03, **corrected by A2 → `Error::Protocol`**) | **self-analog** `:114-146` + `:587-648` | **exact** |
| `src/shared/mod.rs` | config (module decl) | — | `pub mod` for the new ungated module | `src/shared/mod.rs:18-22` (pkce decl + rationale comment) | **exact** |
| `src/lib.rs` | config (re-export) | — | crate-root re-export of the new pure fns | `src/lib.rs:106` | **exact** |
| `src/client/auth.rs` | service (HTTP client) | request-response | SEP-2351 ordered probe, RFC 8414 §3.3 anchor check, 5 bounded reads | `src/shared/sse_optimized.rs:280-319` (bounded read); self `:136-167` | role-match |
| `src/client/oauth.rs` | service (CLI OAuth flow) | request-response | `iss`/`state` validation call, D-04 builder, D-08 `Interactivity`, D-10 derivation, D-14 refresh fixes, store wiring, 5 bounded reads | multiple (see §Pattern Assignments) | role-match |
| `src/server/auth/provider.rs` | model (protocol types) | — | `application_type()` / `set_application_type()` inherent accessors over `extra` (D-09) | `src/types/protocol/mod.rs:380-398`; `src/server/tool_middleware.rs:119-134` | **exact** |
| `src/server/auth/providers/generic_oidc.rs` | service (auth provider) | request-response | SEP-2351 call site + 5 bounded reads | `src/client/auth.rs` (same shape) | **exact** |
| `src/server/auth/providers/cognito.rs` | service (auth provider) | request-response | SEP-2351 call site + 4 bounded reads | `src/client/auth.rs` (same shape) | **exact** |
| `tests/v2_bounded_reads_tripwire.rs` | test (tripwire) | batch | widen `EXTRA_SCOPE` **and** `REQUIRED_FILES` + module doc second-owner note | **self-analog** `:64-82` | **exact** |
| `tests/oauth_dcr_integration.rs` | test (integration) | request-response | AUTH-02 wire-body assertions (extend) | **self-analog** `:88-114` (`Matcher::PartialJsonString`) | **exact** |
| `cargo-pmcp/src/commands/auth_cmd/cache.rs` | store (CLI cache) | file-I/O | schema 1→2 migration; drop `TokenCacheV1` impl in favor of core store (D-17/D-19) | **self-analog** `:54-123` | **exact** |
| `cargo-pmcp/src/commands/auth_cmd/{login,logout,token,refresh,status}.rs` | controller (CLI subcommand) | CRUD | become thin wrappers over the core store trait | self `login.rs:45-72`, `logout.rs:26-62` | **exact** |
| `cargo-pmcp/Cargo.toml` | config | — | version bump + `pmcp` pin | — | n/a |
| `.planning/REQUIREMENTS.md` | docs (booking) | — | AUTH-01/02/03 `[x]` with cited artifact + named test binary + count (D-20) | Phase 115 booking discipline | n/a |

---

## Pattern Assignments

### `src/shared/oauth_validation.rs` (utility, pure library, ungated + wasm-clean)

**Analog:** `src/shared/pkce.rs` — this is an **exact** structural precedent: ungated, no
`reqwest`/`webbrowser`/`dirs`, crate-root re-exported, RFC-vector tested, with a dedicated
ALWAYS-coverage test file and a fuzz target. Copy its shape wholesale.

**Module-doc pattern** (`src/shared/pkce.rs:1-20`) — note it explicitly names *why* it is ungated
and what it contrasts with. The new module's doc must do the same:

```rust
//! Target-agnostic PKCE (RFC 7636) crypto helper for OAuth 2.0 Authorization
//! Code flows.
//! ...
//! Unlike the native CLI flow in
//! [`crate::client::oauth`] (which uses the optional `rand` dependency and is
//! therefore not available on `wasm32`), this module is **ungated** and uses
//! [`getrandom::fill`] for randomness so it compiles and runs identically on
//! the host and on `wasm32-unknown-unknown` (Web Crypto via the `wasm_js`
//! backend).
```

**Imports pattern** (`src/shared/pkce.rs:50-52`) — crate-internal error type, no optional deps:

```rust
use crate::error::{Error, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
```

The new module additionally needs `url::Url` (non-optional, already the callback/discovery parser —
`src/client/oauth.rs:30`) for `query_pairs()` and for SEP-2351 path arithmetic. **Do not add
anything else** — the phase's dependency delta must be empty.

**Pure-function + `#[must_use]` + doc-example pattern** (`src/shared/pkce.rs:99-120`):

```rust
/// Compute the S256 PKCE code challenge for a verifier (RFC 7636 §4.2).
/// ...
/// # Examples
///
/// ```
/// use pmcp::shared::pkce::code_challenge_s256;
///
/// let challenge = code_challenge_s256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
/// assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
/// ```
#[must_use]
pub fn code_challenge_s256(verifier: &str) -> String {
```

**Fallible-primitive pattern** (`src/shared/pkce.rs:65-70`) — errors map to `Error::internal` in
exactly one place, `# Errors` rustdoc section, no `unwrap`/`expect`:

```rust
pub(crate) fn random_bytes() -> Result<[u8; PKCE_RANDOM_BYTES]> {
    let mut buf = [0u8; PKCE_RANDOM_BYTES];
    getrandom::fill(&mut buf)
        .map_err(|e| Error::internal(format!("CSPRNG (getrandom) failed: {e}")))?;
    Ok(buf)
}
```

**Do NOT re-implement `state` generation.** `generate_state()` already exists at
`src/shared/pkce.rs:143-146` and is already re-exported at crate root (`src/lib.rs:106`):

```rust
pub use shared::pkce::{code_challenge_s256, generate_code_verifier, generate_state};
```

**Inline test-module pattern** (`src/shared/pkce.rs:148-199`) — each `#[test]` carries a one-line
doc naming the invariant it pins and, where applicable, the threat tag (`(HIGH-1)`, `(T-103-RNG)`).

**Module declaration to mirror** (`src/shared/mod.rs:18-22`) — the rationale comment is load-bearing;
it is what stops a future contributor from "tidying" the new module behind a `cfg`:

```rust
/// Target-agnostic PKCE (RFC 7636) crypto helper (verifier/challenge/state).
///
/// Ungated on purpose — compiles on host AND wasm32 via `getrandom::fill`
/// (contrast the `#[cfg(not(target_arch = "wasm32"))]` peer/stdio entries).
pub mod pkce;
```

---

### `src/shared/…::CredentialStore` trait + `InMemoryStore` (provider trait, CRUD)

**Analog:** `src/shared/event_store.rs:9,17-41` (trait) and `:101-117` (in-memory impl). Same tier:
ungated, `async_trait`, `Send + Sync`, ships with an in-memory impl, re-exported from
`src/shared/mod.rs:65-68`.

**Trait pattern** (`src/shared/event_store.rs:17-41`):

```rust
use async_trait::async_trait;

/// Event store trait for persisting and retrieving events.
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Store an event with its metadata.
    async fn store_event(&self, event: StoredEvent) -> Result<()>;

    /// Retrieve events since a given event ID.
    async fn get_events_since(
        &self,
        event_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<StoredEvent>>;
    // ...
}
```

**I/O-free construction pattern** (`src/shared/event_store.rs:101-117`) — every value is a
constructor parameter; no `std::env`, no disk, no network. This is exactly D-07's mandate and the
platform-proven constraint from `cognito_external_provider.rs`:

```rust
pub struct InMemoryEventStore {
    events: Arc<RwLock<VecDeque<StoredEvent>>>,
    tokens: Arc<RwLock<HashMap<String, ResumptionState>>>,
    max_events: usize,
    max_age: chrono::Duration,
}

impl InMemoryEventStore {
    /// Create a new in-memory event store.
    pub fn new(max_events: usize, max_age: chrono::Duration) -> Self {
        Self {
            events: Arc::new(RwLock::new(VecDeque::new())),
            tokens: Arc::new(RwLock::new(HashMap::new())),
            max_events,
            max_age,
        }
    }
```

**Default-with-optional-override pattern for the trait's consumer** — see `AuthProvider`
(`src/shared/streamable_http.rs:1516-1539`), the seam this phase supplies machinery *behind*. Note
its `on_unauthorized` default body: **new trait methods must ship with a default impl** so existing
implementors (the durable agent) do not break:

```rust
pub trait AuthProvider: Send + Sync + Debug {
    /// Returns an access token.
    async fn get_access_token(&self) -> Result<String>;

    /// ... The default implementation is a no-op, preserving backward compatibility for
    /// all existing `AuthProvider` implementations.
    async fn on_unauthorized(&self) -> Result<()> {
        Ok(())
    }
}
```

**Re-export pattern** (`src/shared/mod.rs:65-68`) — trait + impls + config types together:

```rust
pub use event_store::{
    EventStore, EventStoreConfig, InMemoryEventStore, MessageDirection, ResumptionManager,
    ResumptionState, ResumptionToken, StoredEvent,
};
```

---

### File-backed `CredentialStore` impl (store, file-I/O, `oauth` + `!wasm32`)

**Analog:** `cargo-pmcp/src/commands/auth_cmd/cache.rs`. D-19 says *port this code into core, don't
rewrite it*. It already does everything the core store needs.

**Atomic 0o600 write pattern** (`cache.rs:91-123`) — tempfile-in-same-dir → parent 0o700 → file
0o600 → `persist` (rename). **Copy verbatim, only swapping `anyhow` for `pmcp::Error`:**

```rust
    /// Atomic write: tempfile-in-same-dir -> chmod -> persist (rename).
    ///
    /// Cross-platform atomic on modern Linux + Windows per tempfile docs.
    /// Concurrent writers are last-writer-wins (see module rustdoc).
    pub fn write_atomic(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("cache path has no parent: {}", path.display()))?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create cache dir {}", parent.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
        }

        let mut tmp = NamedTempFile::new_in(parent)?;
        let json = serde_json::to_vec_pretty(self)?;
        tmp.write_all(&json)?;
        tmp.flush()?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tmp.as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }

        tmp.persist(path)
            .map_err(|e| anyhow::anyhow!("atomic rename failed: {e}"))?;
        Ok(())
    }
```

**Schema-version gate pattern** (`cache.rs:54-89`) — and note the **forward-compat trap** RESEARCH
flagged: this `read()` *hard-errors* on an unknown version, so once core writes `schema_version: 2`
an older installed `cargo-pmcp` fails rather than degrading:

```rust
impl TokenCacheV1 {
    /// Current on-disk schema version.
    pub const CURRENT_VERSION: u32 = 1;

    /// Read a cache file, returning `empty()` if the file does not exist.
    /// Errors on malformed JSON or unsupported `schema_version`.
    pub fn read(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => {
                let v: Self = serde_json::from_str(&s).with_context(|| {
                    format!("cache file corrupt — delete {} to reset", path.display())
                })?;
                if v.schema_version != Self::CURRENT_VERSION {
                    anyhow::bail!(
                        "cache schema_version {} unsupported (expected {}); upgrade cargo-pmcp",
                        v.schema_version,
                        Self::CURRENT_VERSION
                    );
                }
                Ok(v)
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::empty()),
            Err(e) => Err(anyhow::anyhow!("failed to read cache file {}: {e}", path.display())),
        }
    }
```

**Entry shape** (`cache.rs:32-52`) — already carries `issuer` and `client_id` per entry, which is
what makes the D-17 1→2 re-key to `(issuer, account)` losslessly possible:

```rust
pub struct TokenCacheEntry {
    /// Bearer access token. Sensitive — NEVER logged, never printed except by
    /// `cargo pmcp auth token <url>`.
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    /// Effective OAuth issuer (caller-provided or OIDC-discovered).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    /// Effective client_id (DCR-issued or caller-provided).
    pub client_id: String,
}
```

**Key-normalization pattern for the D-18 per-server issuer tracking** (`cache.rs:135-155`) —
`normalize_cache_key` is the existing "one MCP server URL → one stable key" function and is already
property-tested for idempotence (`cache.rs:419-441`).

**Note the deliberate contrast with the core cache** it replaces (`src/client/oauth.rs:149-156`),
which has **no issuer field** — the reason D-17 discards it rather than migrating it:

```rust
/// Token cache stored on disk.
#[derive(Debug, Serialize, Deserialize)]
struct TokenCache {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<u64>,
    scopes: Vec<String>,
}
```

---

### `src/error/mod.rs` — `ISS_MISMATCH_MARKER` (+ state marker) (error model)

**Analog:** the same file. **Self-analog, exact.** Three pieces, all already present.

> **Apply RESEARCH A2:** D-03 said the marker rides `Error::Authentication`. It cannot —
> `Error::Authentication(String)` is a bare-`String` tuple variant (`src/error/mod.rs:40-42`) with
> no `data` member, and the whole marker machinery is hard-wired to `Error::Protocol`. Use
> `Error::Protocol`, exactly as `retired_on_v2` does.

**1. Marker const + `data`-key consts** (`src/error/mod.rs:126-146`) — copy the "do not change this
string" compatibility note verbatim into the new marker's rustdoc:

```rust
/// The stable programmatic identity of [`Error::retired_on_v2`].
///
/// Carried in the error's `data.pmcpError`. It is the discriminator
/// [`Error::is_retired_on_v2`] matches on, so it is part of the crate's
/// compatibility surface: **do not change this string**.
pub const RETIRED_ON_V2_MARKER: &str = "RetiredOnV2";

/// The `data` member both MRTR markers ride under.
const PMCP_ERROR_KEY: &str = "pmcpError";

/// The `data` member carrying the retired method's name.
const RETIRED_METHOD_KEY: &str = "method";
```

**2. Constructor** (`src/error/mod.rs:587-605`) — `#[must_use]`, `Error::Protocol`, a message a
human can act on, and the marker + typed payload keys in `data`. Note the inline comment explaining
the `ErrorCode` choice — the new constructor should justify its code the same way:

```rust
    #[must_use]
    pub fn retired_on_v2(method: &str, replacement: &str) -> Self {
        Self::Protocol {
            // The field is `ErrorCode`, not a bare `i32` — the value comes from
            // the centralized VERS-06 table and is WRAPPED here. `-32601` is the
            // code the SERVER would have answered with; producing it locally
            // keeps a caller that already branches on method-not-found working.
            code: ErrorCode::METHOD_NOT_FOUND,
            message: format!(
                "{method} was removed in MCP 2026-07-28 and this connection speaks that version; \
                 use {replacement} instead"
            ),
            data: Some(serde_json::json!({
                PMCP_ERROR_KEY: RETIRED_ON_V2_MARKER,
                RETIRED_METHOD_KEY: method,
                RETIRED_REPLACEMENT_KEY: replacement,
            })),
        }
    }
```

**3. Predicate + typed field accessors + the private helpers they rest on**
(`src/error/mod.rs:607-648`). The two private helpers **already exist** — the new markers reuse
them and add nothing:

```rust
    /// Whether this is the [`Error::retired_on_v2`] local fail-fast.
    #[must_use]
    pub fn is_retired_on_v2(&self) -> bool {
        self.pmcp_error_marker() == Some(RETIRED_ON_V2_MARKER)
    }

    /// One string field of an [`Error::retired_on_v2`] marker payload.
    ///
    /// Borrows rather than allocating: every caller of the two public accessors
    /// only ever compares the value.
    fn retired_field(&self, key: &str) -> Option<&str> {
        if !self.is_retired_on_v2() {
            return None;
        }
        self.protocol_data()?.get(key)?.as_str()
    }

    /// The `data` object of an [`Error::Protocol`], if it has one.
    fn protocol_data(&self) -> Option<&serde_json::Map<String, serde_json::Value>> {
        match self {
            Self::Protocol { data, .. } => data.as_ref()?.as_object(),
            _ => None,
        }
    }

    /// The `data.pmcpError` marker string, if this error carries one.
    fn pmcp_error_marker(&self) -> Option<&str> {
        self.protocol_data()?.get(PMCP_ERROR_KEY)?.as_str()
    }
```

**Doctest pattern for the constructor** (`src/error/mod.rs:576-586`) — includes a **negative
control** on the last line. Copy that habit; it is the cheapest place to prove the discriminator
does not match unrelated errors:

```rust
    /// ```rust
    /// use pmcp::Error;
    ///
    /// let err = Error::retired_on_v2("resources/subscribe", "subscriptions/listen");
    /// assert!(err.is_retired_on_v2());
    /// assert_eq!(err.retired_method(), Some("resources/subscribe"));
    /// assert!(err.to_string().contains("subscriptions/listen"));
    /// assert!(!Error::internal("nope").is_retired_on_v2());
    /// ```
```

> **Pitfall 7 note:** `make doc-check` is the ONLY gate that compiles this file's rustdoc under
> `oauth`, and `src/error/mod.rs` already carries 1 of the 28 pre-existing errors. Measure as a
> delta, not an absolute.

---

### `src/server/auth/provider.rs` — `application_type()` accessors (model, D-09)

**Analog:** `src/types/protocol/mod.rs:380-398` (the `with_meta` / `get_meta` pair over a flattened
map) and `src/server/tool_middleware.rs:119-134` (the `with_` / `get_` / `set_` triple).

**The carrier already exists** — `DcrRequest` (`src/server/auth/provider.rs:348-350`) and
`DcrResponse` (`:379-381`) both end with:

```rust
    /// Additional metadata.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
```

**Accessor-pair pattern to copy** (`src/types/protocol/mod.rs:380-398`) — note the rustdoc names the
round-trip property AND the collision hazard, which is exactly D-09's "documented precedence rule"
obligation:

```rust
    /// Attach an arbitrary namespaced `_meta` key/value.
    ///
    /// The key is inserted into the flattened [`other`](Self::other) map and
    /// round-trips through serialize/deserialize. Use namespaced keys (e.g.
    /// `io.modelcontextprotocol/related-task`) to avoid collisions with the
    /// typed `progressToken`/`_task_id` fields.
    #[must_use]
    pub fn with_meta(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.other.insert(key.into(), value);
        self
    }

    /// Read a namespaced `_meta` key previously set via [`with_meta`](Self::with_meta)
    /// or populated on deserialize.
    #[must_use]
    pub fn get_meta(&self, key: &str) -> Option<&serde_json::Value> {
        self.other.get(key)
    }
```

**Mutating-setter variant** (`src/server/tool_middleware.rs:125-134`) — for `set_application_type`:

```rust
    /// Get metadata value.
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Set metadata value.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }
```

**Typed-projection-from-a-loose-map pattern** (`src/types/ui.rs:460-508`) — if the planner wants a
typed `ApplicationType` enum rather than a bare `&str`, `UiMetadata::from_metadata` /
`to_metadata` is the in-repo precedent for reading a typed value out of a
`HashMap<String, serde_json::Value>` with a documented fallback order:

```rust
    /// Extract from a metadata `HashMap`
    ///
    /// Reads from nested `"ui"` object first, falling back to legacy flat
    /// `"ui/resourceUri"` key for backward compatibility.
    pub fn from_metadata(metadata: &HashMap<String, serde_json::Value>) -> Self {
        let ui_resource_uri = metadata
            .get("ui")
            .and_then(|v| v.get("resourceUri"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| { /* legacy flat key */ });
```

**The construction site that receives the derived value** (`src/client/oauth.rs:241-257`) — note it
is *also* where SEP-2207's `grant_types` fix lands (RESEARCH A3), i.e. one edit site, two SEPs:

```rust
        let request = crate::server::auth::provider::DcrRequest {
            redirect_uris: vec![redirect_uri],
            client_name: Some(client_name),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            token_endpoint_auth_method: Some("none".to_string()),
            grant_types: vec!["authorization_code".to_string()],   // <- SEP-2207: add "refresh_token"
            // `DcrRequest` has `#[serde(skip_serializing_if = "Vec::is_empty")]`;
            // RFC 7591 §3.1 requires `response_types` in the body, so it must be
            // non-empty. `"code"` is the authorization-code public-PKCE flow.
            response_types: vec!["code".to_string()],
            scope: None,
            software_id: None,
            software_version: None,
            extra: Default::default(),                             // <- D-09's carrier
        };
```

The `redirect_uri` D-10 derives from is built two lines above (`:237-239`), with the RFC 8252 §7.3
rationale already written down — the derivation must agree with it:

```rust
        // Literal `127.0.0.1` rather than `localhost` — per RFC 8252 §7.3, avoids
        // browsers resolving `localhost` to `::1` when the listener binds IPv4-only.
        let redirect_uri = format!("http://127.0.0.1:{}/callback", self.config.redirect_port);
```

---

### `src/client/auth.rs` — SEP-2351 probe + RFC 8414 §3.3 anchor (service, request-response)

**Analog:** self (`:136-197`) for the shape; `src/shared/sse_optimized.rs:280-319` for the bounded
read that replaces `.json().await`.

**Current single-candidate construction to replace** (`src/client/auth.rs:136-140`) — today's form
becomes **candidate #3**, not a deleted branch (Pitfall 2: replacing it 404s Microsoft Entra ID,
whose URL is in this file's own doctest at `:127`):

```rust
    pub async fn discover(&self, issuer_url: &str) -> Result<OidcDiscoveryMetadata> {
        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            issuer_url.trim_end_matches('/')
        );
```

**Retry/probe loop pattern already present** (`src/client/auth.rs:142-167`) — the ordered-candidate
probe slots into the same `while` + `last_error` shape:

```rust
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < self.max_retries {
            match self.fetch_discovery(&discovery_url).await {
                Ok(metadata) => return Ok(metadata),
                Err(e) => {
                    if self.should_retry(&e) && attempts + 1 < self.max_retries {
                        attempts += 1;
                        tokio::time::sleep(self.retry_delay).await;
                        continue;
                    }
                    last_error = Some(e);
                    break;
                },
            }
        }
```

**Where the §3.3 anchor check goes** (`src/client/auth.rs:170-197`) — *before the metadata escapes
the function*, i.e. between the deserialize and the `Ok`. Today there is no comparison at all:

```rust
    /// Fetch discovery metadata from a URL.
    async fn fetch_discovery(&self, url: &str) -> Result<OidcDiscoveryMetadata> {
        let response = self.client.get(url).header("Accept", "application/json").send().await
            .map_err(|e| Error::protocol(ErrorCode::INTERNAL_ERROR,
                format!("Failed to fetch discovery document: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::protocol(ErrorCode::INTERNAL_ERROR,
                format!("Discovery endpoint returned status: {}", response.status())));
        }

        response.json::<OidcDiscoveryMetadata>().await.map_err(|e| {   // <- D-15 unbounded read
            Error::protocol(ErrorCode::PARSE_ERROR,
                format!("Failed to parse discovery document: {}", e))
        })
    }
```

**Builder-with-settings pattern** (`src/client/auth.rs:106-112`) — the semver-safe shape for any new
knob on this client (a new *field* on a pub all-pub-field struct is major; a new **constructor or
inherent method** is minor):

```rust
    pub fn with_settings(max_retries: usize, retry_delay: Duration) -> Self {
        Self {
            client: reqwest::Client::new(),
            max_retries,
            retry_delay,
        }
    }
```

**Existing URL-construction test to REFRAME, not delete** (`src/client/auth.rs:434-461`) — its third
case (`https://auth.example.com/oauth` → appended) is not wrong, it is the *last* candidate:

```rust
    #[test]
    fn test_discovery_url_construction() {
        let test_cases = vec![
            ("https://example.com", "https://example.com/.well-known/openid-configuration"),
            ("https://example.com/", "https://example.com/.well-known/openid-configuration"),
            ("https://auth.example.com/oauth",
             "https://auth.example.com/oauth/.well-known/openid-configuration"),
        ];
```

---

### `src/client/oauth.rs` — the CLI flow becomes a caller of the pure tier (service)

**Analogs:** several, per concern. This is the phase's highest-complexity file
(`authorization_code_flow_inner` is `:621-772`, ~150 lines) and the PR-blocking
`pmat quality-gate --checks complexity` (cog ≤ 25) applies — **extract into the pure tier rather
than inlining**.

**D-12 — the `state` defect at source** (`src/client/oauth.rs:664-672`). The value is an unnamed
temporary, and it also **wrongly reuses the PKCE verifier generator** for a CSRF token:

```rust
        auth_url
            .query_pairs_mut()
            .append_pair("client_id", &resolved_client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("scope", &self.config.scopes.join(" "))
            .append_pair("code_challenge", &code_challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &Self::generate_code_verifier()); // Random state for CSRF protection
```

Replace with `pmcp::shared::pkce::generate_state()` bound into the per-request record. Also retire
the private PKCE duplicates at `:592-604`, which are the reason this flow is not wasm-clean:

```rust
    /// Generate PKCE code verifier (RFC 7636).
    fn generate_code_verifier() -> String {
        let random_bytes: [u8; 32] = rand::rng().random();
        URL_SAFE_NO_PAD.encode(random_bytes)
    }
```

**D-01/D-02/D-12 — the callback that must validate** (`src/client/oauth.rs:687-727`). The `oneshot`
channel carries a bare `String` code; it must carry the full parsed response (or validate in-task):

```rust
        let (tx, rx) = oneshot::channel();
        let callback_task = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut reader = BufReader::new(&mut stream);
                let mut request_line = String::new();

                if reader.read_line(&mut request_line).await.is_ok() {
                    if let Some(path) = request_line.split_whitespace().nth(1) {
                        if let Ok(callback_url) = Url::parse(&format!("http://localhost{}", path)) {
                            let code = callback_url
                                .query_pairs()
                                .find(|(key, _)| key == "code")
                                .map(|(_, value)| value.to_string());
                            // ... success/failure HTML, then:
                            if let Some(code) = code {
                                let _ = tx.send(code);
                            }
```

`callback_url.query_pairs()` is already the right decoder — RFC 9207 §2.4 *requires* the
`application/x-www-form-urlencoded` decode before comparison and `query_pairs()` performs it. Do not
hand-roll a percent-decode.

**The correct `state` comparison already exists in-repo** — lift it, do not re-derive
(`examples/web-channel-client/client/src/lib.rs:247-251`, tagged `T-103-CSRF`):

```rust
// CSRF: the returned state MUST equal the state we generated (T-103-CSRF).
let expected = storage_get(KEY_STATE)?
    .ok_or_else(|| js_error("no stored OAuth state — start login again"))?;
if state != expected {
    return Err(js_error("OAuth state mismatch — possible CSRF, aborting"));
}
```

**D-08 — the fall-through that must become unreachable** (`src/client/oauth.rs:428-482`). Refresh
failure silently proceeds to `authorization_code_flow`, which binds a listener and waits 5 minutes
(`:732-737`):

```rust
                // Try to refresh if we have a refresh token
                if let Some(refresh_token) = cached.refresh_token {
                    tracing::warn!("OAuth token expired, refreshing...");
                    if let Ok(new_token) = self.refresh_token(&refresh_token).await {
                        self.cache_token(&new_token, cache_file).await?;
                        return Ok(new_token.access_token);
                    }
                }
            }
        }

        // No valid cached token, try authorization code flow first
        tracing::info!("No cached token found, starting OAuth flow...");
```

**D-14 — the three refresh defects at source** (`src/client/oauth.rs:915-949` and `:960-975`):

```rust
    /// Refresh an existing token.
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse> {
        let metadata = self.get_metadata().await?;
        let token_endpoint = &metadata.token_endpoint;

        // defect 2 — DCR-issued client_id lives in AuthorizationResult, never in config
        let client_id = self.config.client_id.as_deref().ok_or_else(|| {
            Error::internal("cannot refresh token without a cached client_id".to_string())
        })?;

        let response = self
            .client
            .post(token_endpoint)
            .form(&[
                ("client_id", client_id),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),      // defect 3 — no `scope` sent
            ])
            .send()
            .await
            .map_err(|e| Error::internal(format!("Failed to refresh token: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::internal(format!(
                "Token refresh failed: {}",
                response.text().await.unwrap_or_default()   // <- D-15 unbounded read
            )));
        }
```

```rust
        let cache = TokenCache {
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.clone(),   // defect 1 — None overwrites the good token
            expires_at,
            scopes: self.config.scopes.clone(),
        };
```

**The `TokenExchangeClient::refresh_token` sibling already sends `scope`** — reference shape for
defect 3 (`src/client/auth.rs:378-394`):

```rust
        let mut params = vec![
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
        ];

        if let Some(s) = scope {
            params.push(("scope", s));
        }
```

**D-04 — env-var override precedence pattern** (`src/server/observability/config.rs:127-153`). The
house shape: an `apply_env_overrides` that only overrides when the var parses, so absent = current
behavior. Precedence for D-04 is **env > builder > discovery flag**:

```rust
    /// Apply environment variable overrides.
    fn apply_env_overrides(&mut self) {
        // Master switch
        if let Ok(enabled) = std::env::var("PMCP_OBSERVABILITY_ENABLED") {
            if let Ok(v) = enabled.parse() {
                self.enabled = v;
            }
        }
```

> Read the env var at the **call site**, not in a constructor — D-07's I/O-free-construction rule
> forbids `std::env` inside the store; the same discipline should apply to `OAuthHelper` so a
> platform can supply the policy as a parameter.

**Token-logging hazard to fix while in the area** (`src/client/oauth.rs:1015-1021`) — logs the first
20 chars of a live access token at debug level. The platform convention (adopted as a design input)
is sha256-prefix only:

```rust
        tracing::debug!(
            "Creating OAuth middleware with token: {}...",
            &access_token[..access_token.len().min(20)]
        );
```

**`unwrap()` on `SystemTime` in production paths this phase edits** — `:434-437` and `:962-968`. Note
`make check-unwraps` is a **no-op stub** (`Makefile:768-772`); do not cite it as evidence.

---

### `src/server/auth/providers/{generic_oidc,cognito}.rs` — SEP-2351 + bounded reads (service)

**Analog:** each other, and `src/client/auth.rs`. All three build the same wrong URL, so one shared
pure `discovery_url_candidates(issuer)` serves all three call sites.

**`generic_oidc.rs:390-416`:**

```rust
/// Fetch OIDC discovery document (helper function).
#[cfg(not(target_arch = "wasm32"))]
async fn fetch_discovery_doc(http_client: &reqwest::Client, issuer: &str) -> Result<OidcDiscovery> {
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        issuer.trim_end_matches('/')
    );
    tracing::debug!("Fetching OIDC discovery from {}", discovery_url);
    // ...
    response
        .json()                                    // <- D-15 unbounded read
        .await
        .map_err(|e| Error::internal(format!("Failed to parse discovery: {}", e)))
```

**`cognito.rs:270-292`** — same construction, plus a TTL cache read above it (`:259-267`) that the
new candidate probe must keep intact:

```rust
        // Fetch discovery document
        let discovery_url = format!("{}/.well-known/openid-configuration", self.issuer);
        tracing::debug!("Fetching OIDC discovery from {}", discovery_url);
```

Note `cognito.rs` does **not** even `trim_end_matches('/')`, so a trailing-slash issuer produces a
double slash today.

---

### Bounded whole-body reads (D-15) — all four auth files

**Analog:** `src/shared/sse_optimized.rs:280-319`. This is the **reqwest** worked implementation and
the auth files are reqwest. (`src/shared/streamable_http.rs:528-551`'s `collect_body_within_cap` is
the hyper sibling — wrong client for these files.)

**The two-refusal pattern to copy** (`src/shared/sse_optimized.rs:280-319`):

```rust
    async fn collect_sse_text_within_cap(
        mut response: reqwest::Response,
        max_bytes: usize,
    ) -> Result<String> {
        // Refusal 1 — advisory, and only ever an early exit.
        if let Some(declared) = response.content_length() {
            if declared > max_bytes as u64 {
                return Err(Self::sse_body_over_cap(max_bytes, Some(declared)));
            }
        }

        // Refusal 2 — authoritative, over the bytes actually delivered.
        let mut accumulated: Vec<u8> = Vec::new();
        loop {
            let next = response.chunk().await;
            let Some(chunk) =
                next.map_err(|e| Error::internal(format!("SSE body read failed: {}", e)))?
            else {
                break;
            };
            // Overflow-safe by construction: `accumulated.len() <= max_bytes` is
            // the loop invariant, so `max_bytes - accumulated.len()` cannot
            // underflow, and no unguarded `a + b` is ever computed.
            if chunk.len() > max_bytes - accumulated.len() {
                return Err(Self::sse_body_over_cap(max_bytes, None));
            }
            accumulated.extend_from_slice(&chunk);
        }
```

**Refusal-message invariant** (`src/shared/sse_optimized.rs:321-344`) — the message names the LIMIT
and the observed size and **echoes no body content**:

```rust
    /// Names the LIMIT and the observed size, and deliberately echoes no body
    /// content: the refusal must not become a channel for the very bytes it
    /// refused. `declared` is `Some` only when the peer's `Content-Length` was
    /// itself over the cap; when the peer understated or omitted it the read is
    /// stopped mid-flight and no total is knowable, so the message says so rather
    /// than inventing one.
    fn sse_body_over_cap(max_bytes: usize, declared: Option<u64>) -> Error {
```

**Cap-constant pattern** (`src/shared/http_constants.rs:93`):

```rust
pub const DEFAULT_HTTP_SSE_BUFFERED_BYTES: usize = 16 * 1024 * 1024;
```

**The existing DCR cap is the WEAKER shape and should be upgraded while in the area**
(`src/client/oauth.rs:278-291`) — it allocates the whole body via `.bytes()` and *then* measures it,
so it bounds what is ACCEPTED, not what is ALLOCATED:

```rust
        // reqwest has no direct bytes-limit API — read the body as bytes, enforce
        // the cap, then parse from slice.
        const MAX_DCR_RESPONSE_BYTES: usize = 1_048_576; // 1 MiB
        let bytes = response
            .bytes()                                  // <- allocates the whole body first
            .await
            .map_err(|e| Error::internal(format!("Failed to read DCR response body: {e}")))?;
        if bytes.len() > MAX_DCR_RESPONSE_BYTES {
```

> **Measured 2026-08-02 (naive line-count of the 7 `WHOLE_BODY_NEEDLES` per file):**
> `generic_oidc.rs` 5, `cognito.rs` 4, `client/oauth.rs` 5, `client/auth.rs` 5 = **19 lines**.
> D-113-V records **31 reads** across the same four files. The counting methods differ (D-113-V
> counts per read-site including rustfmt-split chains and multiple needles per statement; the above
> counts matching *lines*). Do not treat either number as the closure condition — the closure
> condition is the tripwire below reporting zero after the fence is widened.
>
> **Critical:** `bound_in_scope` (`tests/v2_bounded_reads_tripwire.rs:611-618`) returns `false` for
> **every reqwest needle** (`.text().await`, `.bytes().await`, `.json().await`, `.json::<`) — there
> is no recognized bounded form. So each site must be *rewritten* to the `chunk()`-accumulate shape
> (then `serde_json::from_slice`), not annotated. Adding an allowlist entry instead would contradict
> the allowlist's own written floor ("This list should shrink, never grow. It is now EMPTY").

---

### `tests/v2_bounded_reads_tripwire.rs` — scope widening (test, D-15)

**Analog:** self. **Both consts must be extended** (`:64-82`); `REQUIRED_FILES` is the anti-vacuity
guard, and widening `EXTRA_SCOPE` alone would let a future path typo silently drop coverage:

```rust
/// The directory walked at runtime, so a NEW file cannot escape the scan by
/// nobody remembering to add it here. Losing coverage by omission is exactly
/// how this requirement reopened three times.
const SHARED_DIR: &str = "src/shared";

/// The two individually-named files HTTP-09 puts in scope beyond `src/shared/`.
const EXTRA_SCOPE: &[&str] = &[
    "src/client/subscriptions.rs",
    "src/server/streamable_http_server.rs",
];

/// Files whose absence from the discovered scope means discovery is broken.
///
/// Without this, a `read_dir` that silently returned nothing would make every
/// check in this file pass over an empty set.
const REQUIRED_FILES: &[&str] = &[
    "http.rs",
    "sse_parser.rs",
    "streamable_http.rs",
    "streamable_http_server.rs",
    "subscriptions.rs",
];
```

**The empty allowlist and its written floor** (`:580-591`) — do not grow it:

```rust
/// Whole-body reads that are not bounded in their own statement.
///
/// An entry is a REVIEWED, WRITTEN exemption, never a silent one: `why` must say
/// either what bounds the read or, plainly, that it is unbounded and who owns
/// the fix. ...
/// This list should shrink, never grow. It is now EMPTY, which is its floor:
const WHOLE_BODY_ALLOWLIST: &[Allowed] = &[];
```

**Module-doc ownership note** (`:1-13`) — the doc quotes HTTP-09's requirement text verbatim as its
scope justification. Adding auth files means the doc must **also name AUTH-03/D-15 as the second
owner**, or the file reads as enforcing something its own stated requirement does not cover.

**Failure message pattern** (`:650-658`) — names the required action and forbids the wrong fix. The
new auth-file guidance must name the `chunk()`-accumulate shape, since `Limited` (hyper) does not
apply to reqwest:

```rust
    assert!(
        violations.is_empty(),
        "HTTP-09: unbounded whole-body read(s) over peer-supplied bytes:{violations}\n\
         Required action: wrap the read in `http_body_util::Limited` with the transport's \
         configured cap, exactly as `collect_body_within_cap` does in src/shared/http.rs and \
         src/shared/streamable_http.rs. If this site genuinely cannot be bounded, add a \
         WHOLE_BODY_ALLOWLIST entry with a written justification and get it reviewed. \
         Deleting the needle is not a fix."
    );
```

---

### `tests/oauth_*.rs` (new integration suites) — mockito mock-AS

**Analog:** `tests/oauth_dcr_integration.rs`. Copy the header, the `discovery_body` helper, and —
most importantly — the `expect(0)` negative-control idiom.

**Header + fixture helper** (`tests/oauth_dcr_integration.rs:1-32`). **Note line 9**: the
`#![cfg(feature = "oauth")]` is precisely why this file contributes zero tests under
`make quality-gate` (Pitfall 3). New files inherit that property:

```rust
//! Integration tests for Dynamic Client Registration (RFC 7591) in `OAuthHelper`.
//!
//! Uses mockito to simulate a real OAuth discovery server + DCR endpoint
//! without needing network access. Covers:
//! - RFC 7591 §3.1 `response_types: ["code"]` must appear in the wire body
//! ...

#![cfg(feature = "oauth")]

use mockito::{Matcher, Server};
use pmcp::client::oauth::{OAuthConfig, OAuthHelper};
use serde_json::json;

fn discovery_body(base: &str, with_reg: bool) -> String {
    let mut v = json!({
        "issuer": base,
        "authorization_endpoint": format!("{}/authorize", base),
        "token_endpoint": format!("{}/token", base),
        // ...
        "scopes_supported": ["openid"],
        "code_challenge_methods_supported": ["S256"],
    });
    if with_reg {
        v["registration_endpoint"] = json!(format!("{}/register", base));
    }
    v.to_string()
}
```

> For AUTH-01's **anchor** test (Pitfall 1) the fixture must be able to LIE: parameterize
> `discovery_body` so the document's `"issuer"` can differ from `base`. A suite where every fixture
> sets `issuer == base` cannot distinguish "validated" from "not validated."

**Wire-body assertion pattern** (`:88-101`) — the mock only matches when the body contains the
required fields, so a regression that drops one produces a 501 and fails the test. This is the exact
mechanism AUTH-02 needs for `application_type`:

```rust
    let _r = server
        .mock("POST", "/register")
        .match_body(Matcher::PartialJsonString(
            json!({
                "grant_types": ["authorization_code"],
                "token_endpoint_auth_method": "none",
                "response_types": ["code"],
            })
            .to_string(),
        ))
        .with_status(200)
        .with_body(json!({"client_id": "x"}).to_string())
        .create_async()
        .await;
```

**Negative-control / "never redeemed" pattern** (`:117-172`) — an `expect(0)` mock plus
`assert_async()`. **This is the exact idiom for AUTH-01's "the authorization code is never
redeemed on `iss` mismatch"**: point it at `/token` instead of `/register`:

```rust
    // Guard: expect ZERO calls to any /register path on our mock server
    // (the SDK must not even attempt the POST).
    let reg_guard = server
        .mock("POST", "/register")
        .expect(0)
        .create_async()
        .await;
    // ... drive the flow, assert the error ...
    let msg = format!("{err}");
    assert!(
        msg.contains("must be https"),
        "expected scheme-guard error, got: {msg}"
    );
    reg_guard.assert_async().await;
```

> Prefer asserting `err.is_iss_mismatch()` (the D-03 marker predicate) over
> `msg.contains(...)` — that stable programmatic discriminator is the whole reason D-03 exists.
> Keep a message assertion only as a secondary, human-readability check.

---

### `tests/oauth_iss_validation.rs` etc. (unit + property over the pure tier)

**Analog:** `tests/pkce_helper.rs` — the ALWAYS-coverage file for the sibling pure module. Copy its
header (which enumerates the validation rows it covers), its RFC-vector-first ordering, its
crate-root-re-export check, and its `proptest!` block.

**Header pattern** (`tests/pkce_helper.rs:1-15`):

```rust
//! ALWAYS coverage for the wasm-safe PKCE crypto helper (`pmcp::shared::pkce`).
//!
//! Covers the four WEBCH-01 validation rows:
//!   1. `pkce_rfc7636_vector`        — RFC 7636 Appendix B published vector (correctness)
//!   2. `pkce_verifier_charset_len`  — every verifier is 43 chars, base64url-no-pad charset
//!   3. `pkce_challenge_deterministic` — same verifier always yields the same S256 challenge
//!   4. `pkce_base64url_roundtrip`   — base64url encode→decode is identity and never panics
//!
//! Tests reference the helper through its public re-export path
//! (`pmcp::shared::pkce::*` plus the crate-root convenience re-export) so they
//! also exercise the public API surface shipped in this release.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use pmcp::shared::pkce::{code_challenge_s256, generate_code_verifier, generate_state};
use proptest::prelude::*;
```

**Public-surface check** (`:26-34`) — proves the crate-root re-export resolves to the same item:

```rust
/// The crate-root re-export resolves to the same helper as the module path.
#[test]
fn pkce_crate_root_reexport_resolves() {
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    assert_eq!(
        pmcp::code_challenge_s256(verifier),
        code_challenge_s256(verifier),
    );
}
```

**Property block with threat tags and actionable failure messages** (`:36-54`):

```rust
proptest! {
    /// (2) Charset + length: every generated verifier is exactly 43 characters
    /// and uses only the base64url-no-pad unreserved charset `[A-Za-z0-9_-]`.
    /// A degenerate RNG (or a wrong byte count) is detectable here (T-103-RNG).
    #[test]
    fn pkce_verifier_charset_len(_seed in any::<u64>()) {
        let verifier = generate_code_verifier().expect("CSPRNG available on host");
        prop_assert_eq!(verifier.len(), 43);
        prop_assert!(
            verifier.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "verifier must use only base64url-no-pad unreserved chars: {}",
            verifier
        );
    }
```

> **Phase 115's discipline applies here (RESEARCH):** prefer invariants **derived from the RFC** over
> invariants restated from the implementation. The RFC 9207 no-normalization sentence ("clients MUST
> NOT apply scheme or host case folding, default-port elision, trailing-slash, or percent-encoding
> normalization") is the ideal generator spec: generate an issuer, apply each of those four
> normalizations, and assert **mismatch** every time.

**Store-test analog** (`cargo-pmcp/.../cache.rs:268-469`) for `tests/oauth_credential_store.rs`:
`tempfile::tempdir()` per test, a `write_sets_0600_perms_on_unix` permission assertion (`:343-352`),
and a `proptests` module (`:413-468`) whose generator comment explains *why* the input domain is
restricted — a habit worth copying.

---

### `fuzz/fuzz_targets/…` (fuzz target, ALWAYS requirement)

**Analog:** `fuzz/fuzz_targets/pkce_helper.rs` — a registration-only target that adds **no
dependency**, reaching the pure module through the `pmcp` dep already in `fuzz/Cargo.toml`:

```rust
//! Fuzz target for `pmcp::shared::pkce` — the wasm-safe PKCE crypto helper.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run pkce_helper` (plain form,
//! no `+nightly` — matches the repo Makefile `test-fuzz` target, LOW-7).
//!
//! Invariant: the verifier → S256 challenge → base64url-decode roundtrip must
//! NEVER panic on arbitrary input bytes. Error paths are acceptable; panics are
//! not. ... (threat T-103-PKCE — no-panic on arbitrary verifier bytes).

#![no_main]

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use libfuzzer_sys::fuzz_target;
use pmcp::shared::pkce::code_challenge_s256;

fuzz_target!(|data: &[u8]| {
    let verifier = URL_SAFE_NO_PAD.encode(data);
    let challenge = code_challenge_s256(&verifier);
    let _ = URL_SAFE_NO_PAD.decode(challenge.as_bytes());
});
```

**Registration pattern** (`fuzz/Cargo.toml`, tail) — the `[[bin]]` stanza carries a comment naming
the phase, the requirement, and the "adds NO dependency" claim:

```toml
# Phase 115-09 (SCHM-01): the era-branched `outputSchema` validation path. ...
# Registration only: this target adds NO dependency, it
# reaches `output_validation::fuzz_support` through the `fuzzing` + `validation`
# features already enabled on the `pmcp` dependency above.
[[bin]]
name = "fuzz_schema_draft_pin"
path = "fuzz_targets/fuzz_schema_draft_pin.rs"
test = false
doc = false
bench = false
```

Natural fuzz surfaces for this phase: `validate_authorization_response` over arbitrary query
strings (a hostile AS controls every byte of the callback URL), and `discovery_url_candidates`
over arbitrary issuer strings. `fuzz/fuzz_targets/auth_flows.rs` (11.8K) and
`dcr_response_parser.rs` already exist — extend where the surface fits rather than minting a new
target.

---

### `examples/cNN_…` (example, ALWAYS requirement)

**Analog:** `examples/c08_oauth_dcr.rs` — the DCR sibling. Note its shape: `#![cfg(feature =
"oauth")]`, an explicit "does NOT require network access" claim, a run command in the module doc,
and a construct-then-narrate body that exits without I/O. That makes it safe under
`make test-examples` (which only *builds*).

```rust
//! Example: Dynamic Client Registration (RFC 7591) with `OAuthHelper`.
//! ...
//! Run with:
//!   cargo run --example c08_oauth_dcr --features oauth
//!
//! This example does NOT require network access: it constructs the
//! `OAuthConfig`, prints what DCR would do, and exits. A live end-to-end
//! invocation requires a real MCP server with a `registration_endpoint`.

#![cfg(feature = "oauth")]

use pmcp::client::oauth::{OAuthConfig, OAuthHelper};

fn main() -> Result<(), Box<dyn std::error::Error>> {
```

> An `iss`/`state` validation example can do better than narrate: because the validation is a
> **pure function**, the example can actually *run* both the accept and the reject path with no
> network — which satisfies the ALWAYS "runnable example" obligation for real rather than by
> compilation alone.

---

### wasm32 build fence (config, D-06)

**Analog:** `Makefile:58-62`:

```make
# WASM build targets
.PHONY: wasm-build
wasm-build:
	@echo "$(BLUE)Building for WASM target (wasm32-unknown-unknown)...$(NC)"
	$(CARGO) build --target wasm32-unknown-unknown --no-default-features --features wasm
	@echo "$(GREEN)✓ WASM build complete$(NC)"
```

The fence D-06 needs is this target passing **after** the new module lands — i.e. the pure tier
compiles for wasm32 with `--no-default-features --features wasm` (no `oauth`, no `reqwest`, no
`dirs`, no `webbrowser`). Add `rustup target add wasm32-unknown-unknown` as an explicit setup step
(RESEARCH A5: the target was not probed).

---

### cargo-pmcp `auth` subcommands (controller, CRUD, D-19)

**Analog:** the subcommands as they stand. They already have the exact wrapper shape D-19 wants; the
change is *which* store they call.

**Import + normalize + read/mutate/write pattern** (`logout.rs:1-10, 26-58`) — `auth logout`
semantics that must be preserved: no-args errors, `--all` clears, positional URL removes one,
missing key is a no-op with a friendly message:

```rust
use crate::commands::auth_cmd::cache::{
    default_multi_cache_path, normalize_cache_key, TokenCacheV1,
};

pub async fn execute(args: LogoutArgs, global_flags: &GlobalFlags) -> Result<()> {
    if args.url.is_none() && !args.all {
        anyhow::bail!("specify a server URL or --all to log out of everything");
    }

    let cache_path = default_multi_cache_path();
    let mut cache = TokenCacheV1::read(&cache_path)?;

    if args.all {
        let count = cache.entries.len();
        cache.entries.clear();
        cache.write_atomic(&cache_path)?;
        // ...
    }

    let raw_url = args.url.as_deref().expect("url set (checked above)");
    let key = normalize_cache_key(raw_url)?;
    match cache.entries.remove(&key) {
```

**Config-construction pattern** (`login.rs:63-72`) — the `OAuthConfig` struct literal. Note this is
one of the **construction sites that makes `OAuthConfig` a semver landmine** (RESEARCH A1): adding a
field breaks it and every downstream caller. D-08's `Interactivity` must therefore be a **builder
method on `OAuthHelper`**, not a field on `OAuthConfig`:

```rust
    let config = OAuthConfig {
        issuer: args.oauth_issuer.clone(),
        mcp_server_url: Some(args.url.clone()),
        client_id: args.oauth_client_id.clone(),
        client_name,
        dcr_enabled: args.oauth_client_id.is_none(),
        scopes: scopes.clone(),
        cache_file: None,
        redirect_port: args.oauth_redirect_port,
    };
```

**Also a construction site:** `examples/c08_oauth_dcr.rs:22-31` and the `oauth.rs` module doctest at
`:1052-1061` — three in-repo `OAuthConfig` literals, plus every downstream user's.

---

## Shared Patterns

### Semver-safe extension (applies to EVERY struct this phase touches)

**Sources:** `src/client/oauth.rs:62-88` (`OAuthConfig`), `src/server/auth/provider.rs:302-351`
(`DcrRequest`), `src/server/auth/oauth2.rs:171-220` (`OidcDiscoveryMetadata`).
**Apply to:** all model/config edits.

All three are `pub`, **all-pub-field**, and **not** `#[non_exhaustive]`. Adding a field triggers
`constructible_struct_adds_field` = **MAJOR**. Phase 115's escape hatch (mark it `#[non_exhaustive]`
first) is unavailable — that is itself a major break.

```rust
// src/server/auth/oauth2.rs:171-176 — all-pub-field, no #[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcDiscoveryMetadata {
    /// Issuer identifier.
    pub issuer: String,
    /// Authorization endpoint URL.
    pub authorization_endpoint: String,
```

**Permitted extension shapes, all semver-minor:**

| Want | Use | In-repo precedent |
|------|-----|-------------------|
| New DCR wire field | inherent accessor over `#[serde(flatten)] extra` | `provider.rs:348-350` + `protocol/mod.rs:386-398` |
| New error identity | marker const + ctor + `is_*` on `Error::Protocol` | `error/mod.rs:126-131, 587-648` |
| New client knob | inherent builder method / new constructor fn | `client/auth.rs:106-112`; `oauth2.rs:169` type alias |
| New discovery signal (RFC 9207 flag) | **NOT** a field on `OidcDiscoveryMetadata` — new sibling type, new method returning `(metadata, extras)`, or keep the flag in the per-request record | RESEARCH A1 option (c) |

**Verification (must be an explicit plan step — it is not wired into CI or the Makefile):**

```bash
cargo semver-checks check-release -p pmcp --baseline-rev <phase-base-sha>
```

State the baseline in the command: against crates.io 2.17.0 there is a **pre-existing** failure
(a `#[deprecated]` on `OptimizedSseTransport`) that is not this phase's (Pitfall 9).

---

### Feature-gating and the wasm-clean tier

**Sources:** `src/client/mod.rs:46`, `Cargo.toml:216`, `src/shared/mod.rs:18-22`.
**Apply to:** every new module and every new public item.

- `src/client/oauth.rs` is `#[cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]`, and
  `oauth = ["http-client", "dep:webbrowser", "dep:dirs", "dep:rand"]` — a Workers platform gets
  **zero** of it.
- The pure tier goes in `src/shared/`, **ungated**, with the rationale comment from
  `src/shared/mod.rs:20-22` mirrored so a future contributor does not "tidy" a `cfg` onto it.
- `#[cfg(not(target_arch = "wasm32"))]` on individual helper fns is the in-repo precedent for
  splitting an otherwise-shared module (`generic_oidc.rs:391`, `cognito.rs:257`).

---

### Verification commands (Pitfalls 3/4/5 — every plan's verify block)

**Source:** `Makefile:150-160` (`lint`), `:210-214` (`test`), `:418-422` (`doc-check`).
**Apply to:** every plan in this phase.

```make
lint:
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) clippy --features "full" --lib --tests -- \
		-D clippy::all -W clippy::pedantic -W clippy::nursery -W clippy::cargo ...
test:
	$(CARGO) nextest run --features "full"
doc-check:
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps \
		--features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,...
```

- `full` does **not** include `oauth` ⇒ `make lint` and `make test` compile **zero** lines of
  `src/client/oauth.rs`. Always add `--features full,oauth` runs alongside.
- `--features oauth` **alone does not compile** at HEAD (4 errors in
  `examples/s51_v2_tasks_agent.rs`). Always `full,oauth`.
- `cargo nextest -E 'test(/name/)'` selects by **test name**, not file, and returns zero while
  exiting 0. Use `binary(name)` and assert a **non-zero count**.
- `make doc-check` is the **only** gate that compiles this phase's rustdoc (its feature list
  includes `oauth`), and it is red at HEAD with 28 pre-existing errors — measure a **delta**.

---

### Logging and secret hygiene

**Sources:** `src/shared/sse_optimized.rs:321-327` (refusal messages echo no body content);
`cargo-pmcp/.../cache.rs:35-37` ("Sensitive — NEVER logged"); `src/client/oauth.rs:1018-1021` (the
current violation).
**Apply to:** every new error path and every store impl.

- Refusal messages name the LIMIT and the observed size, never the refused bytes.
- Tokens are never logged raw. The platform convention (adopted as a design input) is a sha256
  prefix, enforceable by a mirrored static-source invariant test.
- On `iss` mismatch, AS-supplied `error`/`error_description`/`error_uri` must be **neither acted on
  nor displayed** (explicit spec MUST NOT).

---

### Zero-SATD and deferred work

**Source:** `Makefile:763-766` (`check-todos` is a **real** gate; `check-unwraps` at `:768-772` is a
stub).
**Apply to:** all source edits.

No `TODO`/`FIXME`/`HACK`/`XXX` in `src/`. Deferred work goes in the phase's `deferred-items.md`,
never in a source comment. Note the in-repo house style for "known gap, named owner" is a **written
allowlist entry with a `why`** (`tests/v2_bounded_reads_tripwire.rs:573-591`) or a deferred-items
section — both are enumerable; a comment is not.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| SEP-2351 ordered discovery-URL **probe sequence** | service | request-response | pmcp builds exactly **one** candidate everywhere (`client/auth.rs:137`, `generic_oidc.rs:391`, `cognito.rs:270`, `cargo-pmcp/.../cache.rs:194`). No in-repo code probes an ordered candidate list and falls through on 404. The closest structural analog is the retry loop at `client/auth.rs:142-159` (same `while` + `last_error` shape, different exit condition). The **candidate derivation** is pure `url::Url` arithmetic with no precedent — property-test it against the spec's worked examples rather than against a restated implementation rule. |
| D-08 `Interactivity::RefreshOnly` mode | service (state machine) | request-response | No in-repo "capability made unreachable by construction" precedent in the client. `AuthProvider::on_unauthorized`'s default-impl pattern (`shared/streamable_http.rs:1528-1538`) is the closest *semver* analog (new behavior, no existing caller changes), but the "browser path unreachable by construction" shape must be designed. Suggested: a typed mode selected at `OAuthHelper` construction so the interactive branch is not merely skipped but absent from the executed path. |

---

## Metadata

**Analog search scope:** `src/shared/`, `src/client/`, `src/server/auth/`, `src/error/`,
`src/types/`, `tests/`, `fuzz/fuzz_targets/`, `examples/`, `cargo-pmcp/src/commands/auth_cmd/`,
`Makefile`, `fuzz/Cargo.toml`

**Files read in full:** `src/shared/pkce.rs`, `src/shared/mod.rs`, `src/client/auth.rs`,
`tests/oauth_dcr_integration.rs`, `tests/pkce_helper.rs`, `fuzz/fuzz_targets/pkce_helper.rs`,
`examples/c08_oauth_dcr.rs`, `cargo-pmcp/src/commands/auth_cmd/cache.rs`

**Files read in targeted ranges:** `src/error/mod.rs` (28-177, 500-679),
`src/client/oauth.rs` (1-300, 420-539, 580-779, 900-1039),
`src/server/auth/provider.rs` (290-399), `src/server/auth/oauth2.rs` (160-229),
`src/shared/sse_optimized.rs` (255-344), `src/shared/streamable_http.rs` (515-560),
`tests/v2_bounded_reads_tripwire.rs` (1-130, 540-669), `src/types/protocol/mod.rs` (380-405),
`src/types/ui.rs` (455-515), `src/server/tool_middleware.rs` (115-140),
`src/server/observability/config.rs` (108-188), `Makefile` (55-75, 148-160, 208-216, 416-428),
`cargo-pmcp/src/commands/auth_cmd/{login,logout}.rs` (heads)

**Project skills checked:** `.claude/skills/` and `.agents/skills/` both contain only
`spike-findings-rust-mcp-sdk` (schema-server / SQL-dialect / skills-authoring topics). None
constrains OAuth or auth work — consistent with RESEARCH assumption A7. No skill rules loaded.

**Pattern extraction date:** 2026-08-02
