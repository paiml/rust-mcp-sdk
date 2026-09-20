//! Target-agnostic PKCE (RFC 7636) crypto helper for OAuth 2.0 Authorization
//! Code flows.
//!
//! This module provides the pure cryptographic primitives needed to drive an
//! OAuth Authorization Code + PKCE flow — a code verifier, its S256 code
//! challenge, and a CSRF `state` value. Unlike the native CLI flow in
//! [`crate::client::oauth`] (which uses the optional `rand` dependency and is
//! therefore not available on `wasm32`), this module is **ungated** and uses
//! [`getrandom::fill`] for randomness so it compiles and runs identically on
//! the host and on `wasm32-unknown-unknown` (Web Crypto via the `wasm_js`
//! backend).
//!
//! # Why a shared helper
//!
//! Browser PKCE and the existing native loopback flow both need RFC 7636
//! verifier/challenge/state primitives. The native primitives are private and
//! pull in `rand`, which will not build on wasm. This helper extracts the exact
//! same logic (`SHA-256` via the audited [`sha2`] crate, `base64url` no-pad via
//! [`base64`]) with the RNG swapped to `getrandom::fill`, so it is reusable and
//! target-agnostic.
//!
//! # Examples
//!
//! ```
//! use pmcp::shared::pkce::{generate_code_verifier, code_challenge_s256, generate_state};
//!
//! // Generate a fresh PKCE pair for an authorization request.
//! let verifier = generate_code_verifier()?;
//! let challenge = code_challenge_s256(&verifier);
//! let state = generate_state()?;
//!
//! // The verifier is a 43-char base64url (no-pad) string of 32 random bytes.
//! assert_eq!(verifier.len(), 43);
//! // The challenge is deterministic for a given verifier.
//! assert_eq!(challenge, code_challenge_s256(&verifier));
//! # Ok::<(), pmcp::Error>(())
//! ```
//!
//! # RFC 7636 Appendix B vector
//!
//! ```
//! use pmcp::shared::pkce::code_challenge_s256;
//!
//! // The verifier/challenge pair published in RFC 7636 Appendix B.
//! let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
//! let challenge = code_challenge_s256(verifier);
//! assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
//! ```

use crate::error::{Error, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};

/// Number of CSPRNG bytes used to build a code verifier / state value.
///
/// 32 bytes encodes to a 43-character base64url (no-pad) string, which sits
/// inside the RFC 7636 verifier length bounds (43..=128 characters).
const PKCE_RANDOM_BYTES: usize = 32;

/// Fill a fixed-size buffer with cryptographically secure random bytes.
///
/// Centralises the single `getrandom::fill` call so both the verifier and the
/// state generators share one CSPRNG source, and so a `getrandom::Error` is
/// mapped to [`Error::internal`] in exactly one place (no `unwrap`/`expect`).
pub(crate) fn random_bytes() -> Result<[u8; PKCE_RANDOM_BYTES]> {
    let mut buf = [0u8; PKCE_RANDOM_BYTES];
    getrandom::fill(&mut buf)
        .map_err(|e| Error::internal(format!("CSPRNG (getrandom) failed: {e}")))?;
    Ok(buf)
}

/// Generate a PKCE code verifier (RFC 7636 §4.1).
///
/// Returns a base64url (no-pad) encoding of 32 cryptographically secure random
/// bytes — a 43-character string drawn from the unreserved character set
/// `[A-Za-z0-9-_]`.
///
/// # Errors
///
/// Returns [`Error::internal`] if the underlying CSPRNG ([`getrandom::fill`])
/// fails to produce randomness (for example, an unsupported target or an OS
/// entropy source error). The function never panics.
///
/// # Examples
///
/// ```
/// use pmcp::shared::pkce::generate_code_verifier;
///
/// let verifier = generate_code_verifier()?;
/// assert_eq!(verifier.len(), 43);
/// assert!(verifier.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
/// # Ok::<(), pmcp::Error>(())
/// ```
pub fn generate_code_verifier() -> Result<String> {
    let bytes = random_bytes()?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// Compute the S256 PKCE code challenge for a verifier (RFC 7636 §4.2).
///
/// Returns the base64url (no-pad) encoding of `SHA-256(verifier)`. This is
/// deterministic: the same verifier always yields the same challenge, matching
/// the `code_challenge_method=S256` convention validated by the bundled `IdP` in
/// [`crate::server::auth`].
///
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
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let digest = hasher.finalize();
    URL_SAFE_NO_PAD.encode(digest)
}

/// Generate an opaque CSRF `state` value for an authorization request.
///
/// Uses the same CSPRNG source and encoding as [`generate_code_verifier`] (the
/// native flow reuses verifier generation for state), producing a 43-character
/// base64url (no-pad) string suitable as an unguessable, single-use anti-CSRF
/// token bound to the in-flight authorization request.
///
/// # Errors
///
/// Returns [`Error::internal`] if the underlying CSPRNG ([`getrandom::fill`])
/// fails. The function never panics.
///
/// # Examples
///
/// ```
/// use pmcp::shared::pkce::generate_state;
///
/// let state = generate_state()?;
/// assert_eq!(state.len(), 43);
/// # Ok::<(), pmcp::Error>(())
/// ```
pub fn generate_state() -> Result<String> {
    let bytes = random_bytes()?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HOST-target proof the ungated module links `getrandom` on host (HIGH-1).
    #[test]
    fn pkce_verifier_is_43_char_base64url() {
        let verifier = generate_code_verifier().expect("CSPRNG available on host");
        assert_eq!(verifier.len(), 43, "32 bytes base64url-no-pad => 43 chars");
        assert!(
            verifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "verifier must use only the base64url-no-pad unreserved charset"
        );
    }

    /// RFC 7636 Appendix B vector pins S256 correctness.
    #[test]
    fn pkce_rfc7636_appendix_b_vector() {
        let challenge = code_challenge_s256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    /// S256 challenge is deterministic for a given verifier.
    #[test]
    fn pkce_challenge_is_deterministic() {
        let verifier = generate_code_verifier().expect("CSPRNG available on host");
        assert_eq!(
            code_challenge_s256(&verifier),
            code_challenge_s256(&verifier)
        );
    }

    /// Distinct verifiers are produced across calls (entropy sanity check).
    #[test]
    fn pkce_verifiers_are_distinct() {
        let a = generate_code_verifier().expect("CSPRNG available on host");
        let b = generate_code_verifier().expect("CSPRNG available on host");
        assert_ne!(a, b, "two CSPRNG draws must not collide");
    }

    /// State uses the same shape as the verifier.
    #[test]
    fn pkce_state_is_43_char_base64url() {
        let state = generate_state().expect("CSPRNG available on host");
        assert_eq!(state.len(), 43);
        assert!(state
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
    }
}
