//! Fuzz target for the streamable-HTTP CLIENT's incremental SSE reader
//! (CONF-09, phase 118.2).
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo +nightly fuzz run
//! streamable_sse_frames -- -runs=20000`. The `+nightly` is load-bearing —
//! `cargo fuzz run` passes `-Zsanitizer=address`, which stable rustc rejects.
//!
//! Both of a pmcp client's `text/event-stream` read sites — the GET session
//! stream and the POST response that answers `text/event-stream` — read through
//! the same helpers, and every byte they see comes from a REMOTE server (or
//! whatever intermediary sits between). The SSE tokenizing, the incremental
//! UTF-8 decoding and the JSON-RPC classification therefore all run on untrusted
//! input.
//!
//! # It drives the REAL decode path
//!
//! `fuzz/` is a separate, workspace-excluded `pmcp-fuzz` package that depends on
//! `pmcp` as an ordinary dependency, so a target cannot reach private helpers.
//! It reaches them through the `#[doc(hidden)]`, `fuzzing`-gated seam
//! `pmcp::shared::streamable_http::decode_sse_chunks_for_fuzz`, the twin of
//! `pmcp::client::subscriptions::decode_listen_chunks_for_fuzz`. It deliberately
//! does NOT re-implement the sequence: a target that re-implements the code
//! under test proves nothing about the code under test.
//!
//! It also cannot construct a `hyper::body::Incoming` — nothing outside hyper
//! can — which is exactly why the decode helpers in `streamable_http.rs` are
//! free functions over already-decoded values.
//!
//! # Invariants under arbitrary bytes
//!
//!   1. **Never panics** — a hostile or merely broken frame must not take down a
//!      client, and the incremental UTF-8 buffer must not wedge on an invalid
//!      sequence split across a chunk boundary (T-118.2-01-06).
//!   2. **Memory stays bounded** (T-118.2-04-05) — after EVERY chunk, the bytes
//!      the parser retains across lines (`SseParser::buffered_bytes()`: the
//!      unterminated line plus the `data:` payload of an event still awaiting
//!      its blank line) are `<= max_buffer_size`, AND the undecoded UTF-8 tail
//!      is at most 3 bytes, the longest incomplete character.
//!
//!      Carried over explicitly from the sibling campaign: do NOT assert "the
//!      overflow latch never clears" and call it a bound. That assertion cannot
//!      fail for any input at any bound, which is why 20 000 green runs of
//!      `subscription_listen_frames` coexisted with exactly the unbounded-growth
//!      defect it was supposed to catch (`113-VERIFICATION.md` gap item 3).
//!      Asserting a SIZE is what makes this target non-vacuous.
//!   3. **Never fabricates or cross-delivers** — a decoded message's
//!      distinguishing member (`method`, `result` or `error`, the three
//!      `parse_message` dispatches on) must have appeared in the input bytes. A
//!      decoder that leaked one event's `data:` payload into another event, or
//!      invented one, would produce an `Ok` from bytes that carried none of
//!      them.
//!
//! Subordinate note, kept because it is cheap and it documents the latch: once
//! the bounded parser has discarded an oversized line, `overflowed()` stays true
//! for the rest of the stream. `read_next_sse_frame` polls that flag once per
//! body frame and ENDS the stream on the first `true`; if it could clear, a peer
//! could hide a discarded line behind a subsequent well-formed one. Deliberately
//! NOT a numbered invariant: `overflowed` has one write site and no clearing
//! path, so no generated input can falsify it.

#![no_main]

use libfuzzer_sys::fuzz_target;

/// The parser bounds the campaign runs at. Every input is decoded once per
/// bound.
///
/// Both are DELIBERATELY tiny: production bounds this path at 16 MiB
/// (`DEFAULT_MAX_COLLECTED_BODY_BYTES`), and a fuzzer that must synthesise 16
/// MiB of newline-free input to reach the discard-and-latch branch would
/// effectively never reach it — the branch that manipulates buffer state on
/// hostile input would go unfuzzed. The branch itself is bound-agnostic, so a
/// small bound loses no fidelity.
///
/// TWO bounds for the reason the sibling target MEASURED: at 64 bytes alone a
/// `-runs=20000` campaign covered the overflow branch zero times, because
/// libFuzzer ramps its length limit and only reached ~38-byte inputs inside that
/// budget.
///
/// - **64** — the ordinary path: SSE tokenizing, incremental UTF-8 decoding and
///   JSON-RPC classification, the work a healthy stream does.
/// - **8** — the overflow path, reached by any newline-free chunk of 9+ bytes,
///   i.e. by nearly every input libFuzzer generates from its first run.
const MAX_BUFFER_SIZES: [usize; 2] = [64, 8];

/// How the input is sliced into successive "body frames".
///
/// A live session stream is read incrementally, so the SSE line buffer and the
/// undecoded-UTF-8 tail both carry ACROSS chunks: splits land mid-character and
/// mid-line. Slicing at a fixed width keeps a crash artifact deterministic to
/// replay, and 16 bytes means the 64-byte bound is approached by ACCUMULATION
/// (several chunks) rather than only by one oversized chunk.
const CHUNK_LEN: usize = 16;

/// The three members `parse_message` dispatches on. An `Ok` outcome is
/// impossible without at least one of them.
const DISPATCH_MEMBERS: [&str; 3] = ["method", "result", "error"];

/// The longest incomplete UTF-8 character, i.e. the most `take_utf8_prefix` may
/// leave behind.
const MAX_UNDECODED_TAIL: usize = 3;

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
        let (outcomes, overflowed, peak_buffered_bytes, undecoded_tail_bytes) =
            pmcp::shared::streamable_http::decode_sse_chunks_for_fuzz(&chunks, max_buffer_size);

        // Invariant 2: MEMORY STAYS BOUNDED.
        //
        // What this defends: a peer that streams perfectly ordinary
        // newline-terminated `data:` lines and simply never sends the blank line
        // that would dispatch the event must not be able to grow the parser.
        // Each such line COMPLETES — so a "does this chunk carry a newline?"
        // escape hatch waves it through — while its payload accumulates into the
        // pending event's data, which only a BLANK line ever clears. Enforcement
        // discards both accumulators and latches rather than growing, so the
        // bound holds on return from every feed.
        for (index, held) in peak_buffered_bytes.iter().copied().enumerate() {
            assert!(
                held <= max_buffer_size,
                "the parser retained {held} bytes after chunk {index} under a \
                 {max_buffer_size}-byte bound (peaks: {peak_buffered_bytes:?})"
            );
        }
        // The SECOND accumulator, and the one the sibling seam cannot see: the
        // byte buffer feeding the incremental UTF-8 decoder. `take_utf8_prefix`
        // runs in the same iteration as the append, so what survives is at most
        // one incomplete character. A decoder that stopped draining on some
        // invalid sequence would grow this without limit while
        // `buffered_bytes()` stayed perfectly flat.
        for (index, tail) in undecoded_tail_bytes.iter().copied().enumerate() {
            assert!(
                tail <= MAX_UNDECODED_TAIL,
                "the undecoded UTF-8 tail was {tail} bytes after chunk {index}; the longest \
                 incomplete character is {MAX_UNDECODED_TAIL} bytes (tails: \
                 {undecoded_tail_bytes:?})"
            );
        }

        // Invariant 3: nothing is decoded out of bytes that carried no
        // dispatchable member.
        //
        // The precondition is checked against the RAW bytes, which is sound only
        // when no JSON escape could have spelled the member indirectly: a JSON
        // string of `\u`-escaped code points decodes to `method` without those
        // bytes appearing literally, and asserting on such an input would report
        // a SPURIOUS crash. An input containing no backslash at all cannot carry
        // such an escape, so the literal check applies exactly there.
        if outcomes.iter().any(std::result::Result::is_ok) && !text.contains('\\') {
            assert!(
                DISPATCH_MEMBERS.iter().any(|member| text.contains(member)),
                "a message was decoded from chunks that carried none of \
                 {DISPATCH_MEMBERS:?} (bound {max_buffer_size})"
            );
        }

        // Subordinate note, not a numbered invariant: `overflowed()` LATCHES.
        // Once a line has been discarded the stream has lost bytes, so a later
        // chunk must not present it as healthy again. Kept because it is cheap
        // and it documents the latch — but `overflowed` has exactly one write
        // site and no clearing path, so no input can falsify this. It is the
        // assertion that was the sibling target's Invariant 3, and its tautology
        // is why this target needed a real SIZE check.
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
