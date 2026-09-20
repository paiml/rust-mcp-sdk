//! Fuzz target for the `subscriptions/listen` CLIENT frame decoder (HTTP-04).
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo +nightly fuzz run
//! subscription_listen_frames -- -runs=20000`. The `+nightly` is load-bearing —
//! `cargo fuzz run` passes `-Zsanitizer=address`, which stable rustc rejects.
//! The recorded campaigns live in the repo at
//! `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-FUZZ-EVIDENCE.md`
//! (a planning artifact, not shipped in the published crate — the command above
//! is the whole reproduction recipe).
//!
//! A listen stream's bytes come from a REMOTE server (or whatever intermediary
//! sits between), so the SSE tokenizing, incremental UTF-8 decoding, JSON-RPC
//! classification and `subscriptionId` validation in
//! `pmcp::client::subscriptions` all run on untrusted input.
//!
//! Invariants under arbitrary bytes:
//!   1. **Never panics** (T-113-67) — a hostile or merely broken frame must not
//!      take down a client, and the incremental UTF-8 buffer must not wedge on
//!      an invalid sequence.
//!   2. **Never cross-delivers** (T-113-66) — a frame is delivered to the caller
//!      only if it carried THIS subscription's id. The id used here is
//!      improbable enough that a delivery from bytes not containing it would be
//!      a real cross-tag escape, which is exactly the failure the server-side
//!      `ListenKey` pairing prevents and the client must not paper over.
//!   3. **Memory stays bounded** (T-113-79 / T-113-93) — after every chunk, the
//!      bytes the parser RETAINS across lines (`SseParser::buffered_bytes()`:
//!      the unterminated line plus the `data:` payload of the event still
//!      awaiting its blank line) are `<= max_buffer_size`. This is the property
//!      the campaign exists to defend: a peer that streams ordinary
//!      newline-terminated `data:` lines forever must not be able to grow the
//!      parser. It is asserted here because it was NOT — the target previously
//!      asserted only that the overflow latch never clears, which cannot fail
//!      for any input at any bound, so 20 000 green runs coexisted with exactly
//!      that unbounded-growth defect (`113-VERIFICATION.md` gap item 3, review
//!      CR-01/WR-03, closed by plan 113-17 and pinned here by plan 113-19).
//!
//! Subordinate note, kept because it is cheap and it documents the latch: once
//! the bounded parser has discarded an oversized line, `overflowed()` stays true
//! for the rest of the stream (T-113-73). `read_next_frame` polls that flag once
//! per body frame and ends the stream on the first `true`; if it could clear, a
//! peer could hide a discarded line behind a subsequent well-formed one. This is
//! deliberately NOT a numbered invariant: `overflowed` has one write site and no
//! clearing path, so no generated input can falsify it.

#![no_main]

use libfuzzer_sys::fuzz_target;

/// The subscription id the decoder is told it owns. Deliberately improbable, so
/// "the input contained this literal" is a meaningful precondition.
const SUBSCRIPTION_ID: &str = "fuzz-subscription-4f1c9a2e";

/// The line-buffer bounds the campaign runs the parser at — the argument the
/// seam hands to `SseParser::with_max_buffer_size`. Every input is decoded once
/// per bound.
///
/// Both are DELIBERATELY tiny: production bounds this path at 256 KiB
/// (`MAX_LISTEN_LINE_BYTES`), and a fuzzer that must synthesise a quarter of a
/// megabyte of newline-free input to reach the discard-and-latch branch would
/// effectively never reach it — the branch that manipulates buffer state on
/// hostile input would go unfuzzed. The branch itself is bound-agnostic, so a
/// small bound loses no fidelity.
///
/// TWO bounds because one was not enough, MEASURED rather than assumed: at 64
/// bytes alone a `-runs=20000` campaign covered the branch ZERO times. libFuzzer
/// ramps its length limit (`len_control`) and only reached 38-byte inputs within
/// that budget, and 38 bytes cannot push a 64-byte buffer over its bound. So:
///
/// - **64** — the ordinary path. Inputs stay under the bound, which is the SSE
///   tokenizing, incremental-UTF-8 and JSON-RPC classification work a healthy
///   stream does.
/// - **8** — the overflow path, reached by any newline-free chunk of 9+ bytes,
///   i.e. by nearly every input libFuzzer generates from its first run. This is
///   what makes the discard-and-latch branch actually covered by a short
///   campaign with no special flags and no seeded corpus (`fuzz/.gitignore`
///   ignores `corpus`, so a seed would not survive for the next reader).
const MAX_BUFFER_SIZES: [usize; 2] = [64, 8];

/// How the input is sliced into successive "body frames".
///
/// A live listen stream is read incrementally, so the SSE line buffer and the
/// undecoded-UTF-8 tail both carry ACROSS chunks: splits land mid-character and
/// mid-line. Slicing at a fixed width keeps a crash artifact deterministic to
/// replay, and 16 bytes means the 64-byte bound is approached by accumulation
/// (several chunks) rather than only by one oversized chunk.
const CHUNK_LEN: usize = 16;

fuzz_target!(|data: &[u8]| {
    // A zero-length body frame is itself a case worth feeding, and `chunks()`
    // yields nothing for an empty slice.
    let chunks: Vec<&[u8]> = if data.is_empty() {
        vec![data]
    } else {
        data.chunks(CHUNK_LEN).collect()
    };
    let text = String::from_utf8_lossy(data);

    for max_buffer_size in MAX_BUFFER_SIZES {
        // Invariant 1: arbitrary bytes in, no panic out.
        let (outcomes, overflowed, peak_buffered_bytes) =
            pmcp::client::subscriptions::decode_listen_chunks_for_fuzz(
                &chunks,
                SUBSCRIPTION_ID,
                max_buffer_size,
            );

        // Invariant 2: nothing is delivered from bytes that never named this
        // subscription.
        //
        // The precondition is checked against the RAW bytes, which is sound only
        // when no JSON escape could have spelled the id indirectly: a JSON string
        // of \u-escaped code points decodes to the id without the id's bytes ever
        // appearing literally, and asserting on that input would report a SPURIOUS
        // crash (verification finding WR-08). An input containing no backslash at
        // all cannot carry such an escape, so the literal check applies exactly
        // there — and an escape-spelled id is the SAME id anyway, not the cross-tag
        // escape this invariant exists to catch.
        if outcomes.iter().any(std::result::Result::is_ok) && !text.contains('\\') {
            assert!(
                text.contains(SUBSCRIPTION_ID),
                "a notification was delivered from a chunk that never carried this \
                 subscription's id (bound {max_buffer_size})"
            );
        }

        // Invariant 3: MEMORY STAYS BOUNDED.
        //
        // What this defends: a peer that streams perfectly ordinary
        // newline-terminated `data:` lines and simply never sends the blank line
        // that would dispatch the event must not be able to grow the parser. Each
        // such line completes — so a "does this chunk carry a newline?" escape
        // hatch waves it through — while its payload accumulates into
        // `current_event.data`, which only a BLANK line ever clears. That is
        // GAP-A (`113-VERIFICATION.md` gap item 3 / review CR-01), and the reason
        // it survived a 20 000-run green campaign is that the target asserted no
        // SIZE at all: the seam reported outcomes and flags, never retention.
        //
        // `peak_buffered_bytes[i]` is `SseParser::buffered_bytes()` sampled after
        // chunk `i` was drained, i.e. exactly the pair of accumulators
        // `max_buffer_size` bounds. Enforcement discards both and latches rather
        // than growing, so the bound holds on return from every feed — and a
        // regression that reintroduces any escape hatch produces a crash artifact
        // here instead of a silent pass.
        for (index, held) in peak_buffered_bytes.iter().copied().enumerate() {
            assert!(
                held <= max_buffer_size,
                "the parser retained {held} bytes after chunk {index} under a \
                 {max_buffer_size}-byte bound (peaks: {peak_buffered_bytes:?})"
            );
        }

        // Subordinate note, not a numbered invariant: `overflowed()` LATCHES.
        // Once a line has been discarded the stream has lost bytes, so a later
        // chunk must not present it as healthy again. Kept because it is cheap
        // and it documents the latch — but `overflowed` has exactly one write
        // site and no clearing path, so no input can falsify this. It is the
        // assertion that USED to be Invariant 3, and its tautology is why this
        // target needed a real bound check (review WR-03).
        let mut latched = false;
        for (index, seen) in overflowed.into_iter().enumerate() {
            assert!(
                seen || !latched,
                "overflowed() cleared at chunk {index} after latching (bound \
                 {max_buffer_size}) — a discarded line would be hidden from the \
                 stream-ending check"
            );
            latched |= seen;
        }
    }
});
