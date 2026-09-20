---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 15
subsystem: transport
tags: [sse, sse-parser, dos, bounded-buffer, subscriptions-listen, http-transport, semver-additive]

# Dependency graph
requires:
  - phase: 113-stateless-http-multi-round-trip-elicitation
    provides: "plan 13's `subscriptions/listen` client half (`SubscriptionStream`, `drain_sse_payloads`) and its char-boundary fix in the shared `SseParser::feed`"
provides:
  - "a BOUNDED `SseParser` line buffer whose default (1 MiB) is sourced from `SseConfig::default().max_buffer_size` — the config field's first real reader"
  - "`SseParser::with_max_buffer_size` (additive constructor) and `SseParser::overflowed` (latching observer)"
  - "overflow observation at BOTH incremental feeders: the `subscriptions/listen` consumer (256 KiB, stream-ending -32600) and `HttpTransport::connect_sse` (1 MiB default, log-and-break)"
affects: [113-16 fuzz campaign, any future incremental SseParser feeder]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "bound-inside-the-parser + latching flag: one enforcement point covers every present and future feeder; the observation is per-caller"
    - "predicate-as-free-function so an untestable async call site (owning a `hyper::body::Incoming`) still has its decision covered by a unit test"

key-files:
  created: []
  modified:
    - src/shared/sse_parser.rs
    - src/client/subscriptions.rs
    - src/shared/http.rs

key-decisions:
  - "Option A chosen: the bound lives INSIDE `SseParser` with a latching `overflowed()` flag, not at the consumer (option B, bounds the wrong buffer and leaves `max_buffer_size` dead), not as a `Result` return (option C, MAJOR semver break), and never as silent truncation (option D, produces a corrupted-but-parseable frame and a misleading downstream JSON error)"
  - "Enforcement fires only when NO line can be completed by this chunk (neither the buffer nor `data` carries a `\\n`), which is exactly what leaves the two whole-body `feed` call sites in `streamable_http.rs` behaviourally unchanged"
  - "The overflow DISCARDS the oversized partial line and resets the in-progress event, but deliberately leaves `last_event_id` alone — that is stream-level resumption state, not line state"
  - "The flag latches across `reset()` too: resetting line state cannot un-lose bytes a peer already sent"
  - "`HttpTransport::connect_sse` keeps the shared 1 MiB default; tightening the effective payload ceiling of a pre-existing exported transport that carries arbitrary JSON-RPC results is out of this gap's scope"
  - "Both overflow checks are free functions over the parser (`listen_overflow`, `report_sse_line_overflow`) because their call sites own a live `hyper::body::Incoming`, which cannot be constructed outside hyper — the tests drive the production predicates, not reconstructions of them"

patterns-established:
  - "Latching-observer bound: a `pub` parser gains a bound + `overflowed()` observer instead of a fallible `feed`, keeping the change semver-additive while making the failure loud"
  - "Per-caller overflow policy: one shared bound, but each long-lived feeder decides its own tightness and its own failure surface (typed error vs log-and-break)"

requirements-completed: [HTTP-04]

# Metrics
duration: 36min
completed: 2026-07-26
---

# Phase 113 Plan 15: Bounded SSE Line Buffer Summary

**`SseParser`'s line buffer is now bounded at a configurable, `SseConfig`-sourced 1 MiB, an overflow latches an observable flag instead of growing the heap, and both long-lived incremental feeders — the `subscriptions/listen` consumer at a tighter 256 KiB and `HttpTransport::connect_sse` at the default — end their stream with a named error rather than silently discarding.**

## Performance

- **Duration:** 36 min
- **Started:** 2026-07-26T17:41:07Z
- **Completed:** 2026-07-26T18:17:05Z
- **Tasks:** 2 (Task 1 TDD: RED + GREEN)
- **Files modified:** 3

## Accomplishments

- **Closed verification gap item 3 / review CR-03 (T-113-73).** A hostile or broken v2 server that accepts `subscriptions/listen` and then streams without ever emitting a newline can no longer grow a pmcp client's heap without limit. Before this plan it grew to exactly the number of bytes fed — the RED test measured 2 MiB from 2 MiB.
- **`SseConfig::max_buffer_size` stopped being decoration.** It had *no consumers anywhere in the crate*; `SseParser::new()` now takes its bound from `SseConfig::default().max_buffer_size`, so there is one source for the 1 MiB number and the doc comment names its readers.
- **Overflow is loud, not silent.** The parser discards the oversized line (never truncate-and-emit, which would surface later as a misleading JSON parse failure), latches `overflowed()`, and both incremental feeders act on it.
- **Found and honoured a caller the original enumeration nearly missed.** `HttpTransport::connect_sse` is a SECOND incremental feeder (one parser per spawned reader task, fed frame by frame), exported from the crate root and consumed in-repo by `pmcp-team-servers`. Bounding without observing there would have converted correct-but-unbounded behaviour into a silent discard — a regression. It now logs the bound and breaks the reader loop.
- **The whole change is semver-additive:** a new constructor, a new observer, and two PRIVATE fields. `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` reports 223 pass / 30 skip / *no semver update required*, so the v2.5 milestone stays a 2.x minor.

## Task Commits

1. **Task 1 (RED): a newlineless remote flood grows `SseParser` without limit** — `7b4ea073` (test)
2. **Task 1 (GREEN): bound the SSE line buffer and make overflow observable** — `63f68a7a` (feat)
3. **Task 2: both incremental SSE feeders fail loudly on overflow** — `5ddd1b48` (feat)

_The RED commit deliberately contains only the bound test that COMPILES against HEAD (it asserts on the private `buffer` field, so no new API is referenced) and fails at runtime with "the line buffer grew to 2097152 bytes, past the 1 MiB bound". Tests referencing `with_max_buffer_size`/`overflowed()` would not have compiled, and CLAUDE.md's zero-tolerance stance rules out committing a non-building tree._

## Files Created/Modified

- `src/shared/sse_parser.rs` — two private fields (`max_buffer_size`, `overflowed`); `new()` delegating to the new `with_max_buffer_size`; the `overflowed()` latching observer; the bound enforcement block at the top of `feed`; `SseConfig::max_buffer_size` and `reset()` docs naming the new contract; 7 new tests including a bounded-parser property case.
- `src/client/subscriptions.rs` — `MAX_LISTEN_LINE_BYTES` (256 KiB) with its rationale; `sse_payload_stream` constructing the parser with that explicit bound; the `listen_overflow` predicate; `read_next_frame` ending the stream on it; 3 new tests.
- `src/shared/http.rs` — `report_sse_line_overflow`, called by `connect_sse`'s reader loop after every `feed`, logging the 1 MiB bound and breaking; 2 new tests.

## Decisions Made

All four options in the plan's `<design_decision>` were re-verified before implementing, in particular the caller enumeration:

| `feed` caller | Feeds | Guarded here |
|---|---|---|
| `streamable_http.rs:528` (POST response) | a COMPLETE collected body | unreachable — bodies carry `\n` and drain in one call |
| `streamable_http.rs:1150` (`start_sse` GET) | a COMPLETE collected body | unreachable, same reason |
| `client/subscriptions.rs::drain_sse_payloads` | one chunk at a time | Task 2 — 256 KiB, stream-ending error |
| `shared/http.rs::connect_sse` | one chunk at a time | Task 2 — 1 MiB default, log + break |
| `fuzz_targets/subscription_listen_frames.rs` | arbitrary bytes | plan 113-16 |

Beyond the plan's recorded decisions, two implementation choices were made:

- **`reset()` does not clear the latch.** The plan specified latching semantics but did not mention `reset`. Clearing it there would let a caller un-see a data loss that already happened, so `reset()` clears line state only, and its rustdoc says so.
- **The residual buffer may transiently exceed the bound by at most one chunk's tail.** When a chunk DOES contain a `\n`, it is admitted and drained, and whatever follows its last newline stays buffered. The next newline-free chunk then trips the bound. The invariant that actually matters — memory is a function of one chunk, not of stream age — is asserted directly in the new property test (`buffer.len() <= max(bound, chunk.len())`).

## Deviations from Plan

None — plan executed exactly as written. Every step of both `<action>` blocks was followed, including the negative verification (see below). Two refinements *within* the plan's instructions are worth naming:

- Task 2 step 2 said to check `parser.overflowed()` inline in `read_next_frame`. It is checked there, but through a `listen_overflow(&parser) -> Option<Error>` free function rather than a bare `if`, mirroring the shape `report_sse_line_overflow` needed in `http.rs`. This is what makes the plan's required "unit test per path driving a deliberately tiny parser" test the PRODUCTION predicate — `read_next_frame` and the reader task both own a live `hyper::body::Incoming`, which cannot be constructed outside hyper, so an inline check would only have been testable by reconstruction.
- One doc line was reworded (`OOMed` → `ran out of memory`) because `make lint`'s pedantic `doc_markdown` treats it as an item needing backticks.

**Total deviations:** 0
**Impact on plan:** none.

## Issues Encountered

- **`make quality-gate`'s fuzz stage still builds 0 of 17 targets** (`error: the option 'Z' is only accepted on the nightly compiler`) and swallows the failure via `|| echo`. This is the known, ownerless D-113-G, explicitly OUT OF SCOPE for this plan (the plan forbids editing the Makefile). The gate still exits 0.
- **The rtk shell proxy truncated the captured gate log** at ~593 lines with a `... (5954 lines truncated)` marker, so the log tail is not readable. Exit-code propagation through the proxy was verified independently (`make definitely-not-a-target` → exit 2), so `GATE-EXIT=0` is trustworthy; the individual stages were also re-run directly (see Verification).

## Verification

All 13 steps of the plan's `<verification>` block, in order:

| # | Check | Result |
|---|---|---|
| 1 | `cargo test --lib --features full -- sse_parser` | 20 passed, 0 failed (13 at HEAD → +7) |
| 2 | `cargo test --lib --features full -- client::subscriptions` | 27 passed, 0 failed |
| 3 | `cargo test --doc --features full -- sse_parser` | 8 passed, 0 failed |
| 4 | `cargo test --lib --features full -- shared::http` | 21 passed, 0 failed |
| 5 | `cargo test --test v2_subscriptions_client --features full` | 7 passed, 0 failed |
| 6 | `cargo test --test v2_subscriptions --features full` | 10 passed, 0 failed (server side untouched) |
| 7 | `cargo test --test v2_stateless_http --features full` | 23 passed, 0 failed (whole-body `feed` path unaffected) |
| 8 | `cargo build -p pmcp-team-servers --all-features` | exit 0 (in-repo `HttpTransport` consumer) |
| 9 | `cargo run --example s49_v2_subscriptions_client --features full` | exit 0, "all demonstrations behaved as documented" |
| 10 | `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | 223 pass, 30 skip, **no semver update required** |
| 11 | `cargo build --lib --target wasm32-unknown-unknown` | success |
| 12 | `git diff --name-only -- Cargo.toml Cargo.lock` | empty (T-113-SC: no package installed, no manifest touched) |
| 13 | `make quality-gate` | exit 0 |

**Negative verification (plan acceptance criterion):** with the enforcement block in `feed` temporarily disabled (`if false && ...`), **5 tests fail** — `a_newlineless_flood_cannot_grow_the_buffer_past_the_bound`, `with_max_buffer_size_bounds_at_the_value_given`, `the_overflow_flag_latches`, `events_completed_before_an_oversized_line_are_still_returned`, and the `a_bounded_feed_never_panics_on_arbitrary_text` property. The block was then restored and all 20 pass again.

**Acceptance greps:** `max_buffer_size` in `sse_parser.rs` = 19 (was 2, required ≥ 6); `MAX_LISTEN_LINE_BYTES` in `subscriptions.rs` = 9 (required ≥ 3); `.overflowed()` present in both `subscriptions.rs` and `http.rs`.

## Threat Model Disposition

| Threat ID | Disposition | Evidence |
|---|---|---|
| T-113-73 (DoS, `SseParser::feed`) | mitigated | bound enforced in `feed`; RED test measured the unbounded growth, 5 tests fail without the fix |
| T-113-74 (DoS, listen consumer) | mitigated | `MAX_LISTEN_LINE_BYTES` = 256 KiB + stream-ending `-32600` |
| T-113-75 (Tampering, overflow handling) | mitigated | the partial line is DISCARDED and `current_event` reset — never truncate-and-emit |
| T-113-78 (DoS, `connect_sse`) | mitigated | `report_sse_line_overflow` logs the bound and breaks the reader task |
| T-113-76 (Info disclosure, error text) | accepted as planned | both messages name only the limit and the peer's behaviour; no frame content is echoed |
| T-113-SC (supply chain) | accepted as planned | `git diff --name-only -- Cargo.toml Cargo.lock` is empty |

## Known Stubs

None. No placeholder values, no unwired data paths, no TODO/FIXME introduced.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Plan 113-16 (the libFuzzer campaign) is unblocked.** It depended on this bound landing: `fuzz_targets/subscription_listen_frames.rs` feeds arbitrary bytes to a `SseParser::new()` and previously had no bound to exercise. Note that `decode_listen_chunk_for_fuzz` still constructs a DEFAULT-bounded parser (1 MiB), not the listen path's 256 KiB — 113-16 may want to align it so the campaign reaches the enforcement branch cheaply.
- **Phase 113 remains BLOCKED ON PUBLICATION, unchanged by this plan.** HTTP-01..05 / CLNT-01..02 stay `[~]` under the `113-SPEC-RECHECK.md` recorded exception; no requirement checkbox was flipped here.
- **Still open, unchanged:** D-113-F (two pre-existing cog-25 violations in `streamable_http_server.rs`), D-113-G (the fuzz stage building 0 of 17 targets), and UNAS-01.

## Self-Check: PASSED

All 3 modified files exist on disk; all 4 commits (`7b4ea073`, `63f68a7a`, `5ddd1b48`, `7d5aae42`) are present in the repository history.

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-26*
