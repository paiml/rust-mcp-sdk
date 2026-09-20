---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 13
subsystem: api
tags: [mcp-2026-07-28, subscriptions-listen, client, sse, streaming, semver, retired-rpc, fuzz, property]

# Dependency graph
requires:
  - phase: 113-10
    provides: "the SERVER route this consumes — the ack-first frame order, the subscriptionId _meta key, the terminal SubscriptionsListenResult, the -32601 advertise-nothing rejection, and the RAII ListenGuard the client's disconnect fires"
  - phase: 113-05
    provides: "the client v2 opt-in (with_protocol_version), splice_v2_meta, the body-derived Mcp-Method/Mcp-Name emission, and the v2_mode transport latch"
  - phase: 113-07
    provides: "the client-local error pattern — a stable marker in Error::Protocol.data, because pmcp::Error is not #[non_exhaustive]"
provides:
  - "src/client/subscriptions.rs — SubscriptionStream, an SSE-backed futures::Stream<Item = Result<ServerNotification>> with ack-first enforcement, per-frame subscriptionId validation, and RAII teardown"
  - "EventStreamTransport — a narrow additive trait for transports that can open a long-lived server-push stream, implemented only by StreamableHttpTransport"
  - "Client::subscriptions_listen — the v2 client entry point, generic over T: Transport + EventStreamTransport"
  - "Error::retired_on_v2 / is_retired_on_v2 / retired_method / retired_replacement + RETIRED_ON_V2_MARKER"
  - "StreamableHttpTransport::post_streaming (pub(crate)) and the extracted post_once, so the streaming path shares one header emission and one at-most-once 401 retry with post_body"
  - "tests/v2_subscriptions_client.rs — seven live acceptances driving a real pmcp Client against a real pmcp server"
  - "examples/s49_v2_subscriptions_client.rs — a self-contained runnable demonstration"
  - "fuzz/fuzz_targets/subscription_listen_frames.rs — the untrusted-frame boundary"
affects: [113-12, 117, 118, 119]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A narrow capability trait beside the cross-transport Transport trait, so an HTTP-only affordance does not force a meaningless default onto stdio/WebSocket/wasm"
    - "Errors as stream ITEMS, not terminations: a bad frame is surfaced and the stream continues; only transport failure or the terminal result ends it"
    - "Teardown by ownership — the stream owns its HTTP response, so the server's RAII reclaim cannot be skipped on an error path"
    - "Chunk-boundary-safe incremental UTF-8: retain an INCOMPLETE tail, lossily decode a genuinely INVALID one, so hostile bytes cannot wedge the decoder"

key-files:
  created:
    - src/client/subscriptions.rs
    - tests/v2_subscriptions_client.rs
    - examples/s49_v2_subscriptions_client.rs
    - fuzz/fuzz_targets/subscription_listen_frames.rs
  modified:
    - src/client/mod.rs
    - src/error/mod.rs
    - src/shared/streamable_http.rs
    - src/shared/sse_parser.rs
    - Cargo.toml
    - fuzz/Cargo.toml

key-decisions:
  - "subscriptions_listen is generic over a NEW narrow trait (EventStreamTransport), not over another defaulted Transport method: an incrementally-read response body is an HTTP concept, and this keeps the plan's stub-transport send-counter test story reachable while leaving stdio/WebSocket/wasm untouched"
  - "The transport hands back a stream of SSE data PAYLOADS, not parsed values — JSON-RPC classification belongs to the client module, wire framing to the transport"
  - "post_once was EXTRACTED from post_body rather than a second POST path being written, so the long-lived stream inherits the same header emission and the same at-most-once 401 refresh; a hand-rolled sibling would have silently skipped the auth retry"
  - "A malformed / cross-tagged / unmodelled frame is an Err ITEM and the stream CONTINUES; only a transport failure, the terminal result, or end-of-body ends it — one bad frame from an intermediary must not silently drop every later notification"
  - "The retired-RPC error carries METHOD_NOT_FOUND (-32601), the code the server would have answered with, so a caller already branching on method-not-found keeps working while gaining the marker and the named replacement"
  - "The era gate runs BEFORE ensure_initialized and assert_capability: the era is a property of the connection, and neither check is meaningful for a method the wire no longer defines"

patterns-established:
  - "When a property test falsifies an invariant the plan only ASSERTED (here: 'the shared frame parser is reused, so it is already safe'), fix the shared code — the mitigation is void otherwise"
  - "A #[doc(hidden)] decode seam is how a private untrusted-input path becomes fuzzable without growing stable API (the Phase-110 convention)"

requirements-completed: []

# Metrics
duration: 105min
completed: 2026-07-26
---

# Phase 113 Plan 13: The `subscriptions/listen` Client Half Summary

**A v2 pmcp `Client` now opens `subscriptions/listen` and receives typed change notifications — acknowledgement-first, per-frame `subscriptionId`-validated, error-tolerant, and reclaimed by dropping the handle — while the two RPCs the v2 schema retired fail fast locally with an error naming the replacement; and the arbitrary-bytes property test written to guard all that found a real remote-triggerable panic in the shared SSE parser.**

## Performance

- **Duration:** ~105 min
- **Started:** 2026-07-26T01:07:55Z
- **Completed:** 2026-07-26T02:52:42Z
- **Tasks:** 3 (+2 deviation commits)
- **Files modified:** 10 (4 created, 6 modified)

## Accomplishments

- **HTTP-04's client half exists and is proven LIVE.** `tests/v2_subscriptions_client.rs` drives a real `pmcp::Client` against a real `StreamableHttpServer`: the acknowledgement arrives first and carries the AGREED filter, a `tools/list_changed` fired through the server's own `Server::send_notification` path is received as a typed `ServerNotification`, an unrequested type never arrives, and dropping the handle releases the server's slot. Plan 10 proved the wire; this proves the SDK's own client can consume it, which is what the requirement's wording asks for.
- **Cross-delivery cannot reach a caller.** Every frame's `_meta[io.modelcontextprotocol/subscriptionId]` is compared against the stream's own id, and a mismatch is yielded as an `Err` rather than forwarded (T-113-66). This is pinned as an **iff** over the whole `(stream id × frame id × method)` space by a proptest, not just by two example cases.
- **The retired RPCs fail fast, measurably.** `subscribe_resource` / `unsubscribe_resource` on v2 return a typed error naming `subscriptions/listen`, and the live test asserts the server observed **zero** `resources/subscribe` and **zero** `resources/unsubscribe` requests — with a non-vacuity guard proving the same client on the same server does reach it for a method that still exists.
- **A real bug was found and fixed, not filed.** The arbitrary-bytes property test falsified this plan's own T-113-67 mitigation ("the shared frame parser is reused, so no second tokenizer and no new panic surface"): `SseParser::feed` panicked on a char boundary for bytes a remote server chooses. See Deviations — this affected the **pre-existing** GET-SSE path too.
- **The milestone stays additive.** `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` reports `223 checks: 223 pass, 30 skip / Summary no semver update required`. No public enum gained a variant; no constructible struct gained a field.
- **`make quality-gate` PASSED** (`ALL TOYOTA WAY QUALITY CHECKS PASSED`, exit 0), including the ALWAYS validation.

## Task Commits

| # | Task | Commit | Type |
|---|------|--------|------|
| 1 | `SubscriptionStream` — an SSE-backed stream of typed notifications | `e89b9dae` | feat |
| 2 | Era-gate the two retired subscription RPCs on the client | `e8d792b7` | feat |
| 3 | Live proof that a pmcp v2 client receives change notifications | `c9ac1997` | test |
| — | [Rule 2 / CLAUDE.md ALWAYS] property + fuzz + runnable example | `9fcc6cf3` | test |
| — | [Rule 1] `SseParser::feed` char-boundary panic on remote bytes | `408bf7b3` | fix |

## Files Created/Modified

- `src/client/subscriptions.rs` (new, 1160 lines) — `SubscriptionStream` (`subscription_id()`, `acknowledged()`, `impl futures::Stream`), `EventStreamTransport` + its `StreamableHttpTransport` impl, `sse_payload_stream` / `take_utf8_prefix` / `drain_sse_payloads` (the decode), `classify_frame` / `verify_subscription_id` / `decode_notification` (the classification), `rejection_error` (the server's own error, surfaced verbatim), the `#[doc(hidden)]` `decode_listen_chunk_for_fuzz` seam, and 24 tests including 3 proptests.
- `src/client/mod.rs` (+~240) — `pub mod subscriptions`, `Client::subscriptions_listen` (in its own `T: Transport + EventStreamTransport` impl block), `reject_if_retired_on_v2`, the two era-gated retired methods with their "v2 behavior" rustdoc, and 2 new tests.
- `src/error/mod.rs` (+~150) — `RETIRED_ON_V2_MARKER`, `Error::retired_on_v2` / `is_retired_on_v2` / `retired_method` / `retired_replacement`, and 4 tests.
- `src/shared/streamable_http.rs` (+~110/-70) — `apply_post_headers`, the extracted `post_once`, and `post_streaming`.
- `src/shared/sse_parser.rs` (+68/-3) — the char-boundary fix, 2 regression tests and 1 proptest.
- `tests/v2_subscriptions_client.rs` (new, 536 lines) — the seven live acceptances plus the counting middleware.
- `examples/s49_v2_subscriptions_client.rs` (new, 186 lines) — self-contained, verified by running it.
- `fuzz/fuzz_targets/subscription_listen_frames.rs` (new) + `fuzz/Cargo.toml`, `Cargo.toml` — the fuzz target and the example registration.

## Verification

| Command | Result |
|---|---|
| `cargo test --lib --features full -- client::subscriptions` | 24 passed |
| `cargo test --lib --features full -- client` | 170+ passed (0 failed) |
| `cargo test --lib --features full -- sse_parser` | 13 passed |
| `cargo test --test v2_subscriptions_client --features full` | **7 passed** |
| `cargo test --test v2_subscriptions --features full` | 9 passed (plan 10 green) |
| `cargo test --test server_subscriptions --features full` | 6 passed (v1 baseline green) |
| `cargo test --test v2_client --features full` | 21 passed |
| `cargo build --lib --target wasm32-unknown-unknown` | success |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | 223 pass, no update required |
| `cargo run --example s49_v2_subscriptions_client --features full` | exit 0, all four demonstrations |
| `cargo check --bin subscription_listen_frames` (fuzz crate) | success |
| `grep -c 'fn parse_sse\|sse_frame' src/client/subscriptions.rs` | `0` — the parser is IMPORTED, no second tokenizer |
| `make quality-gate` | **PASSED** (exit 0) |

## Decisions Made

1. **`EventStreamTransport`, not a fourth defaulted `Transport` method.** Plan 05 established defaulted `Transport` methods as the additive cross-transport seam, and the obvious move was a fourth one. Rejected: an incrementally-read response body is an HTTP concept, and stdio / WebSocket / wasm would all have to carry a meaningless default (in two trait definitions, `Send` bounds differing between them). A narrow trait implemented by exactly one transport is equally additive, keeps `Client::subscriptions_listen` **generic** — so the plan's stub-transport send-counter test is reachable exactly as written — and adds no obligation to any existing implementor.
2. **`post_once` extracted rather than a second POST path written.** The streaming open needs the same three v2 headers, the same middleware-built request, and the same at-most-once 401 refresh. A parallel implementation would have looked correct and silently skipped the auth retry on every long-lived stream. `post_body` now calls the same extracted head; its behavior is unchanged (its 21 transport tests plus `v2_client`'s 21 live tests are green).
3. **Errors are stream ITEMS.** A malformed frame, an unmodelled notification method, or a cross-tagged frame yields `Some(Err(..))` and the stream keeps going; only a transport failure, the terminal result, or end-of-body ends it. A single bad frame from a buggy intermediary must not silently drop every subsequent notification — and the tests assert recovery explicitly, not just the error.
4. **`METHOD_NOT_FOUND` for the retired-RPC error.** It is the code the server WOULD have answered with, so a caller that already branches on method-not-found keeps working; the marker and the named replacement are the additive part. (The two MRTR client-local errors use `INTERNAL_ERROR` because they describe a local give-up with no server counterpart; this one has one.)
5. **The era gate runs first.** Before `ensure_initialized` and before `assert_capability`: the era is a property of the connection, and a capability assertion about a method the wire no longer defines is noise.
6. **The transport yields payload STRINGS.** SSE framing is the transport's business; JSON-RPC classification is the client module's. That split is what makes every classification test drivable from a canned `Vec<Result<String>>` with no socket.

## Deviations from Plan

### Auto-fixed / added

**1. [Rule 1 - Bug] `SseParser::feed` panicked on a char boundary, for bytes a REMOTE server chooses**

- **Found during:** post-task `make quality-gate` — by this plan's own `arbitrary_bytes_never_panic_the_decoder` proptest, which failed on the first full run.
- **Issue:** The CRLF check read `self.buffer.chars().nth(line_end - 1)` — a CHARACTER index — while `line_end` is the BYTE index `str::find('\n')` returns. On any buffer containing a multi-byte character the two disagree, so the check could report `'\r'` for a position that was not byte `line_end - 1`, and the slice that followed (`self.buffer[..line_end - 1]`) cut INSIDE a character: `byte index N is not a char boundary`. Minimal trigger: `"\u{2602}\n\rX\n"`.
- **Why it is in scope:** this plan's threat register lists T-113-67 (a malformed or hostile SSE frame must not panic or wedge the client) as `mitigate`, and the stated mitigation is "the shared frame parser is reused (no second tokenizer)". If the shared parser panics on remote input, that mitigation is void — and the bug also reached the **pre-existing** `StreamableHttpTransport::start_sse` GET path, so it was a remote-triggerable client crash before this plan existed.
- **Fix:** compare the byte directly (`self.buffer.as_bytes()[line_end - 1] == b'\r'`). `\n` and `\r` are ASCII, so both `line_end` and `line_end - 1` (taken only when that byte IS `\r`) are guaranteed char boundaries. Line-splitting behavior is otherwise unchanged.
- **Files modified:** `src/shared/sse_parser.rs`
- **Verification:** `feed_does_not_panic_on_a_multibyte_char_before_a_later_cr` (the exact shape), `feed_strips_crlf_and_preserves_multibyte_data` (CR is still stripped on the happy path), `feed_never_panics_on_arbitrary_text` (proptest over multi-chunk multi-byte input); all 13 `sse_parser` tests and the full `make quality-gate` green.
- **Committed in:** `408bf7b3`

**2. [Rule 2 - CLAUDE.md ALWAYS] Property, fuzz and example coverage**

- **Found during:** post-task CLAUDE.md compliance check.
- **Issue:** CLAUDE.md mandates FUZZ + PROPERTY + UNIT + a runnable EXAMPLE for every new feature. The plan specified unit tests and a live test only.
- **Fix:** three proptests (the delivery **iff** over the id space; never-panic on arbitrary bytes; never-panic on SSE-shaped text), `fuzz/fuzz_targets/subscription_listen_frames.rs` asserting both no-panic and no-cross-delivery at the untrusted boundary, and `examples/s49_v2_subscriptions_client.rs`. The fuzz target reaches the private decode through `decode_listen_chunk_for_fuzz`, a `#[doc(hidden)]` internal support seam (the Phase-110 convention), so no stable API was added.
- **Files modified:** `src/client/subscriptions.rs`, `fuzz/fuzz_targets/subscription_listen_frames.rs`, `fuzz/Cargo.toml`, `examples/s49_v2_subscriptions_client.rs`, `Cargo.toml`
- **Verification:** 24 lib tests, `cargo check --bin subscription_listen_frames`, and the example run to exit 0 with all four demonstrations printing. **This deviation is what found deviation 1.**
- **Committed in:** `9fcc6cf3`

**3. [Rule 3 - Blocking] `subscriptions_listen` needed a transport seam that does not exist**

- **Found during:** Task 1.
- **Issue:** `Client<T>` is generic and `Transport` has no way to hand back a response body UNREAD. `post_body` collects to completion and would hang forever on a stream that never ends. Plan 05's `send_raw` cannot help: it returns `Result<()>`.
- **Fix:** `EventStreamTransport` (a narrow additive trait) plus `pub(crate) StreamableHttpTransport::post_streaming`. See Decision 1 for why this is preferred to a fourth defaulted `Transport` method.
- **Files modified:** `src/client/subscriptions.rs`, `src/shared/streamable_http.rs`, `src/client/mod.rs`
- **Verification:** `cargo semver-checks` 223/223 with no new obligation on existing `Transport` implementors; the stub-transport tests exercise the trait directly.
- **Committed in:** `e89b9dae`

**4. [Rule 2 - Missing Critical] Chunk-boundary-safe UTF-8 decoding**

- **Found during:** Task 1.
- **Issue:** `SseParser::feed` takes `&str`, and a TCP chunk boundary can fall in the MIDDLE of a multi-byte character. A naive `String::from_utf8_lossy` per chunk would silently corrupt any non-ASCII resource URI travelling on the stream — and the plan's live tests, being ASCII, would never have caught it.
- **Fix:** `take_utf8_prefix` splits off the longest decodable prefix, RETAINS an incomplete tail for the next chunk, and lossily decodes genuinely invalid bytes immediately (retaining those would wedge the stream on hostile input).
- **Files modified:** `src/client/subscriptions.rs`
- **Verification:** `a_multibyte_character_split_across_chunks_survives`, `invalid_bytes_do_not_wedge_the_decoder`.
- **Committed in:** `e89b9dae`

**5. [Rule 2 - Missing Critical] `retired_replacement()` accessor**

- **Found during:** Task 2.
- **Issue:** The plan specifies `retired_method()` only. The replacement API name is already in `data` and is the actionable half of the error; leaving it readable only by parsing the `Display` string would invite exactly that.
- **Fix:** added `Error::retired_replacement()` alongside.
- **Files modified:** `src/error/mod.rs`
- **Verification:** `it_is_identifiable_and_carries_both_names`; semver-checks unchanged.
- **Committed in:** `e8d792b7`

---

**Total deviations:** 5 (1 real bug found by a test this plan added, 2 missing-critical additions, 1 blocking seam, 1 CLAUDE.md ALWAYS compliance)
**Impact on plan:** No scope creep. The plan's shape — one stream type, one method, one era gate — is intact. Deviation 3 substitutes a mechanism the plan did not name but its acceptance criteria require (a generic method testable with a stub transport); deviation 1 is a fix to shared code whose safety the plan's threat register explicitly assumed.

## Deltas against the plan's acceptance criteria

Every criterion is met. Two are worth spelling out because they were stated as `grep` shapes:

- `grep -c 'fn parse_sse\|sse_frame' src/client/subscriptions.rs` → **`0`**. The parser is `crate::shared::sse_parser::SseParser`, imported; there is no second tokenizer. (The plan's prose says "imported from `src/shared/streamable_http.rs`" — that file itself imports `SseParser` from `src/shared/sse_parser.rs`, which is the one definition. Importing the definition rather than re-exporting it through the transport is the same reuse.)
- "`impl futures::Stream for SubscriptionStream`" → present verbatim as `impl Stream for SubscriptionStream` with `use futures::Stream`.

## Threat Register Coverage

| Threat | Mitigation as shipped |
|---|---|
| T-113-66 (a foreign `subscriptionId` delivered as the caller's own) | `verify_subscription_id` runs on EVERY frame including the acknowledgement and the terminal result; a mismatch is an `Err` item. Pinned as an **iff** by proptest over the id space and by the fuzz target's second invariant |
| T-113-67 (a malformed or hostile frame panicking or wedging the client) | The shared `SseParser` is reused (and its char-boundary panic FIXED — deviation 1); a malformed frame is an `Err` item that does not terminate; keep-alive comments are dropped by the shared parser; invalid UTF-8 is consumed rather than retained; error messages truncate untrusted text at 200 chars; every test poll is timeout-bounded |
| T-113-63 (leaked server registry slots) | The stream OWNS its HTTP response; dropping it closes the connection and fires plan 10's `ListenGuard`. `client_stream_drop_releases_server_slot` proves the end-to-end reclaim through the real client |
| T-113-68 (a v2 application silently relying on retired RPCs) | `reject_if_retired_on_v2` before any I/O; the live test asserts a server-observed count of `0` for both methods, with a non-vacuity guard |
| T-113-34 (receiving unrequested notification types) | Server-side filter intersection (plan 10) plus `client_does_not_receive_unrequested_types`, which fires the unrequested type FIRST and asserts it never arrives |

## Known Stubs

**None.** No hardcoded empty values, placeholder text, or unwired data sources were introduced.

## Threat Flags

**None.** The only new network surface is the client SIDE of `subscriptions/listen`, which is the plan's subject and is covered by the register above. No new endpoint, auth path, file access pattern, or schema change was introduced.

## Issues Encountered

- **`cargo test --test v2_subscriptions_client` needed a full rebuild** the first time (~2.5 min) because the new integration target pulls the whole `full` feature set. Subsequent runs are ~2 s.
- **The per-principal stream cap does not bind for an anonymous caller** (plan 10's documented cost: `anonymous_principal` is a per-stream counter). `client_stream_drop_releases_server_slot` therefore authenticates with a `Bearer` header so the cap is reachable in a handful of streams instead of 64. Recorded in the test file's module docs.
- **`.pmat/*`, `pmcp-course/*` and `.planning/config.json` show as modified** in the working tree. They pre-date this plan and were deliberately NOT staged, per the executor scope boundary.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Plan 12 (public-API + semver audit)** should note the newly public surface: `pmcp::client::subscriptions::{SubscriptionStream, EventStreamTransport, SubscriptionFrameStream}`, `Client::subscriptions_listen`, and `Error::{retired_on_v2, is_retired_on_v2, retired_method, retired_replacement}` + `RETIRED_ON_V2_MARKER`. Current measurement: `223 checks: 223 pass, 30 skip / no semver update required`.
- **Phase 117 (`mcp-tester` / `pmcp-agent` on v2)** can consume `subscriptions_listen` directly; the stream is `Send` and its teardown is ownership, so it drops cleanly out of any agent loop.
- **Phase 118 (conformance)** now has a client-side consumer for any `subscriptions/listen` scenario, in addition to plan 10's raw-wire mirror.
- **Phase 119 (docs)** has a runnable, self-contained example (`s49_v2_subscriptions_client`) that demonstrates the retired-RPC migration path in the same breath as the replacement API.
- **Deliberately out of scope, unchanged from plan 10:** cross-instance notification delivery. The client-side rustdoc restates D-11 (polling over Tasks remains the recommended enterprise mechanism) and names the instance-local constraint.

## Self-Check: PASSED

Files verified on disk: `src/client/subscriptions.rs` (1160 lines, min 140, contains `SubscriptionStream`), `tests/v2_subscriptions_client.rs` (536 lines, min 130), `examples/s49_v2_subscriptions_client.rs`, `fuzz/fuzz_targets/subscription_listen_frames.rs`, this SUMMARY.

Commits verified in `git log`: `e89b9dae`, `e8d792b7`, `c9ac1997`, `9fcc6cf3`, `408bf7b3`.

Key links verified: `subscriptions_listen` present in `src/client/mod.rs`; `retired_on_v2` present in both `subscribe_resource` and `unsubscribe_resource` (via `reject_if_retired_on_v2`); `impl Stream for SubscriptionStream` present in `src/client/subscriptions.rs`.

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-26*
