//! Server-owned `requestState` continuation tokens for multi-round-trip
//! requests (MRTR, MCP 2026-07-28).
//!
//! # What the token is
//!
//! A `requestState` token is a **self-contained continuation** (D-01): it carries
//! everything the server needs to resume a multi-round-trip operation, so the
//! server holds *nothing* between round trips. A stateless v2 server behind a load
//! balancer can therefore answer a follow-up on **any** instance — the client
//! carries the state, not the process.
//!
//! # Why AEAD and not sign-only
//!
//! The continuation is **encrypted**, not merely authenticated (D-02). Partially
//! collected tool arguments and the authenticated principal are inside it, and the
//! client — plus every proxy on the path — must not be able to read them. Sign-only
//! (an HMAC over cleartext) would authenticate the blob while leaking its contents.
//!
//! # The multi-instance thesis
//!
//! Every instance that shares `PMCP_REQUEST_STATE_KEY` can resume every other
//! instance's continuations (D-03). An instance that does *not* share the key
//! recognises the situation from the token's cleartext key-id and re-elicits
//! instead of erroring (D-04) — see [`Verdict::UnknownKey`].
//!
//! # Ownership: the codec belongs to the SERVER, not the process
//!
//! There is deliberately **no** process-global one-shot cell and **no** free
//! accessor function returning a `&'static` codec.
//! The codec is an `Arc<RequestStateCodec>` resolved exactly once at server
//! **build** time and stored on the server instance. That is what makes all four
//! of these possible at the same time:
//!
//! * builder configuration ([`crate::ServerBuilder::with_request_state_key`]),
//! * two differently-configured servers in one process,
//! * deterministic integration tests (an injected key **and** an injected clock),
//! * key rotation without stranding in-flight tokens.
//!
//! # Token layout
//!
//! ```text
//! base64url_nopad(
//!     key_id_len : u8            // always 8; a length byte so the layout can evolve
//!     key_id     : [u8; 8]       // first 8 bytes of SHA-256(key) — NON-SECRET, cleartext
//!     nonce      : [u8; 12]      // fresh CSPRNG bytes per mint
//!     sealed     : [..]          // CHACHA20_POLY1305(plaintext, aad) || tag[16]
//! )
//! ```
//!
//! * **plaintext** = `serde_json` of [`Continuation`] (`state`, `exp`, `round`).
//! * **aad** = `principal || 0x00 || method || 0x00 || salient_param_digest[32]`.
//!
//! # Algorithm choice
//!
//! `CHACHA20_POLY1305` rather than AES-256-GCM: it has no AES-NI timing dependence
//! (constant-time on every target `pmcp` builds for, including hosts without
//! hardware AES), and at these payload sizes the API and cost are identical.
//!
//! # Feature gating
//!
//! `D-14` locks MRTR AEAD to native + `streamable-http`. `ring` is only enabled by
//! that feature, and the wasm server (`WasmServerCore`) gets no MRTR this phase.
//!
//! # Property and fuzz coverage
//!
//! Per the repo's ALWAYS requirements, `requestState` carries both. The full
//! strength property sweep and the fuzz target are:
//!
//! ```text
//! PROPTEST_CASES=1000 cargo test --lib --features full -- property_request_state
//! cargo fuzz run fuzz_request_state -- -runs=20000
//! ```
//!
//! The fuzz target reaches [`RequestStateCodec::verify`] through
//! [`fuzz_support`], which exists only behind `feature = "fuzzing"` — a feature
//! deliberately absent from both `default` and `full`, so the seam adds nothing to
//! the shipped public API.

// Why: the `pub(crate)` markers are load-bearing, not redundant. Under
// `feature = "fuzzing"` this module is declared `pub mod` (so the fuzz target can
// reach `fuzz_support`), and `pub(crate)` is then the ONLY thing keeping the
// codec, its keys, `Continuation` and `Verdict` off the shipped public API.
// Downgrading them to bare `pub` as the lint suggests would also trip the
// crate-level `unreachable_pub` warn in the default (non-`fuzzing`) build.
#![allow(clippy::redundant_pub_crate)]

use crate::error::{Error, Result};
use crate::types::mrtr::CanonicalDepthExceeded;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305, NONCE_LEN};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use zeroize::Zeroize;

/// Length, in bytes, of the symmetric AEAD key.
pub(crate) const KEY_LEN: usize = 32;

/// Length of the cleartext key-id prefix, as the `u8` written on the wire.
const KEY_ID_LEN_U8: u8 = 8;

/// Length, in bytes, of the cleartext key-id prefix.
pub(crate) const KEY_ID_LEN: usize = KEY_ID_LEN_U8 as usize;

/// Default continuation lifetime in seconds (D-05).
pub(crate) const DEFAULT_TTL_SECS: u64 = 300;

/// Environment variable carrying the shared 32-byte minting key (D-03/D-04).
pub(crate) const ENV_REQUEST_STATE_KEY: &str = "PMCP_REQUEST_STATE_KEY";

/// Environment variable carrying a rotated-out key, accepted for **verification
/// only** so a rotation does not strand in-flight tokens.
pub(crate) const ENV_REQUEST_STATE_KEY_PREVIOUS: &str = "PMCP_REQUEST_STATE_KEY_PREVIOUS";

/// Environment variable overriding [`DEFAULT_TTL_SECS`] (D-05).
pub(crate) const ENV_REQUEST_STATE_TTL_SECS: &str = "PMCP_REQUEST_STATE_TTL_SECS";

/// The **one** type any pre-codec holder of `requestState` key material must use.
///
/// # Why this exists (D-113-P)
///
/// T-113-05 says raw key bytes never reach the allocator in the clear, and
/// [`RequestStateCodec`] honours that for every buffer it owns. The two SERVER
/// BUILDERS that feed it did not: `ServerCoreBuilder` and `ServerBuilder` each
/// held an `Option<[u8; KEY_LEN]>` plus a `Vec<[u8; KEY_LEN]>` of rotated-out
/// keys and dropped both un-scrubbed, so
/// `.with_request_state_key(k).build()` handed the AEAD key back to the
/// allocator readable by a core dump, an allocator reuse or a debugger.
///
/// # Why a wrapper and not a `Drop` impl on the builder
///
/// A struct-level `Drop` is incompatible with the by-value `mut self` builder
/// idiom: with `Drop`, moving any field out of `self` is `E0509`, so `build()`
/// could no longer destructure the builder. Putting the destructor on the
/// **value** instead scrubs on drop, survives moves, and changes nothing about
/// how the builders chain.
///
/// # What it does and does not buy
///
/// [`zeroize::Zeroizing`] runs [`Zeroize`] from its own `Drop`, and zeroize's
/// primitive is a volatile write followed by a `SeqCst` compiler fence — so the
/// overwrite is not dead-code-eliminated. It cannot recover copies the optimiser
/// made *before* the value reached this wrapper, nor bytes the OS paged out; it
/// bounds what the SDK itself leaves behind, which is the boundary the SDK owns.
pub(crate) type SecretKey = zeroize::Zeroizing<[u8; KEY_LEN]>;

// ---------------------------------------------------------------------------
// Environment access seam
// ---------------------------------------------------------------------------

/// Read a `PMCP_*` variable from the process environment.
///
/// Production always reads [`std::env::var`] directly (the house style). Under
/// `cfg(test)` a **thread-local** override is consulted first, so the env-behaviour
/// tests are deterministic under `cargo test`'s parallel thread pool instead of
/// mutating process-global state that a concurrently-building server would observe.
fn env_var(name: &str) -> Option<String> {
    #[cfg(test)]
    {
        let overridden: Option<Option<String>> =
            tests::ENV_OVERRIDE.with(|o| o.borrow().as_ref().map(|m| m.get(name).cloned()));
        if let Some(value) = overridden {
            return value;
        }
    }
    std::env::var(name).ok()
}

// ---------------------------------------------------------------------------
// Key identity
// ---------------------------------------------------------------------------

/// The non-secret, cleartext identifier for a `requestState` key.
///
/// It is the first [`KEY_ID_LEN`] bytes of `SHA-256(key)`. A pre-image of 8 bytes
/// of SHA-256 output leaks nothing usable about the 32-byte key, and the id is
/// carried in the clear on every token by design.
///
/// # Why it is load-bearing
///
/// The key-id is the **only** thing that distinguishes
/// "this token was minted by an instance whose per-process key I do not share"
/// (D-04 → strip MRTR fields and re-run the handler) from
/// "this token was tampered with" (conformance `sep-2322-reject-tampered-state`
/// → JSON-RPC error). Without it those two cases are indistinguishable and the
/// D-04 degraded path cannot exist.
///
/// # Collision policy
///
/// Eight bytes can collide. The accepting set is therefore a `Vec` and
/// [`RequestStateCodec::verify`] tries **every** entry whose key-id matches,
/// returning on the first successful AEAD open. [`Verdict::UnknownKey`] is
/// returned only when **no** entry's key-id matches. A colliding-but-different
/// key therefore yields [`Verdict::AuthFailed`] (the tag check fails on every
/// candidate) — never a false `Ok`, and never a misleading `UnknownKey`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeyId([u8; KEY_ID_LEN]);

impl KeyId {
    /// The raw id bytes, as written into the token prefix.
    pub(crate) const fn as_bytes(&self) -> &[u8; KEY_ID_LEN] {
        &self.0
    }
}

impl std::fmt::Display for KeyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for KeyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KeyId({self})")
    }
}

/// Derive the [`KeyId`] of a key: the first [`KEY_ID_LEN`] bytes of `SHA-256(key)`.
pub(crate) fn key_id_of(key: &[u8]) -> KeyId {
    let mut hasher = Sha256::new();
    hasher.update(key);
    let digest = hasher.finalize();
    let mut id = [0u8; KEY_ID_LEN];
    id.copy_from_slice(&digest[..KEY_ID_LEN]);
    KeyId(id)
}

// ---------------------------------------------------------------------------
// Clock
// ---------------------------------------------------------------------------

/// The clock [`RequestStateCodec`] reads for minting `exp` and for expiry checks.
///
/// Injectable so an expired token can be produced deterministically — with no
/// sleeping and no hand-crafted ciphertext — by moving the clock forward.
pub(crate) trait RequestStateClock: Send + Sync + std::fmt::Debug {
    /// Current time as Unix seconds.
    fn now_unix(&self) -> i64;
}

/// The production clock: wall-clock Unix seconds.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SystemClock;

impl RequestStateClock for SystemClock {
    fn now_unix(&self) -> i64 {
        chrono::Utc::now().timestamp()
    }
}

/// A clock pinned to a fixed instant, for deterministic expiry tests.
///
/// `#[cfg(test)]` rather than covered by a module-wide `allow(dead_code)`: it has
/// no production caller, and scoping it means a genuinely dead PRODUCTION item
/// still fails the `-D warnings` build instead of hiding behind a blanket allow.
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct FixedClock(pub i64);

#[cfg(test)]
impl RequestStateClock for FixedClock {
    fn now_unix(&self) -> i64 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Codec
// ---------------------------------------------------------------------------

/// The server-owned `requestState` mint/verify codec.
///
/// See the [module docs](self) for the ownership rationale, token layout and
/// algorithm choice.
pub(crate) struct RequestStateCodec {
    /// The single key new tokens are sealed with.
    minting: (KeyId, LessSafeKey),
    /// Every key a presented token may be opened with, including the minting key
    /// and any rotated-out (verify-only) keys.
    accepting: Vec<(KeyId, LessSafeKey)>,
    /// Continuation lifetime baked into each minted token's `exp`.
    ttl: Duration,
    /// Injectable clock (see [`RequestStateClock`]).
    clock: Arc<dyn RequestStateClock>,
}

impl std::fmt::Debug for RequestStateCodec {
    /// Renders **only** key ids and the ttl. Key material is never printed
    /// (threat T-113-05).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestStateCodec")
            .field("minting_key_id", &self.minting.0)
            .field("accepting_key_ids", &self.accepting_key_ids())
            .field("ttl", &self.ttl)
            .finish()
    }
}

/// Bind raw key bytes into a `ring` AEAD key plus its derived [`KeyId`].
fn bind_key(key: &[u8]) -> Result<(KeyId, LessSafeKey)> {
    if key.len() != KEY_LEN {
        return Err(Error::validation(format!(
            "a requestState key must be exactly {KEY_LEN} bytes, got {}",
            key.len()
        )));
    }
    let unbound = UnboundKey::new(&CHACHA20_POLY1305, key)
        .map_err(|_| Error::internal("ring rejected a 32-byte CHACHA20_POLY1305 key"))?;
    Ok((key_id_of(key), LessSafeKey::new(unbound)))
}

impl RequestStateCodec {
    /// Deterministic constructor: build a codec over one known key.
    ///
    /// This is the constructor tests, property tests, the fuzz seam and
    /// [`crate::ServerBuilder::with_request_state_key`] all go through — the
    /// codec never becomes ambient state.
    pub(crate) fn new(key: &[u8; KEY_LEN], ttl: Duration) -> Result<Self> {
        Self::from_key_slice(key, ttl)
    }

    /// Shared body of [`Self::new`] and [`Self::from_env`].
    fn from_key_slice(key: &[u8], ttl: Duration) -> Result<Self> {
        let minting = bind_key(key)?;
        let accepting = vec![minting.clone()];
        Ok(Self {
            minting,
            accepting,
            ttl,
            clock: Arc::new(SystemClock),
        })
    }

    /// Resolve the codec from the process environment (D-03/D-04).
    ///
    /// * `PMCP_REQUEST_STATE_KEY` set and valid → that key mints and verifies.
    /// * `PMCP_REQUEST_STATE_KEY` set and MALFORMED → `Err`. This is an operator
    ///   misconfiguration, and D-04's "no silent hard-error" fallback covers the
    ///   UNSET case only — silently substituting a random key for a key the
    ///   operator believed was shared would degrade security invisibly (T-113-17).
    /// * `PMCP_REQUEST_STATE_KEY` UNSET → a fresh per-process key plus exactly one
    ///   `tracing::warn!` naming the variable and the consequence.
    /// * `PMCP_REQUEST_STATE_KEY_PREVIOUS`, when set, joins the accepting set only.
    /// * `PMCP_REQUEST_STATE_TTL_SECS` overrides [`DEFAULT_TTL_SECS`]; an absent or
    ///   unparseable value falls back to it without erroring.
    ///
    /// Both the decoded key buffer and the `String` read out of the environment are
    /// zeroized once the `UnboundKey` exists (threat T-113-05).
    pub(crate) fn from_env() -> Result<Self> {
        let ttl = ttl_from_env();
        let codec = match env_var(ENV_REQUEST_STATE_KEY) {
            Some(raw) => Self::from_configured_key(&raw, ttl)?,
            None => Self::from_generated_key(ttl)?,
        };
        codec.with_env_previous_key()
    }

    /// Append the `PMCP_REQUEST_STATE_KEY_PREVIOUS` key to the accepting set, if
    /// one is configured.
    ///
    /// Split out of [`Self::from_env`] because a BUILDER-supplied minting key
    /// bypasses `from_env` entirely: the rotated-out key is an independent
    /// operator setting, and skipping it there silently stranded every in-flight
    /// continuation minted before the rotation (they degrade to
    /// [`Verdict::UnknownKey`] and re-elicit) — while
    /// [`resolve_codec_at_build`]'s own contract says the env value is honoured.
    fn with_env_previous_key(mut self) -> Result<Self> {
        if let Some(raw) = env_var(ENV_REQUEST_STATE_KEY_PREVIOUS) {
            self.accepting
                .push(bind_scrubbed(&raw, ENV_REQUEST_STATE_KEY_PREVIOUS)?);
        }
        Ok(self)
    }

    /// Build from an operator-configured key string, scrubbing both buffers.
    fn from_configured_key(raw: &str, ttl: Duration) -> Result<Self> {
        let mut scrubbed = raw.to_string();
        let decoded = decode_key_material(&scrubbed, ENV_REQUEST_STATE_KEY);
        scrubbed.zeroize();
        let mut decoded = decoded?;
        let built = Self::from_key_slice(&decoded, ttl);
        decoded.zeroize();
        built
    }

    /// The D-04 fallback: a fresh per-process key plus the startup warning.
    fn from_generated_key(ttl: Duration) -> Result<Self> {
        let mut key = random_key()?;
        let built = Self::from_key_slice(&key, ttl);
        key.zeroize();
        let codec = built?;
        tracing::warn!(
            env_var = ENV_REQUEST_STATE_KEY,
            key_id = %codec.minting.0,
            "PMCP_REQUEST_STATE_KEY is not set — generated a per-process requestState \
             key. Multi-round-trip requests whose follow-up lands on a DIFFERENT \
             instance behind a load balancer cannot be resumed and will be \
             re-elicited. Set PMCP_REQUEST_STATE_KEY to the SAME 32-byte \
             base64url (or hex) value on every instance to enable resumption."
        );
        Ok(codec)
    }

    /// Add verify-only keys to the accepting set.
    ///
    /// Entries are appended without de-duplication: two DIFFERENT keys may share a
    /// key-id (see [`KeyId`]'s collision policy), and collapsing them would turn a
    /// resolvable collision into a false `UnknownKey`.
    /// Every accepted item — including the ones AFTER a `bind_key` failure — is
    /// scrubbed before it drops: `bound` is deliberately held as a `Result` and
    /// `?`-ed only after `key.zeroize()`, so an early return cannot skip the
    /// scrub. This is the guarantee [`resolve_codec_at_build`] relies on when it
    /// hands over owned copies of its caller's keys.
    pub(crate) fn with_previous_keys(
        mut self,
        keys: impl IntoIterator<Item = [u8; KEY_LEN]>,
    ) -> Result<Self> {
        // `for mut key in` rather than `for key in` + a shadowing `let mut key`:
        // `[u8; KEY_LEN]` is `Copy`, so the shadow produced a SECOND stack slot
        // and `zeroize()` scrubbed only the shadow (D-113-P, copy discipline).
        for mut key in keys {
            let bound = bind_key(&key);
            key.zeroize();
            self.accepting.push(bound?);
        }
        Ok(self)
    }

    /// Replace the clock (see [`RequestStateClock`]). Test seam — see
    /// [`FixedClock`] for why these are `#[cfg(test)]` rather than blanket-allowed.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_clock(mut self, clock: Arc<dyn RequestStateClock>) -> Self {
        self.clock = clock;
        self
    }

    /// Replace the continuation lifetime.
    #[must_use]
    pub(crate) fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// The id of the key new tokens are minted under. Test seam.
    #[cfg(test)]
    pub(crate) const fn minting_key_id(&self) -> KeyId {
        self.minting.0
    }

    /// Every key-id a presented token may be opened under.
    pub(crate) fn accepting_key_ids(&self) -> Vec<KeyId> {
        self.accepting.iter().map(|(id, _)| *id).collect()
    }

    /// The configured continuation lifetime. Test seam.
    #[cfg(test)]
    pub(crate) const fn ttl(&self) -> Duration {
        self.ttl
    }

    /// The codec's current time, through its injected clock. Test seam.
    #[cfg(test)]
    pub(crate) fn now_unix(&self) -> i64 {
        self.clock.now_unix()
    }

    /// Force the minting key-id, so a key-id COLLISION can be constructed.
    ///
    /// SHA-256 pre-images cannot be chosen, so the collision branch of
    /// [`KeyId`]'s documented policy is otherwise untestable.
    #[cfg(test)]
    fn with_forced_minting_key_id(mut self, id: KeyId) -> Self {
        self.minting.0 = id;
        if let Some(first) = self.accepting.first_mut() {
            first.0 = id;
        }
        self
    }

    /// Append an accepting entry under a FORCED key-id (see
    /// [`Self::with_forced_minting_key_id`]).
    #[cfg(test)]
    fn with_forced_accepting_key(mut self, id: KeyId, key: &[u8; KEY_LEN]) -> Result<Self> {
        let (_, bound) = bind_key(key)?;
        self.accepting.push((id, bound));
        Ok(self)
    }
}

// ---------------------------------------------------------------------------
// Continuation, binding and verdict
// ---------------------------------------------------------------------------

/// The sealed payload of a `requestState` token.
///
/// Everything here is CONFIDENTIAL: `state` routinely holds partially collected
/// tool arguments, and `round` is a security counter (D-09). The client sees only
/// the opaque base64url token.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Continuation {
    /// Opaque server-authored continuation state.
    pub state: serde_json::Value,
    /// Absolute expiry, Unix seconds.
    pub exp: i64,
    /// Which multi-round-trip round this continuation belongs to (D-09).
    pub round: u8,
    /// The server's OWN record of which [`InputRequestKind`] it requested under
    /// each `inputRequests` key on the round this continuation was minted for
    /// (D-113-O).
    ///
    /// # What it is
    ///
    /// Built at mint time from [`InputRequest::kind`](crate::types::mrtr::InputRequest::kind)
    /// over the handler's own `inputRequests` map — never from anything the
    /// client sent. It lets ingress decode the answers KIND-DIRECTED
    /// ([`InputResponse::decode_for`](crate::types::mrtr::InputResponse::decode_for))
    /// instead of guessing from overlapping untagged shapes, which is what let a
    /// wrongly-shaped answer be silently reclassified into a variant the handler
    /// would never match.
    ///
    /// # Why it lives in the SEALED payload
    ///
    /// It must be UNFORGEABLE: the whole point is that the server enforces
    /// against what IT asked for, so a client that could choose the kinds map
    /// would simply relabel its answer and be back where D-113-O started.
    /// Sealing it costs nothing in confidentiality — the client was told the
    /// kinds in the previous round's `inputRequests`, so it already knows them —
    /// and buys integrity, which is the property that matters.
    ///
    /// # Absent means PRE-KINDS, and empty means "asked for nothing"
    ///
    /// The field is an `Option` rather than a bare map, and the distinction is
    /// load-bearing:
    ///
    /// | Value | Meaning | Ingress behaviour |
    /// |-------|---------|-------------------|
    /// | `None` (the `#[serde(default)]`) | minted by a build that predates this field | DEGRADE to the untagged decode |
    /// | `Some(map)`, non-empty | this build minted it and asked for these kinds | ENFORCE kind-directed |
    /// | `Some(map)`, EMPTY | this build minted it and asked for NOTHING | ENFORCE — every answer is unsolicited |
    ///
    /// A bare map would conflate the first and third rows, and the third row is
    /// reachable in production: a handler may signal `input_required` with an
    /// empty `inputRequests`, and an "empty means degrade" rule would then accept
    /// arbitrary untagged answers on a round that requested none.
    ///
    /// # Why the `None` degradation is not a bypass
    ///
    /// A rejection would fail every in-flight token during a rolling deploy — a
    /// real availability cost. And an attacker cannot reach the degraded path
    /// anyway: it requires presenting a continuation whose AEAD tag verifies, and
    /// only a holder of the server's key can mint one. The only party who can
    /// produce a kinds-less continuation is a previous build of this same server.
    #[serde(default)]
    pub kinds: Option<crate::types::mrtr::InputRequestKinds>,
}

/// The three values a `requestState` token is cryptographically bound to.
///
/// This struct is the ONLY way an AAD is composed, so [`RequestStateCodec::mint`]
/// and [`RequestStateCodec::verify`] can never disagree about its layout.
///
/// The three bindings are exactly the spec's replay-prevention clauses:
///
/// | Clause | Binding | Enforced or refused |
/// |--------|---------|---------------------|
/// | 5a — authenticated principal | `principal` (from `AuthContext::subject`) | enforced |
/// | 5b — short expiry | the ttl baked into [`Continuation::exp`] | enforced |
/// | 5c — identifier for the originating request | `method` + `param_digest` | enforced, or the request is REFUSED |
///
/// The 5c row is a two-state row on purpose (D-113-M). `param_digest` cannot always
/// be computed: params nested past the canonicalizer's depth cap have no digest that
/// identifies them uniquely. There is no third state in which a binding exists with
/// a digest that does not identify its request — [`Self::from_request`] returns
/// `Err` instead, and its callers refuse.
#[derive(Debug, Clone)]
pub(crate) struct RequestBinding<'a> {
    /// The authenticated principal — `AuthContext::subject`, never a
    /// client-asserted identity.
    pub principal: &'a str,
    /// The JSON-RPC method the token was minted for.
    pub method: &'a str,
    /// [`crate::types::mrtr::salient_param_digest`] over the originating params.
    pub param_digest: [u8; 32],
}

impl<'a> RequestBinding<'a> {
    /// Compose a binding from a live request.
    ///
    /// Fails with [`CanonicalDepthExceeded`] when the params nest past the
    /// canonicalizer's depth cap and therefore have no digest that identifies THIS
    /// request rather than a class of them (D-113-M). This is the ONLY constructor,
    /// which is what forces every caller — mint site, verify site, test seam and
    /// fuzz seam alike — to decide explicitly what an unidentifiable request means
    /// to it, instead of inheriting a silently aliasing digest.
    pub(crate) fn from_request(
        principal: &'a str,
        method: &'a str,
        params: &serde_json::Value,
    ) -> std::result::Result<Self, CanonicalDepthExceeded> {
        Ok(Self {
            principal,
            method,
            param_digest: crate::types::mrtr::salient_param_digest(method, params)?,
        })
    }

    /// The AEAD additional authenticated data:
    /// `principal || 0x00 || method || 0x00 || param_digest[32]`.
    ///
    /// # Why the plain concatenation is unambiguous
    ///
    /// The trailing 32 bytes are a fixed-length digest and the byte before them is
    /// a `0x00` separator, so `principal || 0x00 || method` is recovered exactly.
    /// Every method that can MINT is drawn from
    /// [`crate::types::mrtr::MRTR_METHODS`], all of which are NUL-free, so
    /// `method` is unambiguously the segment after the LAST `0x00` and `principal`
    /// is everything before it. Belt and braces: `param_digest` itself hashes the
    /// method name, so even a contrived concatenation collision would still have to
    /// agree on the method.
    fn aad(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            self.principal.len() + self.method.len() + 2 + self.param_digest.len(),
        );
        out.extend_from_slice(self.principal.as_bytes());
        out.push(0x00);
        out.extend_from_slice(self.method.as_bytes());
        out.push(0x00);
        out.extend_from_slice(&self.param_digest);
        out
    }
}

/// The outcome of presenting a `requestState` token — locked by D-15.
///
/// `verify` deliberately never returns a `Result`, so no caller can accidentally
/// `?` a security decision into a 500.
///
/// | Condition | Verdict | Caller behaviour (plan 06 wires it) |
/// |-----------|---------|-------------------------------------|
/// | oversized, not base64url, or too short to hold key-id + nonce | [`Verdict::AuthFailed`] | JSON-RPC error |
/// | no accepting entry's key-id matches | [`Verdict::UnknownKey`] | strip MRTR fields and RE-RUN the handler (D-04 degraded path) |
/// | a key-id matches but the AEAD tag fails on every matching entry | [`Verdict::AuthFailed`] | JSON-RPC error (`sep-2322-reject-tampered-state`) |
/// | decrypts, `exp` in the past | [`Verdict::Expired`] | re-elicit cleanly, PRESERVING `continuation.round` (D-15/D-05) |
/// | decrypts, `exp` in the future | [`Verdict::Ok`] | resume |
///
/// # No discrimination oracle
///
/// A wrong principal, a token replayed onto a different method, and a token
/// replayed onto different salient arguments ALL surface as
/// [`Verdict::AuthFailed`] rather than as distinct verdicts — those values live in
/// the AAD, so they fail `ring`'s constant-time tag check rather than an
/// application-level comparison (T-113-10). That is deliberate: it removes an
/// oracle, and it means no auxiliary `==` over a principal or a digest is needed
/// anywhere in this module.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Verdict {
    /// Authentic and live — resume from the carried continuation.
    Ok(Continuation),
    /// Authentic but past its `exp`. Carries the DECRYPTED continuation: the tag
    /// check already passed, so the plaintext is available, and plan 06 needs
    /// `round` to re-elicit cleanly without resetting a client's D-09 bound
    /// (T-113-49).
    Expired(Continuation),
    /// No accepting key-id matched — most likely another instance's per-process
    /// key (D-04). Carries nothing secret: it says only "not my key".
    UnknownKey,
    /// Authentication failed: tampered, wrong principal, or replayed onto a
    /// different request.
    AuthFailed,
}

/// The three cleartext fields split off the front of a decoded token.
struct TokenParts<'a> {
    key_id: KeyId,
    nonce: [u8; NONCE_LEN],
    sealed: &'a [u8],
}

/// Length-bound and base64url-decode a presented token.
fn decode_token(token: &str) -> Option<Vec<u8>> {
    if token.is_empty() || token.len() > crate::types::mrtr::MAX_REQUEST_STATE_LEN {
        return None;
    }
    URL_SAFE_NO_PAD.decode(token.as_bytes()).ok()
}

/// Split the cleartext key-id and nonce off a decoded token.
///
/// Every malformed shape returns `None` (which the caller renders as
/// [`Verdict::AuthFailed`]) rather than panicking — the input is
/// attacker-controlled by spec (T-113-14).
fn split_key_id(raw: &[u8]) -> Option<TokenParts<'_>> {
    let (&declared_len, rest) = raw.split_first()?;
    if usize::from(declared_len) != KEY_ID_LEN {
        return None;
    }
    if rest.len() <= KEY_ID_LEN + NONCE_LEN {
        return None;
    }
    let (id_bytes, rest) = rest.split_at(KEY_ID_LEN);
    let (nonce_bytes, sealed) = rest.split_at(NONCE_LEN);
    let mut key_id = [0u8; KEY_ID_LEN];
    key_id.copy_from_slice(id_bytes);
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(nonce_bytes);
    Some(TokenParts {
        key_id: KeyId(key_id),
        nonce,
        sealed,
    })
}

/// Open one sealed payload under one candidate key.
///
/// `open_in_place` mutates its buffer, so each candidate gets its own copy.
fn open_sealed(
    key: &LessSafeKey,
    nonce: [u8; NONCE_LEN],
    aad: &[u8],
    sealed: &[u8],
) -> Option<Continuation> {
    let mut buffer = sealed.to_vec();
    let opened = key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(aad),
            &mut buffer,
        )
        .ok()
        .and_then(|plaintext| serde_json::from_slice::<Continuation>(plaintext).ok());
    // The decrypted buffer holds the CONFIDENTIAL continuation (routinely
    // partially collected tool arguments). Scrub it for the same T-113-05 reason
    // the key buffers are scrubbed, rather than leaving it for the allocator.
    buffer.zeroize();
    opened
}

impl RequestStateCodec {
    /// Seal a continuation into an opaque `requestState` token.
    ///
    /// A fresh 12-byte nonce is drawn per call, `exp` is
    /// `clock.now_unix() + ttl`, and the seal uses the MINTING key only — a
    /// rotated-out key verifies but never mints.
    ///
    /// `kinds` is an explicit parameter rather than an `Option` defaulted inside,
    /// for the same reason [`RequestBinding::from_request`] is the only binding
    /// constructor (D-113-M): it forces every mint site to DECIDE whether it can
    /// state which kinds were requested. Passing `None` opts that token into the
    /// untagged-decode degradation documented on [`Continuation::kinds`], so it
    /// must be a choice, never an omission.
    ///
    /// # Errors
    ///
    /// Returns an error if the CSPRNG fails, if the continuation is not
    /// serializable, if `ring` refuses to seal, or if the resulting token would
    /// exceed [`crate::types::mrtr::MAX_REQUEST_STATE_LEN`] — the server must never
    /// mint a token it would itself reject at ingress. That last check is what
    /// bounds the `kinds` map: a map large enough to push the token past the
    /// accepted bound is REFUSED at the mint rather than emitted.
    pub(crate) fn mint(
        &self,
        state: &serde_json::Value,
        binding: &RequestBinding<'_>,
        round: u8,
        kinds: Option<crate::types::mrtr::InputRequestKinds>,
    ) -> Result<String> {
        let ttl_secs = i64::try_from(self.ttl.as_secs()).unwrap_or(i64::MAX);
        let continuation = Continuation {
            state: state.clone(),
            exp: self.clock.now_unix().saturating_add(ttl_secs),
            round,
            kinds,
        };
        let mut sealed = serde_json::to_vec(&continuation).map_err(|e| {
            Error::internal(format!(
                "requestState continuation is not serializable: {e}"
            ))
        })?;
        // `seal_in_place_append_tag` APPENDS the 16-byte tag. Without headroom
        // that push reallocates, and the old allocation — still holding the
        // CONFIDENTIAL plaintext continuation — is handed back to the allocator
        // unscrubbed, with no handle left to zeroize it (T-113-05). Reserving the
        // tag up front keeps the seal genuinely in place, so the only copy of the
        // plaintext is the buffer that is overwritten with ciphertext.
        sealed.reserve(CHACHA20_POLY1305.tag_len());

        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce)
            .map_err(|e| Error::internal(format!("CSPRNG (getrandom) failed: {e}")))?;

        let aad = binding.aad();
        self.minting
            .1
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(aad.as_slice()),
                &mut sealed,
            )
            .map_err(|_| Error::internal("requestState AEAD sealing failed"))?;

        let mut raw = Vec::with_capacity(1 + KEY_ID_LEN + NONCE_LEN + sealed.len());
        raw.push(KEY_ID_LEN_U8);
        raw.extend_from_slice(self.minting.0.as_bytes());
        raw.extend_from_slice(&nonce);
        raw.extend_from_slice(&sealed);

        let token = URL_SAFE_NO_PAD.encode(&raw);
        if token.len() > crate::types::mrtr::MAX_REQUEST_STATE_LEN {
            return Err(Error::validation(format!(
                "minted requestState token is {} bytes, over the {} byte accepted \
                 bound — the continuation state is too large to be self-contained",
                token.len(),
                crate::types::mrtr::MAX_REQUEST_STATE_LEN
            )));
        }
        Ok(token)
    }

    /// Verify a presented token against the binding of the CURRENT request.
    ///
    /// Never returns a `Result`: every failure mode is a [`Verdict`], so a caller
    /// cannot accidentally `?` a security decision into a 500. See [`Verdict`] for
    /// the D-15 decision table.
    pub(crate) fn verify(&self, token: &str, binding: &RequestBinding<'_>) -> Verdict {
        let Some(raw) = decode_token(token) else {
            return Verdict::AuthFailed;
        };
        let Some(parts) = split_key_id(&raw) else {
            return Verdict::AuthFailed;
        };
        if !self.has_candidate_key(parts.key_id) {
            // Not tampering — most likely another instance's per-process key.
            return Verdict::UnknownKey;
        }
        let aad = binding.aad();
        for (candidate, key) in &self.accepting {
            if *candidate != parts.key_id {
                continue;
            }
            if let Some(continuation) = open_sealed(key, parts.nonce, &aad, parts.sealed) {
                return self.check_expiry(continuation);
            }
        }
        Verdict::AuthFailed
    }

    /// Whether any accepting key has this key-id (see [`KeyId`]'s collision policy).
    ///
    /// `accepting` holds 1–3 entries, so the linear scan is the right structure —
    /// but answering this with a `collect()` into a `Vec` allocated once per verify,
    /// on the hot path, purely to call `is_empty()`. The loop above re-filters
    /// instead, which is the same short-circuit with no allocation.
    fn has_candidate_key(&self, id: KeyId) -> bool {
        self.accepting.iter().any(|(candidate, _)| *candidate == id)
    }

    /// Classify an opened continuation as live or expired.
    fn check_expiry(&self, continuation: Continuation) -> Verdict {
        if continuation.exp <= self.clock.now_unix() {
            Verdict::Expired(continuation)
        } else {
            Verdict::Ok(continuation)
        }
    }
}

/// Decode + bind one configured key string, scrubbing both buffers.
fn bind_scrubbed(raw: &str, var: &str) -> Result<(KeyId, LessSafeKey)> {
    let mut scrubbed = raw.to_string();
    let decoded = decode_key_material(&scrubbed, var);
    scrubbed.zeroize();
    let mut decoded = decoded?;
    let bound = bind_key(&decoded);
    decoded.zeroize();
    bound
}

/// Decode configured key material: base64url-no-pad **or** hex, exactly
/// [`KEY_LEN`] bytes after decoding.
///
/// Length disambiguates the two encodings: a 64-character hex string is also
/// valid base64url, but decodes to 48 bytes rather than 32, so it falls through
/// to the hex attempt. EVERY buffer this function decodes and does not return is
/// zeroized before it is dropped — including the attempts AFTER the accepted one.
/// Returning early out of the loop dropped those unscrubbed, which is exactly the
/// key material T-113-05 says never reaches the allocator in the clear.
fn decode_key_material(raw: &str, var: &str) -> Result<Vec<u8>> {
    let trimmed = raw.trim();
    let attempts = [
        URL_SAFE_NO_PAD.decode(trimmed.as_bytes()).ok(),
        decode_hex(trimmed),
    ];
    let mut accepted: Option<Vec<u8>> = None;
    for mut bytes in attempts.into_iter().flatten() {
        if accepted.is_none() && bytes.len() == KEY_LEN {
            accepted = Some(bytes);
            continue;
        }
        bytes.zeroize();
    }
    if let Some(bytes) = accepted {
        return Ok(bytes);
    }
    Err(Error::validation(format!(
        "{var} must decode to exactly {KEY_LEN} bytes as base64url-no-pad or hex; \
         the configured value does not. Generate one with: \
         `head -c 32 /dev/urandom | base64 | tr '+/' '-_' | tr -d '='`"
    )))
}

/// Decode a lowercase/uppercase hex string, or `None` if it is not valid hex.
fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if s.is_empty() || !s.len().is_multiple_of(2) || !s.is_ascii() {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    // `as_chunks::<2>()` rather than `chunks_exact(2)`: the const-generic form
    // gives the compiler a fixed-size array per step instead of a slice whose
    // length it must re-prove, which is what `clippy::chunks_exact_to_as_chunks`
    // (new in Rust 1.98) asks for. The trailing remainder is discarded here
    // exactly as `chunks_exact` discarded it, and cannot be non-empty anyway:
    // the length was already checked to be a multiple of 2 above.
    let (pairs, _remainder) = s.as_bytes().as_chunks::<2>();
    for pair in pairs {
        let hi = char::from(pair[0]).to_digit(16)?;
        let lo = char::from(pair[1]).to_digit(16)?;
        out.push(u8::try_from(hi * 16 + lo).ok()?);
    }
    Some(out)
}

/// Draw a fresh 32-byte key from the CSPRNG.
///
/// Delegates to `crate::shared::pkce::random_bytes` rather than mirroring it. That
/// helper's own doc states it exists to centralise "the single `getrandom::fill`
/// call so both the verifier and the state generators share one CSPRNG source, and
/// so a `getrandom::Error` is mapped to [`Error::internal`] in exactly one place" —
/// a second copy here would have broken the invariant it was written to hold.
fn random_key() -> Result<[u8; KEY_LEN]> {
    crate::shared::pkce::random_bytes()
}

/// Resolve the configured ttl, defaulting on absent or unparseable.
fn ttl_from_env() -> Duration {
    env_var(ENV_REQUEST_STATE_TTL_SECS)
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .map_or_else(
            || Duration::from_secs(DEFAULT_TTL_SECS),
            Duration::from_secs,
        )
}

/// Resolve the server's codec **once**, at build time.
///
/// Returns `Ok(None)` — reading no environment variable and emitting no warning —
/// for a server that did not opt into the v2 (`2026-07-28`) era, so a v1-only
/// server pays nothing for MRTR (D-04's zero-era-code rule).
///
/// Precedence for an opted-in server:
/// 1. a builder-supplied key beats `PMCP_REQUEST_STATE_KEY` entirely;
/// 2. a builder-supplied ttl beats `PMCP_REQUEST_STATE_TTL_SECS`, which beats
///    [`DEFAULT_TTL_SECS`] (D-05 — configurable by env OR builder);
/// 3. builder-supplied previous keys are appended to whatever
///    `PMCP_REQUEST_STATE_KEY_PREVIOUS` already contributed.
///
/// An `Err` here is a BUILD failure by design: see [`RequestStateCodec::from_env`].
///
/// # Key material is taken BY REFERENCE (D-113-P, copy 3 of 3)
///
/// The key arrives as `Option<&SecretKey>`, not `Option<[u8; KEY_LEN]>`. The
/// by-value form meant that merely CALLING this function manufactured a fresh,
/// never-scrubbed stack copy of the caller's key — so even a perfectly zeroizing
/// builder still leaked one copy per `build()`. Do not widen it back.
pub(crate) fn resolve_codec_at_build(
    accept_list: &[crate::types::ProtocolVersion],
    key: Option<&SecretKey>,
    previous_keys: &[SecretKey],
    ttl: Option<Duration>,
) -> Result<Option<Arc<RequestStateCodec>>> {
    if !crate::types::protocol::context::is_v2_opted_in(accept_list) {
        return Ok(None);
    }
    let effective_ttl = ttl.unwrap_or_else(ttl_from_env);
    let codec = match key {
        // A builder key overrides `PMCP_REQUEST_STATE_KEY` — but NOT
        // `PMCP_REQUEST_STATE_KEY_PREVIOUS`, which is a separate rotation
        // setting this function's contract promises to honour either way.
        //
        // `explicit` auto-derefs THROUGH the zeroizing wrapper to the caller's
        // own bytes; no `[u8; KEY_LEN]` is copied out of it here.
        Some(explicit) => {
            RequestStateCodec::new(explicit, effective_ttl)?.with_env_previous_key()?
        },
        None => RequestStateCodec::from_env()?.with_ttl(effective_ttl),
    };
    // `with_previous_keys` takes an OWNING iterator, so this does copy each
    // rotated-out key once. That copy is not a leak: the loop above zeroizes
    // every item it receives before it drops, including on the error path. The
    // caller's own `Vec<SecretKey>` is untouched and scrubs itself on drop.
    let codec = codec.with_previous_keys(previous_keys.iter().map(|k| **k))?;
    Ok(Some(Arc::new(codec)))
}

/// Minimal seam for `fuzz/fuzz_targets/fuzz_request_state.rs`.
///
/// # ⚠️ Not stable API
///
/// This module exists only behind `feature = "fuzzing"`, which is in neither
/// `default` nor `full`, so `cargo public-api` never sees it on the shipped
/// surface. Do not depend on it.
#[cfg(feature = "fuzzing")]
pub mod fuzz_support {
    use super::{RequestBinding, RequestStateCodec, Verdict, DEFAULT_TTL_SECS, KEY_LEN};
    use std::time::Duration;

    /// Discriminant for [`super::Verdict::Ok`].
    pub const VERDICT_OK: u8 = 0;
    /// Discriminant for [`super::Verdict::Expired`].
    pub const VERDICT_EXPIRED: u8 = 1;
    /// Discriminant for [`super::Verdict::UnknownKey`].
    pub const VERDICT_UNKNOWN_KEY: u8 = 2;
    /// Discriminant for [`super::Verdict::AuthFailed`].
    pub const VERDICT_AUTH_FAILED: u8 = 3;
    /// No verdict could be reached: the codec could not be constructed, or the
    /// request could not be BOUND (its params exceed the canonicalization depth
    /// cap, D-113-M). Neither is expected for this target's FIXED params.
    ///
    /// Deliberately an existing discriminant rather than a new one. The fuzz
    /// target's invariant is "never `VERDICT_OK` for input `mint` did not produce";
    /// both conditions are "no verification happened at all", so folding them
    /// together cannot mask an `Ok`, and adding a discriminant would invalidate
    /// every archived crash artifact's expected output.
    pub const VERDICT_UNAVAILABLE: u8 = 4;

    /// A FIXED key, so the fuzz target is reproducible from a crash artifact.
    /// Deliberately not `from_env`, which would make replay depend on ambient
    /// process state.
    const FIXED_KEY: [u8; KEY_LEN] = [0x5a; KEY_LEN];

    /// Drive [`RequestStateCodec::verify`] with arbitrary bytes.
    ///
    /// Invariants the target asserts: this never panics, and it never returns
    /// [`VERDICT_OK`] for input that was not produced by
    /// [`RequestStateCodec::mint`].
    #[must_use]
    pub fn verify_bytes(input: &[u8]) -> u8 {
        let Ok(codec) = RequestStateCodec::new(&FIXED_KEY, Duration::from_secs(DEFAULT_TTL_SECS))
        else {
            return VERDICT_UNAVAILABLE;
        };
        // Lossy rather than a UTF-8 bail, so every arbitrary byte string still
        // reaches the decoder rather than being filtered out before it.
        let token = String::from_utf8_lossy(input);
        let params = serde_json::json!({ "name": "fuzz", "arguments": {} });
        // The params are FIXED and two levels deep, so the depth refusal is
        // unreachable here — handled explicitly rather than unwrapped, because a
        // panic in a fuzz target is a false crash artifact.
        let Ok(binding) = RequestBinding::from_request("fuzz-principal", "tools/call", &params)
        else {
            return VERDICT_UNAVAILABLE;
        };
        match codec.verify(&token, &binding) {
            Verdict::Ok(_) => VERDICT_OK,
            Verdict::Expired(_) => VERDICT_EXPIRED,
            Verdict::UnknownKey => VERDICT_UNKNOWN_KEY,
            Verdict::AuthFailed => VERDICT_AUTH_FAILED,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::{Arc as StdArc, Mutex};

    thread_local! {
        /// Thread-local `PMCP_*` override consulted by [`env_var`] under `cfg(test)`.
        pub(super) static ENV_OVERRIDE: RefCell<Option<HashMap<String, String>>> =
            const { RefCell::new(None) };
    }

    /// Serialises the one test that mutates the REAL process environment.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const KEY_A: [u8; KEY_LEN] = [0x11; KEY_LEN];
    const KEY_B: [u8; KEY_LEN] = [0x22; KEY_LEN];

    /// Run `f` with a thread-local view of the `PMCP_*` environment.
    ///
    /// Any variable NOT listed in `pairs` reads as absent, so a test that wants
    /// "unset" gets it deterministically regardless of the ambient environment.
    fn with_env<T>(pairs: &[(&str, &str)], f: impl FnOnce() -> T) -> T {
        /// RAII teardown: the repo runs `cargo test --test-threads=1`, where
        /// libtest executes every test on the SAME thread. A `f()` that panics
        /// (an assertion failure) would otherwise leave this thread's override
        /// installed and silently reshape every later env test.
        struct Restore;
        impl Drop for Restore {
            fn drop(&mut self) {
                ENV_OVERRIDE.with(|o| *o.borrow_mut() = None);
            }
        }

        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        ENV_OVERRIDE.with(|o| *o.borrow_mut() = Some(map));
        let _restore = Restore;
        f()
    }

    fn b64(key: &[u8]) -> String {
        URL_SAFE_NO_PAD.encode(key)
    }

    fn hex(key: &[u8]) -> String {
        use std::fmt::Write as _;
        key.iter().fold(String::new(), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
    }

    // -- WARN capture -------------------------------------------------------

    #[derive(Clone, Default)]
    struct WarnCounter {
        warns: StdArc<Mutex<Vec<String>>>,
    }

    struct MessageVisitor<'a>(&'a mut String);

    impl tracing::field::Visit for MessageVisitor<'_> {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            if field.name() == "message" {
                *self.0 = format!("{value:?}");
            }
        }
        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            if field.name() == "message" {
                *self.0 = value.to_string();
            }
        }
    }

    impl tracing::Subscriber for WarnCounter {
        fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            tracing::span::Id::from_u64(1)
        }
        fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}
        fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}
        fn event(&self, event: &tracing::Event<'_>) {
            if *event.metadata().level() != tracing::Level::WARN {
                return;
            }
            let mut message = String::new();
            event.record(&mut MessageVisitor(&mut message));
            if let Ok(mut warns) = self.warns.lock() {
                warns.push(message);
            }
        }
        fn enter(&self, _span: &tracing::span::Id) {}
        fn exit(&self, _span: &tracing::span::Id) {}
    }

    /// Run `f` with a WARN-counting subscriber installed, returning the captured
    /// WARN messages alongside `f`'s output.
    ///
    /// # Why the warm-up and the explicit cache rebuild
    ///
    /// `tracing` caches an `Interest` per callsite, computed the FIRST time that
    /// callsite executes, against the executing thread's dispatcher. Other tests in
    /// this suite build v2 servers with no `PMCP_REQUEST_STATE_KEY` and no
    /// subscriber, so whichever thread reaches the D-04 `warn!` first can cache it
    /// as `Interest::never()` — after which no scoped subscriber ever sees it, and
    /// this assertion fails intermittently depending on test-thread interleaving.
    ///
    /// `tracing_core::dispatcher::set_default` (the SCOPED form that
    /// `with_default` uses) deliberately does not rebuild that cache — only
    /// `set_global_default` does. So: register the callsite outside the capture
    /// scope, then rebuild the cache once this thread has a subscriber. Both steps
    /// are needed; a rebuild alone cannot reach a callsite that is not yet
    /// registered.
    fn capture_warns<T>(f: impl FnOnce() -> T) -> (T, Vec<String>) {
        let _warm_up = with_env(&[], RequestStateCodec::from_env);
        let counter = WarnCounter::default();
        let sink = counter.warns.clone();
        let out = tracing::subscriber::with_default(counter, || {
            tracing::callsite::rebuild_interest_cache();
            f()
        });
        let warns = sink.lock().map(|w| w.clone()).unwrap_or_default();
        (out, warns)
    }

    fn v2_versions() -> Vec<crate::types::ProtocolVersion> {
        vec![
            crate::types::ProtocolVersion("2026-07-28".to_string()),
            crate::types::ProtocolVersion("2025-11-25".to_string()),
        ]
    }

    // -- key resolution -----------------------------------------------------

    #[test]
    fn from_env_with_valid_base64url_key_succeeds_and_key_id_is_deterministic() {
        let codec = with_env(&[(ENV_REQUEST_STATE_KEY, &b64(&KEY_A))], || {
            RequestStateCodec::from_env().expect("a valid 32-byte key must be accepted")
        });
        assert_eq!(codec.minting_key_id(), key_id_of(&KEY_A));

        let again = with_env(&[(ENV_REQUEST_STATE_KEY, &b64(&KEY_A))], || {
            RequestStateCodec::from_env().expect("a valid 32-byte key must be accepted")
        });
        assert_eq!(
            codec.minting_key_id(),
            again.minting_key_id(),
            "key-id must be deterministic for a given key"
        );
    }

    #[test]
    fn from_env_accepts_a_hex_encoded_key() {
        let codec = with_env(&[(ENV_REQUEST_STATE_KEY, &hex(&KEY_A))], || {
            RequestStateCodec::from_env().expect("a hex 32-byte key must be accepted")
        });
        assert_eq!(codec.minting_key_id(), key_id_of(&KEY_A));
    }

    #[test]
    fn from_env_unset_generates_a_key_and_warns_exactly_once() {
        let (codec, warns) = capture_warns(|| with_env(&[], RequestStateCodec::from_env));
        assert!(codec.is_ok(), "an unset key must NOT fail the build (D-04)");
        assert_eq!(
            warns.len(),
            1,
            "exactly one WARN must be emitted, got {warns:?}"
        );
        assert!(
            warns[0].contains(ENV_REQUEST_STATE_KEY),
            "the WARN must name the env var: {}",
            warns[0]
        );
    }

    #[test]
    fn from_env_with_malformed_key_errors_naming_the_expected_length() {
        let err = with_env(&[(ENV_REQUEST_STATE_KEY, "not-a-valid-key")], || {
            RequestStateCodec::from_env()
                .expect_err("a malformed CONFIGURED key must fail, not silently fall back")
        });
        let rendered = err.to_string();
        assert!(
            rendered.contains("32"),
            "the error must name the expected byte length: {rendered}"
        );
        assert!(
            rendered.contains(ENV_REQUEST_STATE_KEY),
            "the error must name the offending variable: {rendered}"
        );
    }

    #[test]
    fn from_env_fallback_keys_are_distinct_across_calls() {
        let (a, b) = with_env(&[], || {
            (
                RequestStateCodec::from_env().expect("fallback key"),
                RequestStateCodec::from_env().expect("fallback key"),
            )
        });
        assert_ne!(
            a.minting_key_id(),
            b.minting_key_id(),
            "two CSPRNG fallback draws must not collide"
        );
    }

    #[test]
    fn from_env_reads_the_real_process_environment() {
        // The ONE test that touches process-global state. It sets a VALID key, so
        // even if a concurrently-building server observes it the build succeeds.
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = std::env::var(ENV_REQUEST_STATE_KEY).ok();
        std::env::set_var(ENV_REQUEST_STATE_KEY, b64(&KEY_B));
        let resolved = RequestStateCodec::from_env();
        match previous {
            Some(value) => std::env::set_var(ENV_REQUEST_STATE_KEY, value),
            None => std::env::remove_var(ENV_REQUEST_STATE_KEY),
        }
        assert_eq!(
            resolved
                .expect("valid key in the real env")
                .minting_key_id(),
            key_id_of(&KEY_B),
            "production must read std::env::var, not only the test seam"
        );
    }

    #[test]
    fn previous_key_is_accepted_for_verification_but_never_for_minting() {
        let codec = with_env(
            &[
                (ENV_REQUEST_STATE_KEY, &b64(&KEY_A)),
                (ENV_REQUEST_STATE_KEY_PREVIOUS, &b64(&KEY_B)),
            ],
            || RequestStateCodec::from_env().expect("both keys valid"),
        );
        assert_eq!(codec.minting_key_id(), key_id_of(&KEY_A));
        let accepting = codec.accepting_key_ids();
        assert!(accepting.contains(&key_id_of(&KEY_A)));
        assert!(accepting.contains(&key_id_of(&KEY_B)));
        assert_ne!(
            codec.minting_key_id(),
            key_id_of(&KEY_B),
            "the previous key must never become the minting key"
        );
    }

    #[test]
    fn ttl_env_overrides_the_default() {
        let codec = with_env(
            &[
                (ENV_REQUEST_STATE_KEY, &b64(&KEY_A)),
                (ENV_REQUEST_STATE_TTL_SECS, "42"),
            ],
            || RequestStateCodec::from_env().expect("valid key"),
        );
        assert_eq!(codec.ttl(), Duration::from_secs(42));
    }

    #[test]
    fn ttl_env_unparseable_falls_back_to_default_without_erroring() {
        let codec = with_env(
            &[
                (ENV_REQUEST_STATE_KEY, &b64(&KEY_A)),
                (ENV_REQUEST_STATE_TTL_SECS, "five minutes"),
            ],
            || RequestStateCodec::from_env().expect("an unparseable ttl must not error"),
        );
        assert_eq!(codec.ttl(), Duration::from_secs(DEFAULT_TTL_SECS));
    }

    #[test]
    fn key_id_is_the_first_eight_bytes_of_sha256() {
        let mut hasher = Sha256::new();
        hasher.update(KEY_A);
        let digest = hasher.finalize();
        assert_eq!(key_id_of(&KEY_A).as_bytes()[..], digest[..KEY_ID_LEN]);
    }

    #[test]
    fn debug_never_renders_key_material() {
        let codec = RequestStateCodec::new(&KEY_A, Duration::from_secs(45)).expect("valid key");
        let rendered = format!("{codec:?}");
        assert!(rendered.contains("ttl"));
        assert!(rendered.contains(&key_id_of(&KEY_A).to_string()));
        assert!(
            !rendered.contains(&hex(&KEY_A)),
            "key material must never appear in Debug output: {rendered}"
        );
    }

    #[test]
    fn fixed_clock_makes_now_deterministic() {
        let codec = RequestStateCodec::new(&KEY_A, Duration::from_secs(45))
            .expect("valid key")
            .with_clock(Arc::new(FixedClock(1_700_000_000)));
        assert_eq!(codec.now_unix(), 1_700_000_000);
        assert_eq!(codec.now_unix(), 1_700_000_000);
    }

    // -- server wiring ------------------------------------------------------

    #[test]
    fn server_builder_malformed_request_state_key_fails_the_build() {
        let result = with_env(&[(ENV_REQUEST_STATE_KEY, "bogus")], || {
            crate::server::Server::builder()
                .name("t")
                .version("1")
                .with_supported_protocol_versions(v2_versions())
                .build()
        });
        assert!(
            result.is_err(),
            "a malformed CONFIGURED key must fail the server build (T-113-17)"
        );
    }

    #[test]
    fn server_builder_unset_key_warns_once_at_startup() {
        let (server, warns) = capture_warns(|| {
            with_env(&[], || {
                crate::server::Server::builder()
                    .name("t")
                    .version("1")
                    .with_supported_protocol_versions(v2_versions())
                    .build()
            })
        });
        assert!(server.is_ok(), "an unset key must still serve (D-04)");
        assert_eq!(
            warns.len(),
            1,
            "the D-04 warning must be emitted exactly once, at BUILD time: {warns:?}"
        );
    }

    #[test]
    fn server_builder_with_request_state_key_overrides_env() {
        let server = with_env(&[(ENV_REQUEST_STATE_KEY, &b64(&KEY_A))], || {
            crate::server::Server::builder()
                .name("t")
                .version("1")
                .with_supported_protocol_versions(v2_versions())
                .with_request_state_key(KEY_B)
                .build()
                .expect("builder key must win")
        });
        assert_eq!(
            server
                .request_state_codec()
                .expect("v2 server has a codec")
                .minting_key_id(),
            key_id_of(&KEY_B)
        );
    }

    #[test]
    fn server_builder_with_request_state_ttl_overrides_default_and_env() {
        let server = with_env(
            &[
                (ENV_REQUEST_STATE_KEY, &b64(&KEY_A)),
                (ENV_REQUEST_STATE_TTL_SECS, "42"),
            ],
            || {
                crate::server::Server::builder()
                    .name("t")
                    .version("1")
                    .with_supported_protocol_versions(v2_versions())
                    .with_request_state_ttl(Duration::from_secs(7))
                    .build()
                    .expect("builder ttl must win")
            },
        );
        assert_eq!(
            server.request_state_codec().expect("codec").ttl(),
            Duration::from_secs(7)
        );
    }

    #[test]
    fn two_servers_with_different_keys_have_different_key_ids() {
        // The direct regression guard for the rejected process-global design:
        // two differently-configured servers must coexist in ONE process.
        let first = crate::server::Server::builder()
            .name("a")
            .version("1")
            .with_supported_protocol_versions(v2_versions())
            .with_request_state_key(KEY_A)
            .build()
            .expect("first server");
        let second = crate::server::Server::builder()
            .name("b")
            .version("1")
            .with_supported_protocol_versions(v2_versions())
            .with_request_state_key(KEY_B)
            .build()
            .expect("second server");
        assert_ne!(
            first.request_state_codec().expect("codec").minting_key_id(),
            second
                .request_state_codec()
                .expect("codec")
                .minting_key_id(),
        );
    }

    #[test]
    fn v1_only_server_constructs_no_codec_and_reads_no_env() {
        // A deliberately MALFORMED key: a v1-only server must not even look.
        let server = with_env(&[(ENV_REQUEST_STATE_KEY, "bogus")], || {
            crate::server::Server::builder()
                .name("t")
                .version("1")
                .build()
                .expect("a v1-only server must pay nothing for MRTR (D-04)")
        });
        assert!(server.request_state_codec().is_none());
    }

    #[test]
    fn server_core_builder_carries_the_codec() {
        let core = crate::server::builder::ServerCoreBuilder::new()
            .name("t")
            .version("1")
            .with_supported_protocol_versions(v2_versions())
            .with_request_state_key(KEY_A)
            .build()
            .expect("core builds");
        assert_eq!(
            core.request_state_codec()
                .expect("v2 core has a codec")
                .minting_key_id(),
            key_id_of(&KEY_A)
        );
    }

    // -- mint / verify ------------------------------------------------------

    const KEY_C: [u8; KEY_LEN] = [0x33; KEY_LEN];

    /// A codec pinned to a fixed clock so expiry is deterministic.
    fn codec_at(key: &[u8; KEY_LEN], now: i64, ttl_secs: u64) -> RequestStateCodec {
        RequestStateCodec::new(key, Duration::from_secs(ttl_secs))
            .expect("valid key")
            .with_clock(Arc::new(FixedClock(now)))
    }

    fn tool_params(path: &str) -> serde_json::Value {
        serde_json::json!({ "name": "read_file", "arguments": { "path": path } })
    }

    /// Every fixture in this module is shallow, so the depth refusal is
    /// unreachable through this helper; the depth boundary itself is pinned in
    /// `crate::types::mrtr`'s own tests, where the canonicalizer lives.
    fn binding<'a>(
        principal: &'a str,
        method: &'a str,
        params: &serde_json::Value,
    ) -> RequestBinding<'a> {
        RequestBinding::from_request(principal, method, params)
            .expect("shallow fixture params bind")
    }

    #[test]
    fn mint_then_verify_round_trips_the_continuation_state() {
        let codec = codec_at(&KEY_A, 1_000, 300);
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);
        let state = serde_json::json!({ "collected": { "path": "/safe" }, "step": 2 });
        let token = codec.mint(&state, &bind, 1, None).expect("mint");
        match codec.verify(&token, &bind) {
            Verdict::Ok(continuation) => {
                assert_eq!(continuation.state, state);
                assert_eq!(continuation.round, 1);
                assert_eq!(continuation.exp, 1_300);
            },
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------
    // The sealed kinds map (D-113-O).
    // -----------------------------------------------------------------

    /// A kinds map of `count` entries at a realistic `inputRequests` key length.
    ///
    /// The keys are the shape a real handler uses — `user_name`, `workspace`,
    /// `model_says` — padded to a fixed width so the measurement below is a
    /// property of the COUNT rather than of whichever names this test picked.
    fn kinds_map(count: usize, key_len: usize) -> crate::types::mrtr::InputRequestKinds {
        use crate::types::mrtr::InputRequestKind;
        let cycle = [
            InputRequestKind::Elicitation,
            InputRequestKind::Sampling,
            InputRequestKind::Roots,
        ];
        (0..count)
            .map(|index| {
                let key = format!("{index:0>width$}", width = key_len);
                (key, cycle[index % cycle.len()])
            })
            .collect()
    }

    /// The kinds the server sealed come back out of a VERIFIED token unchanged.
    ///
    /// This is the whole mechanism D-113-O needs: the server's record of what it
    /// asked for survives the round trip through the client, integrity-protected,
    /// without the client being able to choose or alter it.
    #[test]
    fn mint_then_verify_round_trips_the_requested_kinds() {
        use crate::types::mrtr::InputRequestKind;
        let codec = codec_at(&KEY_A, 1_000, 300);
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);
        let kinds: crate::types::mrtr::InputRequestKinds = [
            ("user_name".to_string(), InputRequestKind::Elicitation),
            ("model_says".to_string(), InputRequestKind::Sampling),
            ("workspace".to_string(), InputRequestKind::Roots),
        ]
        .into_iter()
        .collect();
        let token = codec
            .mint(
                &serde_json::json!({ "step": 1 }),
                &bind,
                0,
                Some(kinds.clone()),
            )
            .expect("mint");
        match codec.verify(&token, &bind) {
            Verdict::Ok(continuation) => assert_eq!(continuation.kinds, Some(kinds)),
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    /// A continuation minted by a build WITHOUT the field still deserializes, and
    /// yields `None` — the rolling-deploy degradation, pinned against the
    /// serialized form rather than against a hand-built `Continuation`, because
    /// what has to survive is the BYTES an older build sealed.
    #[test]
    fn a_continuation_serialized_without_kinds_still_deserializes_as_none() {
        let legacy = serde_json::json!({ "state": { "step": 1 }, "exp": 1_300, "round": 2 });
        let continuation: Continuation =
            serde_json::from_value(legacy).expect("a pre-kinds continuation must still decode");
        assert_eq!(continuation.round, 2);
        assert_eq!(
            continuation.kinds, None,
            "an ABSENT kinds field must be None — the pre-kinds marker — and must never \
             be conflated with an empty map, which means \"this build asked for nothing\""
        );
    }

    /// An EMPTY kinds map survives as `Some(empty)`, distinct from absent.
    ///
    /// The two are different statements and ingress treats them differently
    /// (`Continuation::kinds`), so a serde round trip that collapsed one into the
    /// other would silently reopen D-113-O for any handler that signalled an
    /// empty `inputRequests`.
    #[test]
    fn an_empty_kinds_map_survives_as_some_not_none() {
        let continuation = Continuation {
            state: serde_json::json!({}),
            exp: 1_300,
            round: 0,
            kinds: Some(crate::types::mrtr::InputRequestKinds::new()),
        };
        let bytes = serde_json::to_vec(&continuation).expect("serializable");
        let decoded: Continuation = serde_json::from_slice(&bytes).expect("deserializable");
        assert_eq!(
            decoded.kinds,
            Some(crate::types::mrtr::InputRequestKinds::new())
        );
        assert_ne!(decoded.kinds, None);
    }

    /// The kinds map does NOT push a minted token past the bound the server's own
    /// ingress applies — MEASURED at the worst case ingress will ever accept.
    ///
    /// `MAX_INPUT_RESPONSES` (64) is the most entries a client may ANSWER, so a
    /// handler asking for more than 64 has already built a round that cannot be
    /// completed; 64 is therefore the widest kinds map worth minting. The token
    /// size is printed so the SUMMARY records a number rather than a claim.
    ///
    /// A guard already exists and needs no addition: `mint` refuses with an error
    /// when the encoded token would exceed `MAX_REQUEST_STATE_LEN`, so the failure
    /// mode at the bound is a loud refusal, never a token the server would reject
    /// at its own front door. This test pins that the REALISTIC case is nowhere
    /// near it.
    #[test]
    fn a_full_width_kinds_map_stays_within_the_accepted_token_bound() {
        let codec = codec_at(&KEY_A, 1_000, 300);
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);
        let state = serde_json::json!({ "collected": {}, "step": 1 });

        let bare = codec.mint(&state, &bind, 0, None).expect("mint");
        let full = codec
            .mint(
                &state,
                &bind,
                0,
                Some(kinds_map(
                    crate::types::mrtr::MAX_INPUT_RESPONSES,
                    "user_name_00".len(),
                )),
            )
            .expect("a 64-entry kinds map must still mint");

        println!(
            "D-113-O minted token bytes: bare = {}, with {} kinds entries = {} (bound {})",
            bare.len(),
            crate::types::mrtr::MAX_INPUT_RESPONSES,
            full.len(),
            crate::types::mrtr::MAX_REQUEST_STATE_LEN
        );
        assert!(
            full.len() <= crate::types::mrtr::MAX_REQUEST_STATE_LEN,
            "a token carrying the widest kinds map ingress can ever be answered with \
             must not exceed the bound ingress applies: {} > {}",
            full.len(),
            crate::types::mrtr::MAX_REQUEST_STATE_LEN
        );
    }

    /// A kinds map big enough to burst the bound is REFUSED at the mint, not
    /// emitted — the "never mint a token our own ingress rejects" rule, exercised
    /// through the new field rather than through `state`.
    #[test]
    fn an_absurd_kinds_map_is_refused_at_the_mint_rather_than_minted() {
        let codec = codec_at(&KEY_A, 1_000, 300);
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);
        assert!(
            codec
                .mint(&serde_json::json!({}), &bind, 0, Some(kinds_map(512, 64)),)
                .is_err(),
            "a kinds map that would burst MAX_REQUEST_STATE_LEN must fail the mint"
        );
    }

    #[test]
    fn two_mints_of_identical_input_produce_different_tokens() {
        let codec = codec_at(&KEY_A, 1_000, 300);
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);
        let state = serde_json::json!({ "a": 1 });
        let first = codec.mint(&state, &bind, 0, None).expect("mint");
        let second = codec.mint(&state, &bind, 0, None).expect("mint");
        assert_ne!(first, second, "a fresh nonce must be drawn per mint");
    }

    #[test]
    fn token_layout_is_key_id_len_then_key_id_then_nonce() {
        let codec = codec_at(&KEY_A, 1_000, 300);
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);
        let token = codec
            .mint(&serde_json::json!({}), &bind, 0, None)
            .expect("mint");
        let raw = URL_SAFE_NO_PAD.decode(token.as_bytes()).expect("base64url");
        assert_eq!(raw[0], KEY_ID_LEN_U8, "leading length byte");
        assert_eq!(
            &raw[1..=KEY_ID_LEN],
            key_id_of(&KEY_A).as_bytes(),
            "cleartext key-id prefix"
        );
        assert!(
            raw.len() > 1 + KEY_ID_LEN + NONCE_LEN,
            "a nonce plus a non-empty sealed body must follow"
        );
    }

    #[test]
    fn flipping_a_ciphertext_byte_yields_auth_failed() {
        let codec = codec_at(&KEY_A, 1_000, 300);
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);
        let token = codec
            .mint(&serde_json::json!({ "a": 1 }), &bind, 0, None)
            .expect("mint");
        let mut raw = URL_SAFE_NO_PAD.decode(token.as_bytes()).expect("base64url");
        let last = raw.len() - 1;
        raw[last] ^= 0xff;
        let mutated = URL_SAFE_NO_PAD.encode(&raw);
        assert_eq!(codec.verify(&mutated, &bind), Verdict::AuthFailed);
    }

    #[test]
    fn sep_2322_reject_tampered_state_suffix_mutation_yields_auth_failed() {
        // The EXACT mutation the conformance check `sep-2322-reject-tampered-state`
        // applies: append a marker to an otherwise valid token.
        let codec = codec_at(&KEY_A, 1_000, 300);
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);
        let token = codec
            .mint(&serde_json::json!({ "a": 1 }), &bind, 0, None)
            .expect("mint");
        let tampered = format!("{token}-TAMPERED");
        assert_eq!(codec.verify(&tampered, &bind), Verdict::AuthFailed);
    }

    #[test]
    fn a_token_minted_for_another_principal_yields_auth_failed() {
        let codec = codec_at(&KEY_A, 1_000, 300);
        let params = tool_params("/safe");
        let alice = binding("alice", "tools/call", &params);
        let bob = binding("bob", "tools/call", &params);
        let token = codec
            .mint(&serde_json::json!({ "a": 1 }), &alice, 0, None)
            .expect("mint");
        assert_eq!(codec.verify(&token, &bob), Verdict::AuthFailed);
    }

    #[test]
    fn replaying_a_token_onto_different_arguments_yields_auth_failed() {
        let codec = codec_at(&KEY_A, 1_000, 300);
        let safe = tool_params("/safe");
        let shadow = tool_params("/etc/shadow");
        let minted_for = binding("alice", "tools/call", &safe);
        let replayed_onto = binding("alice", "tools/call", &shadow);
        let token = codec
            .mint(&serde_json::json!({ "a": 1 }), &minted_for, 0, None)
            .expect("mint");
        assert_eq!(codec.verify(&token, &replayed_onto), Verdict::AuthFailed);
    }

    #[test]
    fn replaying_a_token_onto_a_different_method_yields_auth_failed() {
        let codec = codec_at(&KEY_A, 1_000, 300);
        let call = serde_json::json!({ "name": "x", "arguments": {} });
        let prompt = serde_json::json!({ "name": "x", "arguments": {} });
        let minted_for = binding("alice", "tools/call", &call);
        let replayed_onto = binding("alice", "prompts/get", &prompt);
        let token = codec
            .mint(&serde_json::json!({ "a": 1 }), &minted_for, 0, None)
            .expect("mint");
        assert_eq!(codec.verify(&token, &replayed_onto), Verdict::AuthFailed);
    }

    #[test]
    fn an_expired_token_yields_expired_carrying_a_readable_continuation() {
        // Produced with FixedClock, no sleeping and no hand-crafted ciphertext.
        let minter = codec_at(&KEY_A, 1_000, 60);
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);
        let state = serde_json::json!({ "collected": { "path": "/safe" } });
        let token = minter.mint(&state, &bind, 3, None).expect("mint");

        let verifier = codec_at(&KEY_A, 5_000, 60);
        match verifier.verify(&token, &bind) {
            Verdict::Expired(continuation) => {
                assert_eq!(continuation.state, state, "state must be READABLE");
                assert_eq!(
                    continuation.round, 3,
                    "round must survive so D-09 is not reset"
                );
            },
            other => panic!("expected Expired, got {other:?}"),
        }
    }

    #[test]
    fn a_token_from_an_unknown_key_id_yields_unknown_key() {
        let minter = codec_at(&KEY_A, 1_000, 300);
        let verifier = codec_at(&KEY_B, 1_000, 300);
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);
        let token = minter
            .mint(&serde_json::json!({ "a": 1 }), &bind, 0, None)
            .expect("mint");
        assert_eq!(
            verifier.verify(&token, &bind),
            Verdict::UnknownKey,
            "an unshared key must be DISTINGUISHABLE from tampering (D-04)"
        );
    }

    #[test]
    fn a_token_minted_under_the_previous_key_still_verifies() {
        let old = codec_at(&KEY_B, 1_000, 300);
        let rotated = codec_at(&KEY_A, 1_000, 300)
            .with_previous_keys([KEY_B])
            .expect("previous key");
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);
        let token = old
            .mint(&serde_json::json!({ "a": 1 }), &bind, 0, None)
            .expect("mint");
        assert!(matches!(rotated.verify(&token, &bind), Verdict::Ok(_)));
    }

    #[test]
    fn colliding_key_ids_resolve_to_ok_or_auth_failed_never_unknown_key() {
        let forced = key_id_of(b"a deliberately forced key id");
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);

        // A verifier holding TWO different keys under the SAME key-id.
        let verifier = codec_at(&KEY_A, 1_000, 300)
            .with_forced_minting_key_id(forced)
            .with_forced_accepting_key(forced, &KEY_B)
            .expect("second entry");

        // Minted under KEY_A -> the matching entry opens it.
        let minter_a = codec_at(&KEY_A, 1_000, 300).with_forced_minting_key_id(forced);
        let token_a = minter_a
            .mint(&serde_json::json!({ "a": 1 }), &bind, 0, None)
            .expect("mint");
        assert!(matches!(verifier.verify(&token_a, &bind), Verdict::Ok(_)));

        // Minted under KEY_B -> the OTHER matching entry opens it.
        let minter_b = codec_at(&KEY_B, 1_000, 300).with_forced_minting_key_id(forced);
        let token_b = minter_b
            .mint(&serde_json::json!({ "b": 2 }), &bind, 0, None)
            .expect("mint");
        assert!(matches!(verifier.verify(&token_b, &bind), Verdict::Ok(_)));

        // Minted under an unrelated third key wearing the SAME id -> AuthFailed,
        // never UnknownKey and never a false Ok.
        let minter_c = codec_at(&KEY_C, 1_000, 300).with_forced_minting_key_id(forced);
        let token_c = minter_c
            .mint(&serde_json::json!({ "c": 3 }), &bind, 0, None)
            .expect("mint");
        assert_eq!(verifier.verify(&token_c, &bind), Verdict::AuthFailed);
    }

    #[test]
    fn malformed_tokens_yield_auth_failed_and_never_panic() {
        let codec = codec_at(&KEY_A, 1_000, 300);
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);

        let oversized = "A".repeat(crate::types::mrtr::MAX_REQUEST_STATE_LEN + 1);
        let too_short = URL_SAFE_NO_PAD.encode([KEY_ID_LEN_U8, 1, 2, 3]);
        for candidate in [
            "",
            "!!!not base64!!!",
            "AAAA",
            oversized.as_str(),
            too_short.as_str(),
        ] {
            assert_eq!(
                codec.verify(candidate, &bind),
                Verdict::AuthFailed,
                "malformed input {candidate:?} must be a verdict, not a panic"
            );
        }
    }

    #[test]
    fn a_minted_token_fits_inside_the_accepted_bound() {
        let codec = codec_at(&KEY_A, 1_000, 300);
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);
        let token = codec
            .mint(&serde_json::json!({ "a": "x".repeat(64) }), &bind, 0, None)
            .expect("mint");
        assert!(token.len() <= crate::types::mrtr::MAX_REQUEST_STATE_LEN);
    }

    #[test]
    fn minting_an_oversized_state_errors_rather_than_producing_a_self_rejecting_token() {
        let codec = codec_at(&KEY_A, 1_000, 300);
        let params = tool_params("/safe");
        let bind = binding("alice", "tools/call", &params);
        let huge =
            serde_json::json!({ "blob": "x".repeat(crate::types::mrtr::MAX_REQUEST_STATE_LEN) });
        assert!(
            codec.mint(&huge, &bind, 0, None).is_err(),
            "a token the server would itself reject must never be minted"
        );
    }

    // -- SecretKey / resolve_codec_at_build (D-113-P) ------------------------

    /// Pins [`SecretKey`]'s CONTRACT, which is all a test can honestly pin.
    ///
    /// What this PROVES: the zeroization code path `Zeroizing`'s `Drop` invokes
    /// — `<[u8; KEY_LEN] as Zeroize>::zeroize` — replaces the key bytes with
    /// zeroes when it runs.
    ///
    /// What it does NOT prove: that the destructor ran, or that the freed heap
    /// or stack slot afterwards contains zeroes. Reading a dropped or freed
    /// value is undefined behaviour, so no test in safe Rust can observe it.
    /// "The drop runs" comes from [`zeroize::Zeroizing`]'s own `Drop` impl, and
    /// "the write is not optimised away" comes from zeroize's volatile write
    /// plus its `SeqCst` compiler fence — mechanisms, not assertions.
    #[test]
    fn secret_key_zeroize_replaces_the_key_bytes_with_zeroes() {
        let secret = SecretKey::new(KEY_A);
        let before: [u8; KEY_LEN] = *secret;
        assert_eq!(before, KEY_A, "the wrapper must not alter the key it holds");

        // Same code path `Zeroizing::drop` takes, run on a value we still own so
        // the result is observable without touching dropped memory.
        let mut scrubbed: [u8; KEY_LEN] = *secret;
        scrubbed.zeroize();
        assert_eq!(scrubbed, [0u8; KEY_LEN]);
        assert_ne!(
            scrubbed, before,
            "a fixture key of all zeroes would make this test vacuous"
        );
    }

    #[test]
    fn resolve_codec_at_build_returns_none_for_a_v1_only_accept_list() {
        // A deliberately MALFORMED env key: a v1-only server must not even look,
        // so this resolves to `None` rather than erroring (D-04).
        let resolved = with_env(&[(ENV_REQUEST_STATE_KEY, "bogus")], || {
            resolve_codec_at_build(
                &crate::types::protocol::context::default_accept_list(),
                None,
                &[],
                None,
            )
        })
        .expect("a v1-only accept-list reads no env var");
        assert!(resolved.is_none());
    }

    #[test]
    fn resolve_codec_at_build_prefers_the_by_reference_key_over_the_environment() {
        let key = SecretKey::new(KEY_B);
        let codec = with_env(&[(ENV_REQUEST_STATE_KEY, &b64(&KEY_A))], || {
            resolve_codec_at_build(&v2_versions(), Some(&key), &[], None)
        })
        .expect("resolves")
        .expect("a v2 accept-list has a codec");
        assert_eq!(
            codec.minting_key_id(),
            key_id_of(&KEY_B),
            "a builder-supplied key must beat PMCP_REQUEST_STATE_KEY"
        );
        assert_eq!(
            *key, KEY_B,
            "taking the key by reference must not consume or alter the caller's copy"
        );
    }

    #[test]
    fn resolve_codec_at_build_appends_previous_keys_to_the_env_derived_set() {
        let previous = [SecretKey::new(KEY_C)];
        let codec = with_env(
            &[
                (ENV_REQUEST_STATE_KEY, &b64(&KEY_A)),
                (ENV_REQUEST_STATE_KEY_PREVIOUS, &b64(&KEY_B)),
            ],
            || resolve_codec_at_build(&v2_versions(), None, &previous, None),
        )
        .expect("resolves")
        .expect("codec");
        let accepting = codec.accepting_key_ids();
        assert_eq!(codec.minting_key_id(), key_id_of(&KEY_A));
        assert!(
            accepting.contains(&key_id_of(&KEY_B)),
            "the env-derived rotated-out key must survive: {accepting:?}"
        );
        assert!(
            accepting.contains(&key_id_of(&KEY_C)),
            "builder previous keys are APPENDED, not substituted: {accepting:?}"
        );
    }

    #[test]
    fn resolve_codec_at_build_honours_env_previous_key_even_with_an_explicit_key() {
        let key = SecretKey::new(KEY_A);
        let codec = with_env(&[(ENV_REQUEST_STATE_KEY_PREVIOUS, &b64(&KEY_B))], || {
            resolve_codec_at_build(&v2_versions(), Some(&key), &[], None)
        })
        .expect("resolves")
        .expect("codec");
        assert!(
            codec.accepting_key_ids().contains(&key_id_of(&KEY_B)),
            "a builder key overrides PMCP_REQUEST_STATE_KEY but NOT the separate \
             rotation setting"
        );
    }

    #[test]
    fn resolve_codec_at_build_fails_the_build_on_a_malformed_env_key() {
        let result = with_env(&[(ENV_REQUEST_STATE_KEY, "bogus")], || {
            resolve_codec_at_build(&v2_versions(), None, &[], None)
        });
        assert!(
            result.is_err(),
            "a malformed CONFIGURED key must fail the BUILD (T-113-17)"
        );
    }

    #[test]
    fn resolve_codec_at_build_prefers_an_explicit_ttl_over_the_environment() {
        let codec = with_env(&[(ENV_REQUEST_STATE_TTL_SECS, "42")], || {
            resolve_codec_at_build(
                &v2_versions(),
                Some(&SecretKey::new(KEY_A)),
                &[],
                Some(Duration::from_secs(7)),
            )
        })
        .expect("resolves")
        .expect("codec");
        assert_eq!(codec.ttl(), Duration::from_secs(7));
    }

    #[test]
    fn server_core_builder_previous_keys_reach_the_accepting_set() {
        let core = crate::server::builder::ServerCoreBuilder::new()
            .name("t")
            .version("1")
            .with_supported_protocol_versions(v2_versions())
            .with_request_state_key(KEY_A)
            .with_request_state_previous_keys(vec![KEY_B])
            .build()
            .expect("core builds");
        let accepting = core
            .request_state_codec()
            .expect("codec")
            .accepting_key_ids();
        assert!(accepting.contains(&key_id_of(&KEY_A)));
        assert!(accepting.contains(&key_id_of(&KEY_B)));
    }

    // -- fuzz seam ----------------------------------------------------------

    /// The seam cannot rot silently: if `fuzz_support` is removed or its
    /// discriminants shift, this fails under `--features fuzzing`.
    #[cfg(feature = "fuzzing")]
    #[test]
    fn fuzz_support_seam_rejects_garbage() {
        assert_eq!(
            super::fuzz_support::verify_bytes(b"garbage"),
            super::fuzz_support::VERDICT_AUTH_FAILED
        );
        assert_ne!(
            super::fuzz_support::verify_bytes(&[0xff, 0xfe, 0xfd]),
            super::fuzz_support::VERDICT_OK
        );
    }

    // -- properties ---------------------------------------------------------
    //
    // Run the full-strength sweep with:
    //   PROPTEST_CASES=1000 cargo test --lib --features full -- property_request_state

    /// Arbitrary JSON-ish continuation state.
    fn arb_state() -> impl proptest::strategy::Strategy<Value = serde_json::Value> {
        use proptest::prelude::*;
        prop_oneof![
            Just(serde_json::Value::Null),
            any::<bool>().prop_map(serde_json::Value::from),
            any::<i32>().prop_map(serde_json::Value::from),
            "[ -~]{0,64}".prop_map(serde_json::Value::from),
            proptest::collection::vec("[ -~]{0,16}", 0..6).prop_map(|v| serde_json::json!(v)),
            proptest::collection::hash_map("[a-z]{1,8}", "[ -~]{0,16}", 0..6)
                .prop_map(|m| serde_json::json!(m)),
        ]
    }

    proptest::proptest! {
        /// mint -> verify is the identity on the continuation state, for arbitrary
        /// state, principal, method and ttl.
        #[test]
        fn property_request_state_roundtrip(
            state in arb_state(),
            principal in "[ -~]{0,48}",
            method in "[a-z/]{1,24}",
            ttl_secs in 1u64..3600,
            round in 0u8..=255,
        ) {
            let codec = RequestStateCodec::new(&KEY_A, Duration::from_secs(ttl_secs))
                .expect("valid key")
                .with_clock(Arc::new(FixedClock(1_000)));
            let params = serde_json::json!({ "name": "n", "arguments": { "k": "v" } });
            let bind = RequestBinding::from_request(&principal, &method, &params)
                .expect("a two-level fixture is far inside the canonical depth cap");
            let token = codec.mint(&state, &bind, round, None).expect("mint");
            match codec.verify(&token, &bind) {
                Verdict::Ok(continuation) => {
                    proptest::prop_assert_eq!(continuation.state, state);
                    proptest::prop_assert_eq!(continuation.round, round);
                },
                other => proptest::prop_assert!(false, "expected Ok, got {:?}", other),
            }
        }

        /// The binding is TOTAL: a token minted under one binding never verifies
        /// `Ok` under a different one, whichever component differs.
        #[test]
        fn property_request_state_binding_is_total(
            principal_a in "[ -~]{0,24}",
            principal_b in "[ -~]{0,24}",
            method_a in "[a-z/]{1,16}",
            method_b in "[a-z/]{1,16}",
            arg_a in "[ -~]{0,24}",
            arg_b in "[ -~]{0,24}",
        ) {
            let params_a = serde_json::json!({ "name": "n", "arguments": { "k": arg_a } });
            let params_b = serde_json::json!({ "name": "n", "arguments": { "k": arg_b } });
            let bind_a = RequestBinding::from_request(&principal_a, &method_a, &params_a)
                .expect("a two-level fixture is far inside the canonical depth cap");
            let bind_b = RequestBinding::from_request(&principal_b, &method_b, &params_b)
                .expect("a two-level fixture is far inside the canonical depth cap");
            // Only interesting when the bindings genuinely differ.
            proptest::prop_assume!(
                principal_a != principal_b || method_a != method_b || arg_a != arg_b
            );

            let codec = RequestStateCodec::new(&KEY_A, Duration::from_secs(DEFAULT_TTL_SECS))
                .expect("valid key")
                .with_clock(Arc::new(FixedClock(1_000)));
            let token = codec.mint(&serde_json::json!({ "s": 1 }), &bind_a, 0, None).expect("mint");
            proptest::prop_assert_eq!(codec.verify(&token, &bind_b), Verdict::AuthFailed);
        }

        /// `verify` is TOTAL over arbitrary strings: it always returns a verdict
        /// and never panics (T-113-14).
        #[test]
        fn property_request_state_never_panics(token in ".{0,512}") {
            let codec = RequestStateCodec::new(&KEY_A, Duration::from_secs(DEFAULT_TTL_SECS))
                .expect("valid key")
                .with_clock(Arc::new(FixedClock(1_000)));
            let params = serde_json::json!({ "name": "n", "arguments": {} });
            let bind = RequestBinding::from_request("alice", "tools/call", &params)
                .expect("a two-level fixture is far inside the canonical depth cap");
            // A token this codec did not mint must never verify Ok.
            proptest::prop_assert!(!matches!(codec.verify(&token, &bind), Verdict::Ok(_)));
        }
    }
}
