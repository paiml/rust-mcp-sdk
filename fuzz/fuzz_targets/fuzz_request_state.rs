//! Fuzz target for the `requestState` AEAD continuation token verifier.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run fuzz_request_state` (plain
//! form, no `+nightly` — matches the repo Makefile `test-fuzz` target).
//!
//! `requestState` is the one place in the SDK where a cryptographic mistake is
//! directly exploitable by a remote party: the MCP 2026-07-28 spec says servers
//! MUST treat the value as attacker-controlled input. This target drives
//! `RequestStateCodec::verify` with arbitrary bytes through a
//! `feature = "fuzzing"`-gated seam.
//!
//! Invariants:
//!
//! 1. `verify` NEVER panics, whatever the input bytes (threat T-113-14: a
//!    malformed, oversized, or truncated token must produce a verdict rather than
//!    an unwind that a remote party can trigger at will).
//! 2. `verify` NEVER returns the `Ok` discriminant for input the codec did not
//!    mint. Reaching `Ok` requires forging a Poly1305 tag under a key the fuzzer
//!    does not have, so any hit here is a genuine authentication break, not a
//!    flake.
//!
//! The seam pins a FIXED key rather than reading `PMCP_REQUEST_STATE_KEY`, so a
//! crash artifact replays deterministically regardless of ambient process state.
//!
//! Corpus cases worth seeding:
//!   - a real token produced by `mint` (should verify `Ok` — the only input that
//!     legitimately can), plus single-byte mutations of it
//!   - the empty string, and strings just under / just over
//!     `MAX_REQUEST_STATE_LEN`
//!   - valid base64url that decodes to fewer than `1 + key_id + nonce` bytes
//!   - a valid key-id prefix followed by random ciphertext

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::server::request_state::fuzz_support::{verify_bytes, VERDICT_OK};

fuzz_target!(|data: &[u8]| {
    // Invariant 1: total over arbitrary bytes — a panic here fails the target.
    let verdict = verify_bytes(data);

    // Invariant 2: an unforgeable verdict stays unforgeable.
    assert_ne!(
        verdict, VERDICT_OK,
        "verify() accepted a token the fuzzer produced without the key — \
         this is an AEAD authentication break, not a flake"
    );
});
