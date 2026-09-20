//! Fuzz target for `pmcp::types::protocol::TraceContext::from_meta`.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run trace_context_from_meta`.
//!
//! `from_meta` parses UNTRUSTED client-supplied `_meta` JSON (threat T-112-09).
//! Invariants under arbitrary bytes:
//!   1. Never panics (arbitrary bytes -> `serde_json::Value` -> `from_meta`).
//!   2. Bounded-length: no surfaced trace field ever exceeds the ingress cap,
//!      so an attacker-controlled oversized value can't be propagated.

#![no_main]

use libfuzzer_sys::fuzz_target;

// Mirrors `MAX_TRACE_VALUE_LEN` in src/types/protocol/context.rs. Kept as a
// local literal because the const is module-private; the invariant this fuzz
// target asserts is the public, observable bound.
const MAX_TRACE_VALUE_LEN: usize = 8192;

fuzz_target!(|data: &[u8]| {
    // Arbitrary bytes -> JSON value (error paths are fine, panics are not).
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) {
        if let Some(ctx) = pmcp::types::protocol::TraceContext::from_meta(&value) {
            assert!(
                ctx.traceparent.len() <= MAX_TRACE_VALUE_LEN,
                "traceparent exceeded bound"
            );
            if let Some(ts) = &ctx.tracestate {
                assert!(ts.len() <= MAX_TRACE_VALUE_LEN, "tracestate exceeded bound");
            }
            if let Some(bg) = &ctx.baggage {
                assert!(bg.len() <= MAX_TRACE_VALUE_LEN, "baggage exceeded bound");
            }
        }
    }
});
