---
phase: 113-stateless-http-multi-round-trip-elicitation
verified: 2026-07-28T00:44:06Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
verified_at_commit: c9944a65
re_verification:
  # ---------------------------------------------------------------------------
  # Round 1 — 2026-07-26T21:40:00Z (preserved verbatim; do not edit)
  # ---------------------------------------------------------------------------
  - round: 1
    verified: 2026-07-26T21:40:00Z
    previous_status: gaps_found
    previous_score: 4/5 must-haves verified
    resulting_status: gaps_found
    resulting_score: 4/5 must-haves verified
    gaps_closed:
      - "Gap item 3 (unbounded SSE client buffer on newline-carrying floods) is now genuinely closed by 113-17. Independently re-read src/shared/sse_parser.rs:365-419: SseParser::feed's pre-check is unconditional over `buffered_bytes() + data.len()` — the `!data.contains('\\n')` escape that let 899,999 bytes accumulate under a 64-byte bound is gone, replaced by an unconditional discard-and-latch. A second, independently-sufficient post-drain check covers the residual. No contains('\\n') gate remains anywhere in the enforcement path."
      - "The 113-14-introduced regression (an ordinary client reconnect after an ungraceful disconnect was refused as a duplicate with a non-retryable HTTP 400) is resolved by 113-18, by contract rather than by the liveness-reclaim originally planned. Independently re-read src/server/subscriptions.rs:396-434: all three ListenRejection variants (PerPrincipalLimit, GlobalLimit, DuplicateSubscriptionId) now map to RATE_LIMITED (-32005), and streamable_http_server.rs:690-702 confirms RATE_LIMITED is absent from the 400 arm of v2_status_for_code, so it resolves to HTTP 200 -- a retryable shape instead of the previous terminal 400. pmcp's own Client::subscriptions_listen mints a fresh Uuid::new_v4() request id on every call (independently confirmed), so this client can never collide with itself; a third-party client that reuses an id now gets a retryable signal instead of a misleading hard error. This is an architecturally-justified resolution (the receiver and the RAII guard share one stream::unfold state tuple server-side, so remote-peer liveness is genuinely unobservable at that layer) documented with its own evidence, not a silent scope cut."
    gaps_remaining: []
    regressions:
      - "NONE of the four gap-closure plans' own changes regressed anything I could independently verify -- the four protected 113-14 tests are unchanged, the fresh-id tripwire fires correctly, and the newline-flood/oversized-line SSE tests pass. However, a FRESH code review (committed 57aad7bc, same day as this re-verification) found three BLOCKER-severity defects in code this phase's gap-closure round directly touched, which I independently reproduced by reading the current source (not by trusting the review): CR-01 (src/client/subscriptions.rs:147-166, an uncapped body.collect().await on the subscriptions/listen client's non-stream rejection path), CR-02 (src/shared/sse_parser.rs:164-189, take_utf8_prefix is O(n^2) on invalid UTF-8 bytes and runs BEFORE every byte-count bound this phase added, reachable via read_next_frame at client/subscriptions.rs:237 on the same subscriptions/listen stream), and CR-03 (src/shared/http.rs:346-356, HttpTransport::send_request collects an unbounded peer-controlled body with zero cap, in the same file 113-17 hardened the SSE reader of). These are not regressions caused by 113-17/18/19/20's edits -- CR-02 and CR-03 predate this round and CR-01 was never in scope of any of the four plans -- but they are real, currently-shipping defects that directly falsify this round's own documentation (DEFAULT_MAX_COLLECTED_BODY_BYTES's rustdoc states 'every one of this transport's response reads is a whole-body read... and the peer chooses how many bytes it sends', which is false for the HttpTransport transport entirely and false for one StreamableHttpTransport-adjacent path). They keep Success Criterion 3's 'memory-bounded' claim not fully true."
  # ---------------------------------------------------------------------------
  # Round 2 — 2026-07-28T00:44:06Z (this round)
  # ---------------------------------------------------------------------------
  - round: 2
    verified: 2026-07-28T00:44:06Z
    previous_status: gaps_found
    previous_score: 4/5 must-haves verified
    resulting_status: passed
    resulting_score: 5/5 must-haves verified
    scope_note: >-
      Twenty plans landed between round 1 and round 2: the Phase-113 gap-closure round
      113-21..113-32 (2026-07-27) and all six plans of the inserted Phase 113.1 "Merge Unblock"
      (2026-07-27). Round 2 re-measured CR-01/CR-02/CR-03 from current source by symbol (line
      numbers had moved), independently re-implemented the pre-fix algorithmic shapes to test the
      falsifiability of the committed complexity guards rather than accept the recorded numbers,
      and spot-checked the four previously-VERIFIED criteria for regression.
    gaps_closed:
      - "CR-01 CLOSED. src/client/subscriptions.rs `EventStreamTransport::open_event_stream` (rejection branch, now lines 135-150) no longer calls `body.collect().await`. It calls `self.collect_capped_body(response).await`, which delegates to `StreamableHttpTransport::collect_body_within_cap` at the transport's configured `max_collected_body_bytes` — a Content-Length pre-check plus a streaming `http_body_util::Limited` bound, so an over-cap body is never allocated whole. `rejection_error` (now line 171) was re-signatured to take ALREADY-COLLECTED `&[u8]`, structurally removing the site where the unbounded read lived. The secondary character-vs-byte issue is also addressed: `truncate` (line 333) now scans at most MAX_ECHOED_FRAME+1 char_indices instead of walking the whole untrusted string with `chars().count()`. Verified by direct source read at HEAD c9944a65."
      - "CR-02 CLOSED. src/shared/sse_parser.rs `take_utf8_prefix` (now line 175) is a single-pass cursor scan: a `consumed` offset advances, `from_utf8` re-validates only `&buffer[consumed..]` (and returns Err at the first bad byte, so the per-iteration cost is the valid run it just consumed), and there is exactly ONE mutation of the buffer per call — `buffer.clear()` on the all-valid exit or a single `buffer.drain(..consumed + valid_up_to)` on the incomplete-tail exit. The per-invalid-run `drain` inside the loop is gone. INDEPENDENTLY MEASURED, not accepted from the SUMMARY: I re-implemented the pre-fix shape (re-validate from index 0, one drain per invalid run) in a standalone opt-level-0 harness and ran both over 1 MiB of 0xFF — committed 21.5 ms vs pre-fix shape 8.89 s, and over a 4x input step committed 4.17x vs pre-fix 17.47x, with byte-identical output on both shapes and parity with `String::from_utf8_lossy`. The committed in-tree guards then measured 37.9 ms (ceiling 1 s) and 4.16x (ceiling 8.0x) on this machine."
      - "CR-03 CLOSED. src/shared/http.rs `HttpTransport::send_request` (now line 416; the read at 457) no longer does a bare `response.collect().await...to_bytes()`. It calls `Self::collect_body_within_cap(response, self.max_collected_body_bytes)` — a sibling of the StreamableHttpTransport helper with the same two independently-sufficient refusals (declared Content-Length over cap refused before a byte is read; delivered bytes read through `Limited`). The cap is a real private field defaulting to `DEFAULT_HTTP_COLLECTED_BODY_BYTES` (16 MiB, line 125) with an additive builder `with_max_collected_body_bytes` (line 252). Verified by direct source read."
      - "D-113-R CLOSED (the second, per-CALL quadratic that round 1 did not know about). src/shared/sse_parser.rs `drain_complete_lines` (now line 482) starts its `find('\\n')` at `search_from = scan_start` — the buffer length BEFORE this chunk was appended — instead of restarting at 0 every call, and performs one `drain(..consumed)` for the whole call. The per-call `debug_assert!(!self.buffer.contains('\\n'))` that was itself an O(retained) scan on every call was removed in the same atomic change, with the invariant pinned by a named test instead. INDEPENDENTLY MEASURED: my standalone harness of both scan windows over 512 KiB fed one byte per call gave committed 29.2 ms vs pre-fix 4.26 s, and 3.98x vs 15.26x over a 4x step, with identical line counts across 1-char chunking. The committed in-tree guards measured 53.2 ms (ceiling 1 s) and 4.02x (ceiling 8.0x) on this machine."
      - "D-113-Q CLOSED. src/shared/sse_optimized.rs `OptimizedSseTransport::connect_sse` no longer does an unbounded `reqwest::Response::text().await`. `collect_sse_text_within_cap` (line 280) accumulates `reqwest::chunk()` frames against a running total, refusing with `chunk.len() > max_bytes - accumulated.len()` BEFORE each append (underflow-safe by the stated loop invariant), at `DEFAULT_HTTP_SSE_BUFFERED_BYTES`. The transport is additionally `#[deprecated]` toward `StreamableHttpTransport`. `WHOLE_BODY_ALLOWLIST` in tests/v2_bounded_reads_tripwire.rs:591 is now literally `&[]`, and a test asserts its length is 0 as a ratchet floor."
    regressions:
      - "NONE FOUND. The four previously-VERIFIED criteria were spot-checked for regression rather than re-derived. Every mechanism round 1 verified is still intact at HEAD: `SseParser::feed`'s bound is still unconditional over `buffered_bytes() + data.len()` with no `contains('\\n')` escape and a second post-drain check (lines 408-433); all three `ListenRejection` variants still map to `RATE_LIMITED` (subscriptions.rs:439) and `RATE_LIMITED` is still absent from `v2_status_for_code`'s 400 arm (streamable_http_server.rs:690-702), so the refusal is still retryable HTTP 200; `Client::subscriptions_listen` still mints a fresh `Uuid::new_v4()` per call (client/mod.rs:504,797) with its tripwire at tests/v2_subscriptions_client.rs:292-335. `make quality-gate` is GREEN end-to-end (exit 0). Targeted suites green: v2_bounded_reads_tripwire 13/13, v2_mrtr 34/34, v2_mrtr_ingress 12/12, v2_stateless_http 23/23, v2_subscriptions 19/19, v2_subscriptions_client 8/8, sse_parser unit module 34/34."
      - "113-26 and 113-27 changed request-identity and answer-typing semantics with a stated blast radius ('rejects requests that previously succeeded'). Examined: both are security narrowings that reject only MALFORMED/AMBIGUOUS input, not correct input. 113-26 deletes `write_canonical`'s `__mrtr_depth_capped__` aliasing marker (verified absent from src/types/mrtr.rs) and makes the canonicalizer fallible past `MAX_CANONICAL_DEPTH` = 64 (line 853), so two requests agreeing to depth 64 can no longer share one AAD digest; params nesting deeper than 64 now get a typed -32602 at BOTH the verify and mint paths. 113-27 seals the server's requested `InputRequestKind`s inside the AEAD continuation as `Option<InputRequestKinds>` (request_state.rs:559) — ABSENT means pre-kinds/degrade, EMPTY means reject-everything, deliberately not conflated — and re-decodes answers kind-directed on the `Proceed` arm only, i.e. strictly AFTER the AEAD tag check, so the enforced kinds cannot be client-chosen. The MRTR end-to-end proofs still pass: `client_server_mrtr_three_rounds` (tests/v2_mrtr.rs:2237) and `sep_2322_request_state_incomplete_then_complete` (:745) are both inside the green 34/34. NOT a regression against SC-2/SC-5."
deferred:
  - truth: "The 31 reviewed-unbounded whole-body reads on the auth surface (src/client/oauth.rs, src/client/auth.rs and two auth providers) are outside the HTTP-09 tripwire's scope fence and remain unbounded"
    addressed_in: "Phase 116"
    evidence: "ROADMAP Phase 116 'Auth Hardening SEPs — ... all source changes to the hand-rolled OAuth stack'; recorded as D-113-V in deferred-items.md:1280 with owner Phase 116. HTTP-09 scopes to 'the v2 transport path' (src/shared/, src/client/subscriptions.rs, src/server/streamable_http_server.rs) and these files are outside it by the requirement's own text, not by a fence drawn to make it pass."
  - truth: "`make quality-gate`'s fuzz stage does not actually fuzz — every `cargo fuzz run` fails on the stable toolchain and the failure is swallowed"
    addressed_in: "Recorded deferral (D-113-G), not owned by Phase 113"
    evidence: "Independently reproduced this round: Makefile:239 is `timeout 30s $(CARGO) fuzz run $$target || echo ...`, and the gate log shows every target failing with 'the option `Z` is only accepted on the nightly compiler' while the gate still exits 0. Pre-existing Makefile property recorded at deferred-items.md:351."
human_verification: []
---

# Phase 113: Stateless HTTP + Multi-Round-Trip Elicitation Verification Report

**Phase Goal:** v2 HTTP requests run with no `initialize` handshake and no `Mcp-Session-Id`, era-gated onto pmcp's existing `stateless()` branch (not a transport fork); multi-round-trip elicitation works end-to-end; and the pmcp `Client` is the v2-speaking counterpart, folding the Phase-106 host handlers into the v2 flow. v1 session behavior is untouched.
**Verified:** 2026-07-28T00:44:06Z at commit `c9944a65` (branch `fix/mcp-publisher-oidc-audience`)
**Status:** passed
**Re-verification:** Yes — round 2, after gap-closure plans 113-21…113-32 and the inserted Phase 113.1 (113.1-01…06). Round 1's single FAILED criterion (SC-3 / HTTP-04, over CR-01/CR-02/CR-03) is now closed on the merits.

## Method note — what this round did NOT do

Round 1's discipline was to reproduce every defect by reading current source rather than believing
a review. This round held to the same standard **in both directions**: no closure was accepted
because a SUMMARY asserted it.

Specifically, for the two algorithmic-complexity claims (CR-02 and D-113-R) the phase records a
pre-fix RED at 6.81 s / 15.06x and a committed 63.6 ms / 4.39x. Those are SUMMARY numbers. Rather
than accept them, I extracted the committed algorithms verbatim from the current tree, re-implemented
the pre-fix shapes described in their own rustdoc, and measured both myself in a standalone
`opt-level = 0` harness — then separately ran the committed in-tree guards and captured their own
`eprintln` measurements. Both the falsifiability of the guards and the linearity of the committed
code are therefore first-hand findings, not restatements.

## Publication-block context (not a codebase gap — carried forward, updated)

Eleven requirements (HTTP-01…HTTP-08, CLNT-01/02/05) remain `[~]` in `.planning/REQUIREMENTS.md`
under a recorded `hold` decision made by Guy Ernest on 2026-07-27 (`113-SPEC-RECHECK.md`
§ Third Outcome Policy). **This verification did not flip any checkbox and does not recommend one.**
That obligation has its own binding re-verification date (on or after 2026-07-28) and is outside
this phase's control.

What changed since round 1: **HTTP-09 is now `[x]`**. Round 1 could not see this requirement because
it did not exist in its current form — HTTP-04 was split on 2026-07-26 into HTTP-04/06/07/08/09 and
CLNT-05, promoting the previously *derived* "memory-bounded long-lived stream" criterion into an
explicit requirement with an enumerable, mechanical closure condition. I checked whether that split
was a criterion-weakening move and it was not:

- ROADMAP Success Criterion 3's text is **byte-identical** across `29873ce4` (2026-07-25, pre-split),
  `c3678eb2` (2026-07-26, the split commit) and `c9944a65` (HEAD). Verified by direct `git show`
  comparison. The words "collision-safe" and "memory-bounded" have **never** appeared in ROADMAP.md —
  they were round 1's own (correct) derivation from the then-bundled HTTP-04 requirement text.
- HTTP-09's requirement sentence is byte-unchanged at the `[ ]` → `[x]` flip (`1df85229`); the only
  deletion is the trailing status annotation `— *NOT met; see below*`. The obligation was made
  harder to satisfy (a mechanical tripwire is now required, "the fixes alone do not satisfy it"),
  not easier.

This round therefore holds SC-3 to the ROADMAP contract **plus** the memory-bounded obligation as
now formalized in HTTP-09.

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A v2 HTTP request completes with no `initialize` handshake and no `Mcp-Session-Id`, era-gated onto the existing `stateless()` branch, while v1 session behavior is unchanged (HTTP-01) | ✓ VERIFIED (no regression) | Regression-checked, not re-derived. 113.1-01/05 refactored the two handlers that own the v2 header gate (`handle_post_fast_path`, `handle_post_with_middleware`) for PMAT complexity — behaviour-preserving extraction into `resolve_v2_gate` / `dispatch_message_fast` / per-path preamble + legacy guard. `tests/v2_stateless_http.rs` 23/23 green; `make quality-gate` green end-to-end. |
| 2 | A handler returns `input_required` with `inputRequests` and an opaque `requestState` that is integrity-protected, principal-bound, and TTL'd; a client retry carrying `inputResponses` + the echoed `requestState` resumes the operation correctly (HTTP-02, HTTP-03) | ✓ VERIFIED (strengthened, no regression) | 113-26 and 113-27 changed identity and answer-typing semantics with a wire-visible blast radius; both examined and found to be security narrowings that reject only malformed/ambiguous input. The AAD aliasing marker is gone from `src/types/mrtr.rs`; `MAX_CANONICAL_DEPTH` = 64 now REFUSES (typed -32602) at both verify and mint. Sealed `Continuation.kinds: Option<InputRequestKinds>` (request_state.rs:559) enforces kind-directed decode strictly after the AEAD tag check. `client_server_mrtr_three_rounds` and `sep_2322_request_state_incomplete_then_complete` both green inside `v2_mrtr` 34/34; `v2_mrtr_ingress` 12/12. |
| 3 | On the v2 path `resources/subscribe`/`unsubscribe` and the HTTP GET stream endpoint are **removed**, notifications arrive over an opt-in `subscriptions/listen` long-lived stream (ack-first, `subscriptionId`-tagged), and the client half ships as `Client::subscriptions_listen` with `retired_on_v2` on the retired methods (HTTP-04) — **plus** the memory-bounded obligation now carried by HTTP-09 | ✓ **VERIFIED** | **The three round-1 BLOCKERs are each closed, re-measured from current source by symbol.** CR-01: the listen client's rejection branch routes through `collect_capped_body`; `rejection_error` re-signatured to take already-collected bytes. CR-02: `take_utf8_prefix` is a single-pass cursor with one buffer mutation per call — independently measured 21.5 ms vs 8.89 s against a re-implemented pre-fix shape at 1 MiB. CR-03: `HttpTransport::send_request` reads through `collect_body_within_cap`. Plus two defects round 1 did not know about: D-113-R (`drain_complete_lines`' per-CALL quadratic — independently measured 29.2 ms vs 4.26 s) and D-113-Q (`sse_optimized.rs`). `WHOLE_BODY_ALLOWLIST` is EMPTY. Tripwire 13/13, `v2_subscriptions` 19/19, `v2_subscriptions_client` 8/8. |
| 4 | SSE resumability (`Last-Event-ID`) is not offered on the v2 path, and a regression test proves response ids are always derived from the live request (HTTP-05) | ✓ VERIFIED (no regression) | Untouched by either gap-closure round. Covered by `tests/v2_stateless_http.rs` 23/23 and the green full gate. |
| 5 | The pmcp `Client` speaks v2 (per-request `_meta`, `server/discover`, required headers, no `initialize`) and fulfills MRTR `input_required` by producing `inputResponses`, folding the Phase-106 host handlers into the v2 flow (CLNT-01, CLNT-02) | ✓ VERIFIED (no regression) | `src/client/mod.rs` was touched by 113-27 (kind-directed answer production). The `SubscriptionStream`'s shared `SseParser` dependency — which round 1 flagged as inheriting truth 3's defect — is now clean, since truth 3 is closed. `v2_client`, `v2_mrtr`, `v2_subscriptions_client` all green inside the full gate. |

**Score:** 5/5 truths verified

### Required Artifacts (delta from round 1; baseline unchanged)

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `src/shared/sse_parser.rs` (`take_utf8_prefix`, line 175) | O(n) single-pass UTF-8 prefix decoder | ✓ VERIFIED — **round-1 DEFECT closed** | `consumed` cursor; `from_utf8` re-validates only `&buffer[consumed..]` and fails at the first bad byte, so per-iteration cost is the run just consumed; exactly one `clear()` or one `drain()` per call. Measured linear first-hand. |
| `src/shared/sse_parser.rs` (`drain_complete_lines`, line 482) | Per-call scan window, not a restart at 0 | ✓ VERIFIED — **new defect (D-113-R) closed** | `search_from = scan_start` (buffer length before append); single `drain(..consumed)` at line 567; the O(retained)-per-call `debug_assert` removed. Soundness precondition (retained buffer is newline-free) documented and pinned by a named test. |
| `src/shared/sse_parser.rs` (`SseParser::feed`, line 382) | Unconditionally-bounded SSE decoder | ✓ VERIFIED (no regression) | Pre-check at line 408 covers `buffered_bytes() + data.len()` with no `contains('\n')` escape; independently-sufficient post-drain check at 429. Unchanged from round 1. |
| `src/client/subscriptions.rs` (`open_event_stream` rejection branch, 135-150) | Capped body read on the listen client's rejection path | ✓ VERIFIED — **CR-01 closed** | `self.collect_capped_body(response).await`; `rejection_error` (171) takes `&[u8]`. `truncate` (333) bounds its own scan at MAX_ECHOED_FRAME+1 char_indices. |
| `src/shared/http.rs` (`HttpTransport::send_request`, 416; read at 457) | Capped response body read | ✓ VERIFIED — **CR-03 closed** | `Self::collect_body_within_cap(response, self.max_collected_body_bytes)`. Helper at 266: Content-Length pre-check + `Limited`. Cap field at 79, default at 125 (16 MiB), builder at 252. |
| `src/shared/sse_optimized.rs` (`collect_sse_text_within_cap`, 280) | Bounded reqwest SSE body read | ✓ VERIFIED — **D-113-Q closed** | Running-total refusal before each append, underflow-safe; transport additionally `#[deprecated]`. |
| `src/shared/streamable_http.rs` (`collect_body_within_cap` 528 / `collect_capped_body` 566) | The shared capped-read seam | ✓ VERIFIED | Content-Length pre-check + `Limited`; `collect_capped_body` exposes it at the transport's configured cap so `with_max_collected_body_bytes` moves the listen client's cap too. 6 tests in `mod collected_body_cap`. |
| `tests/v2_bounded_reads_tripwire.rs` | A mechanical, non-vacuous HTTP-09 check | ✓ VERIFIED | 13/13 green. Scope = `src/shared/**` + the two named files — **exactly** HTTP-09's stated scope. Carries three anti-vacuity guards (scope-discovery assertion on 5 required files, "exactly 2 capped reads found and both `Limited`-wrapped", and a substantive-justification rule). `WHOLE_BODY_ALLOWLIST` = `&[]` at line 591 with a ratchet-floor assertion. |
| `src/types/mrtr.rs` (`write_canonical` / `MAX_CANONICAL_DEPTH` 853) | Non-aliasing canonicalizer | ✓ VERIFIED | `__mrtr_depth_capped__` marker absent from the tree; `CanonicalDepthExceeded` (1268) is `pub(crate)`, so no semver event. |
| `src/server/request_state.rs` (`Continuation.kinds` 559) | Sealed record of requested kinds | ✓ VERIFIED | `Option<InputRequestKinds>` — ABSENT vs EMPTY deliberately distinguished; a pre-kinds continuation still deserializes as `None` (pinned by a named test). |

### Key Link Verification (delta)

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `EventStreamTransport::open_event_stream` (rejection branch) | body cap | `self.collect_capped_body(...)` → `collect_body_within_cap` → `Limited` | ✓ **WIRED** (was NOT WIRED) | client/subscriptions.rs:141 → streamable_http.rs:566 → :528. |
| `HttpTransport::send_request` | body cap | `Self::collect_body_within_cap(response, self.max_collected_body_bytes)` | ✓ **WIRED** (was NOT WIRED) | http.rs:457 → :266. |
| `OptimizedSseTransport::connect_sse` | body cap | `collect_sse_text_within_cap(..., DEFAULT_HTTP_SSE_BUFFERED_BYTES)` | ✓ **WIRED** (was allowlisted NOT BOUNDED) | sse_optimized.rs:394-396 → :280. |
| `read_next_frame` (listen stream) | `take_utf8_prefix` | called per hyper frame, before any byte bound | ✓ **WIRED, no longer defective** | client/subscriptions.rs:257. The callee is now O(n); the previously-quadratic scan upstream of every bound is gone. |
| `SseParser::feed` | `drain_complete_lines` | shared tokenizer behind `feed` and `feed_complete_body` | ✓ **WIRED, no longer defective** | sse_parser.rs:419 → :482, now scan-window-cursored. |
| `ListenRejection::code` | `v2_status_for_code` | `RATE_LIMITED` falls to `_ => StatusCode::OK` | ✓ WIRED (no regression) | subscriptions.rs:439; streamable_http_server.rs:690-702, `RATE_LIMITED` still absent from the 400 arm. |
| `Client::subscriptions_listen` | fresh subscription id | `Uuid::new_v4()` per call | ✓ WIRED (re-confirmed this round) | client/mod.rs:504,797; tripwire tests/v2_subscriptions_client.rs:292-335. |

### Behavioral Spot-Checks (this round's independent measurements)

| Behavior | Method | Result | Status |
|---|---|---|---|
| `make quality-gate` (the project's CI-equivalent gate) | Run once, full, foreground-equivalent | **exit 0**. fmt-check ✓, lint ✓ (`--features full`, pedantic+nursery), build, test-all (unit + doc + property + examples + integration), pmcp-package-gate, audit (807 deps scanned, 4 allowed warnings), unused-deps, check-todos, check-unwraps, validate-always, purity-check, comply — all traversed without abort | ✓ PASS |
| rtk exit-code fidelity (the gate ran under the `rtk` proxy, which filters output) | `rtk make definitely-not-a-target` → 2; `rtk make -n fmt-check` → 0 | Exit codes propagate faithfully, so `EXIT_CODE=0` is authoritative even though ~6230 log lines were filtered | ✓ PASS |
| `take_utf8_prefix` committed vs pre-fix shape @ 1 MiB of 0xFF | Standalone `opt-level=0` harness; committed algorithm copied verbatim from HEAD, pre-fix shape re-implemented from its own rustdoc | **committed 21.49 ms, pre-fix shape 8.89 s** (414x). Output byte-identical between shapes and equal to `String::from_utf8_lossy`; buffer left empty; one U+FFFD per invalid byte | ✓ CONFIRMS CR-02 closed, and the guard falsifiable |
| `take_utf8_prefix` growth over a 4x input step | Same harness, min of N runs | **committed 4.17x, pre-fix shape 17.47x** (ceiling 8.0x sits between them) | ✓ CONFIRMS the ratio guard separates the shapes |
| `drain_complete_lines` scan window, 512 KiB fed 1 byte per call | Same harness, both scan windows, line-count parity asserted | **committed 29.22 ms, pre-fix shape 4.26 s** (146x); identical line counts across 1-char chunking | ✓ CONFIRMS D-113-R closed |
| `drain_complete_lines` growth over a 4x step | Same harness | **committed 3.98x, pre-fix shape 15.26x** | ✓ CONFIRMS the ratio guard separates the shapes |
| The committed in-tree guards' own measurements | `cargo test --features full --lib shared::sse_parser -- --nocapture` | `take_utf8_prefix: 1048576 invalid bytes in 37.91 ms (ceiling 1s)`; `take_utf8_prefix growth ... ratio 4.16x (ceiling 8.0x)`; `SseParser::feed: 524288 single-byte chunks in 53.23 ms (ceiling 1s)`; `SseParser::feed growth ... ratio 4.02x (ceiling 8.0x)`. **34 passed; 0 failed** | ✓ PASS — and the recorded 6.81 s / 15.06x and 63.6 ms / 4.39x are corroborated to within machine variance |
| Independent replication of the tripwire's whole-body scan | Own whitespace-stripping, comment-stripping Python scan of `src/shared/**` + the two named files for all 8 needles | `src/shared/http.rs .collect().await x1`, `src/shared/streamable_http.rs x2`, `src/server/streamable_http_server.rs body::to_bytes( x1`. The streamable_http.rs surplus resolves to line 1820, inside `#[cfg(all(test, not(target_arch="wasm32"), feature="streamable-http"))]` at :1538 — correctly excluded by the tripwire's `cfg_test_spans`. `body::to_bytes(body, max_bytes)` is bounded by construction. Net: exactly the 2 shipped `Limited`-wrapped reads the anti-vacuity test asserts | ✓ CONFIRMS the tripwire is sound and non-vacuous |
| The allowlist's claim that `SimdSseParser` has no production caller | `grep -rn SimdSseParser src/` | Only a `pub use` re-export at `src/shared/mod.rs:122`; the sole construction (`simd_parsing.rs:550`) is inside `#[cfg(test)]` (module opens at :507) | ✓ CONFIRMS the justification is accurate |
| SC-3 / HTTP-09 targeted suites | `cargo test --features full --test …` | v2_bounded_reads_tripwire **13/13**, v2_mrtr **34/34**, v2_mrtr_ingress **12/12**, v2_stateless_http **23/23**, v2_subscriptions **19/19**, v2_subscriptions_client **8/8**. 0 failed, 0 ignored | ✓ PASS |
| Debt-marker scan across all shipped `.rs` | `grep -rn -E "TBD\|FIXME\|XXX\|TODO\|HACK" --include=*.rs src tests fuzz/fuzz_targets` | One hit: `tests/workflow_prompt_e2e_test.rs:35`, a domain string literal (`"Task state: TODO, DOING, DONE…"`), not a debt marker | ✓ PASS |
| Placeholder / stub scan across phase-touched files | `grep -inE "placeholder\|coming soon\|not yet implemented\|will be here"` | One hit: `src/types/mrtr.rs:1296`, prose in a rustdoc explaining why the code REFUSES **instead of** substituting a placeholder | ✓ PASS |
| Disabled-test scan on requirement-linked files | `grep -rn "#\[ignore\]" tests/v2_*.rs` + the six touched `src/` modules | Zero matches | ✓ PASS |
| Criterion-weakening check | `git show` of ROADMAP SC-3 at `29873ce4` / `c3678eb2` / `c9944a65`, and of the HTTP-09 flip `1df85229` | SC-3 byte-identical across all three; HTTP-09's requirement sentence byte-unchanged at the flip | ✓ PASS — no criterion was narrowed to make the phase pass |

### Probe Execution

Step 7c: **SKIPPED.** No `scripts/*/tests/probe-*.sh` conventional probes exist in this repo, and no
PLAN or SUMMARY in Phase 113 or 113.1 declares one. The project's runnable gate is `make quality-gate`,
which was executed once in full (exit 0) and is reported above under Behavioral Spot-Checks rather
than duplicated here.

The libFuzzer campaigns (113-19 Campaign 2, 113.1-02 Campaign 3 at `113-FUZZ-EVIDENCE.md:534`,
commit `647d2f4b`) were **not independently re-run**: they require a nightly toolchain, and the
gate's own fuzz stage is a no-op on stable (see the `deferred` entry for D-113-G — independently
reproduced this round). Their documentary evidence is specific and checkable (commit SHA, seeds,
counters, shown-crashing negative controls) and is accepted as documentation, not as executed proof.

### Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|---|---|---|---|---|
| HTTP-01 | 113-04 | v2 stateless era gate, no session/handshake | SATISFIED\* | `sessions_active` gate + `tests/v2_stateless_http.rs` 23/23; green gate |
| HTTP-02 | 113-02/03/06/09/11/25/26/27 | `input_required` + AEAD `requestState` | SATISFIED\* | `src/server/request_state.rs`, `tests/v2_mrtr.rs` 34/34; AAD aliasing closed (113-26) |
| HTTP-03 | 113-02/06/09/11/24/26/27 | Client retry resumes via echoed `requestState` | SATISFIED\* | `client_server_mrtr_three_rounds`, `sep_2322_request_state_incomplete_then_complete`; kind-directed typing (113-27) |
| HTTP-04 | 113-10/13/14…20/23/31 | `subscriptions/listen` stream, retirement of legacy RPCs | SATISFIED\* — **UNBLOCKED** | Round 1 marked this BLOCKED for CR-01/02/03. All three closed and re-measured this round; `v2_subscriptions` 19/19, `v2_subscriptions_client` 8/8 |
| HTTP-05 | 113-08 | SSE resumability off on v2, id-replay regression | SATISFIED\* | `tests/v2_stateless_http.rs` id-invariant tests |
| HTTP-06 | 113-10 | GET stream endpoint not served on v2 | SATISFIED\* | Part of the green `v2_stateless_http` / `v2_subscriptions` surface |
| HTTP-07 | 113-10/13 | ack-first frame + `subscriptionId` tagging | SATISFIED\* | `v2_subscriptions` / `v2_subscriptions_client` green. ⚠ Highest drift risk in the phase — both obligations are post-RC spec additions, PR #3006 still open (REQUIREMENTS.md warning block) |
| HTTP-08 | 113-10/31/32 | Opt-in capability gating, advertise-implies-serve tripwire | SATISFIED\* | `v2_conformance_pin`, `v2_subscriptions` green. ⚠ Predicate lives in the conformance repo, not the schema; the pin gate now has a second arm (113-32) |
| **HTTP-09** | 113-21/22, 113.1-02/03 | Every peer-controlled read on the v2 transport path is memory-bounded, closure enumerable via a tripwire, and no scan over peer-chosen input worse than O(n) | ✓ **SATISFIED — `[x]`, on the merits** | Tripwire 13/13 with `WHOLE_BODY_ALLOWLIST` EMPTY; scope matches the requirement's stated scope exactly; anti-vacuity independently replicated; the O(n) clause proven by 5 named guards whose falsifiability I measured myself. Requirement text byte-unchanged at the flip. |
| CLNT-01 | 113-05/13 | Client speaks v2 | SATISFIED\* | `src/client/mod.rs` `with_protocol_version` etc.; `v2_client` green |
| CLNT-02 | 113-07/11/27 | Client fulfills MRTR via Phase-106 host handlers | SATISFIED\* | `fold_input_requests` → `self.host_registry`; kind-directed answers (113-27) |
| CLNT-05 | 113-13 | `subscriptions_listen` + typed `retired_on_v2` | SATISFIED\* | `v2_subscriptions_client` 8/8 |

\* The eleven `[~]` requirements carry "implemented — pending final schema" under the recorded
`hold` decision. **HTTP-09 is `[x]` and is the only Phase-113 requirement whose closure this round
independently re-derived end-to-end.** No checkbox was flipped by this verification.

**No orphaned requirements.** REQUIREMENTS.md's Phase-113 row set (HTTP-01…09, CLNT-01/02/05 — 12
IDs after the 2026-07-26 split) matches the ROADMAP phase header exactly. HTTP-09's REQUIREMENTS.md
coverage row now reads "Phase 113.1", correctly attributing where it closed.

### Decision Coverage

**SKIPPED (tooling).** `gsd-sdk query check.decision-coverage-verify` is not available in the
installed gsd-sdk (`Error: Unknown command: check`). This gate is non-blocking by design and does
not influence the status determination. As a partial substitute, the 18 recent SUMMARYs were scanned
for self-declared unresolved items; every `D-113-*` reference found resolves to an entry in
`deferred-items.md` with a named owner.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| — | — | No unreferenced `TBD`/`FIXME`/`XXX` debt markers anywhere in shipped `.rs` | ✓ none | — |
| — | — | No `#[ignore]`d tests on any requirement-linked file | ✓ none | — |
| `src/client/subscriptions.rs` | 135-150 | The CR-01 fix has **no site-specific over-cap regression test** — the cap is proven at the shared `collect_body_within_cap` helper (6 tests in `streamable_http.rs::collected_body_cap`) and the call site is protected structurally by the tripwire, but the end-to-end test round 1 explicitly asked for ("a test mirroring `collected_body_cap::…` for this fourth site") was not written | ⚠️ Warning | Low residual risk: the seam is shared and independently tested, and the site is inside `EXTRA_SCOPE` so a bare `.collect()` reappearing there fails the tripwire. Worth closing for symmetry with the other three capped sites. |
| `tests/v2_bounded_reads_tripwire.rs` | — | HTTP-09's third clause ("no scan over peer-chosen input is worse than O(n)") has **no mechanical enumeration**, unlike the whole-body-read and accumulation clauses. It is proven by 5 named guards over the two scans that were actually found defective (`take_utf8_prefix`, `SseParser::feed`) | ⚠️ Warning | A future quadratic scan introduced elsewhere in scope would not be caught by omission the way an unbounded read would. The guards that exist are strong and falsifiable — this is about coverage breadth, not guard quality. |
| `src/shared/simd_parsing.rs` | 285-422 | `SimdSseParser` is a **publicly exported** accumulator with no ceiling of its own. Honestly disclosed in the tripwire allowlist ("carries no ceiling of its own… nothing feeds it a peer byte stream today") | ℹ️ Info | Independently confirmed to have zero production callers (only construction is under `#[cfg(test)]`), so it is off the v2 transport path and outside HTTP-09's scope. A downstream consumer wiring it to a peer stream would inherit an unbounded accumulator. |
| `Makefile` | 239 | `make quality-gate`'s fuzz stage swallows every failure (`\|\| echo …`); on stable every `cargo fuzz run` fails on `-Zsanitizer` | ⚠️ Warning | Pre-existing and already recorded as **D-113-G**. Means "the gate is green" does **not** include fuzz evidence. Not a Phase-113 defect; reported so the gate's green is not over-read. |
| `.planning/*` (records) | — | Round 1's SC-3 wording ("collision-safe, memory-bounded") was a verifier derivation, never ROADMAP text. Round 2 confirmed SC-3 is byte-identical pre/post split and HTTP-09's text byte-unchanged at the flip | ℹ️ Info (verifier-added) | The split was a strengthening, not a narrowing. Recorded so a later reader does not mistake the wording change between the two verification reports for a moved goalpost. |

**Anti-patterns:** 6 findings — **0 blockers**, 4 warnings, 2 info.

### Test Quality Audit

| Test surface | Linked req | Active | Skipped | Circular | Assertion level | Verdict |
|---|---|---|---|---|---|---|
| `tests/v2_bounded_reads_tripwire.rs` | HTTP-09 | 13 | 0 | No | Structural + anti-vacuity | ✓ Sufficient — three independent anti-vacuity guards; scope discovery asserted at runtime |
| `src/shared/sse_parser.rs` unit module | HTTP-09 (O(n) clause) | 34 | 0 | No | Wall-clock budget + machine-independent ratio + output pinning | ✓ Sufficient — falsifiability measured first-hand this round; each guard also pins output/retention so "fast by doing less" cannot pass |
| `src/shared/streamable_http.rs::collected_body_cap` | HTTP-09 | 6 | 0 | No | Behavioral (over-cap refused before the parser; at-cap parses) | ✓ Sufficient |
| `tests/v2_subscriptions{,_client}.rs` | HTTP-04, HTTP-07, HTTP-08, CLNT-05 | 27 | 0 | No | Behavioral, live-socket | ✓ Sufficient |
| `tests/v2_mrtr{,_ingress}.rs` | HTTP-02, HTTP-03 | 46 | 0 | No | Value + behavioral (multi-round e2e) | ✓ Sufficient |
| `tests/v2_stateless_http.rs` | HTTP-01, HTTP-05, HTTP-06 | 23 | 0 | No | Behavioral | ✓ Sufficient |

**Disabled tests on requirements:** 0. **Circular patterns detected:** 0 — expected values in the
complexity guards come from an *independent* source (an explicitly re-implemented pre-fix shape run
as a negative control), which is the opposite of circular; this round reproduced that independence
rather than assuming it. **Insufficient assertions:** 0.

### Deferred Items

| # | Item | Addressed In | Evidence |
|---|---|---|---|
| 1 | 31 reviewed-unbounded whole-body reads on the auth surface (4 files) | Phase 116 | ROADMAP Phase 116 "Auth Hardening SEPs — all source changes to the hand-rolled OAuth stack"; recorded as D-113-V. Outside HTTP-09's scope by the requirement's own text, not by a fence drawn to pass. |
| 2 | `make quality-gate`'s fuzz stage never actually fuzzes on stable | D-113-G (recorded, unowned by Phase 113) | Makefile:239 swallows the failure; independently reproduced in this round's gate log. |

### Human Verification Required

**N/A — infrastructure/foundation phase.** Phase 113 delivers wire-protocol behaviour, HTTP
transport hardening and a client library surface. It has no UI, no end-user-visible CLI output and
no real-time UX to observe. All five success criteria are verifiable programmatically and were
verified that way. `human_verification: []`.

## Gaps Summary

**No gaps found. Phase goal achieved.**

Round 1 left one FAILED criterion — SC-3 / HTTP-04 — over three BLOCKER defects (CR-01, CR-02,
CR-03). All three are closed in the current tree, and I confirmed each by reading the current source
at the symbol rather than the line, since every line number had moved:

1. **CR-01** — `src/client/subscriptions.rs`. The bare `body.collect().await` on the
   `subscriptions/listen` client's non-stream rejection path is gone. `open_event_stream` now calls
   `self.collect_capped_body(response)`, and `rejection_error` was re-signatured to take
   already-collected `&[u8]` — so the size-sensitive half moved to the transport's capped collector
   and the function is *structurally* no longer a place an unbounded read can return to. The
   secondary character-vs-byte defect is addressed too: `truncate` bounds its own scan.
2. **CR-02** — `src/shared/sse_parser.rs`. `take_utf8_prefix` is a single-pass cursor scan with one
   buffer mutation per call. **I did not accept this from the SUMMARY.** I re-implemented the
   pre-fix shape from its own rustdoc and measured both at 1 MiB of `0xFF`: committed **21.5 ms**,
   pre-fix **8.89 s**, with byte-identical output and parity with `from_utf8_lossy`; over a 4x step,
   **4.17x** vs **17.47x**. The committed in-tree guards then measured 37.9 ms and 4.16x against a
   1 s / 8.0x ceiling. The guards are genuinely falsifiable — the quadratic shape blows the absolute
   budget by ~8.9x and the ratio ceiling by ~2.2x.
3. **CR-03** — `src/shared/http.rs`. `HttpTransport::send_request` reads through
   `collect_body_within_cap`, a real sibling of the StreamableHttpTransport helper with a
   Content-Length pre-check and a streaming `Limited` bound, backed by a private cap field, a named
   16 MiB default and an additive builder.

Two further defects that round 1 did not know about were also found and closed inside this window,
and I verified both the same way: **D-113-R** (`drain_complete_lines`' per-CALL quadratic — measured
**29.2 ms** vs **4.26 s**, and **3.98x** vs **15.26x**) and **D-113-Q** (`sse_optimized.rs`'s
unbounded `reqwest` read), the latter driving `WHOLE_BODY_ALLOWLIST` to **empty**.

The requirement these all serve, **HTTP-09**, is now `[x]` and I re-derived its closure
independently rather than reading it off the ratchet: I replicated the tripwire's whitespace- and
comment-stripping scan myself over its declared scope, resolved the one apparent surplus to a
`#[cfg(all(test, …))]` region the tripwire correctly excludes, and confirmed the allowlist's
"`SimdSseParser` has no production caller" justification by grep. I also checked the obvious
adversarial hypothesis — that the 2026-07-26 HTTP-04 split narrowed the criterion so the phase could
pass — and it does not hold: ROADMAP SC-3 is byte-identical before and after the split, and HTTP-09's
requirement sentence is byte-unchanged at the `[ ]` → `[x]` flip.

The four criteria round 1 verified were spot-checked for regression rather than re-derived, with
particular attention to 113-26 and 113-27, whose stated blast radius is "rejects requests that
previously succeeded". Both are security narrowings that reject only malformed or ambiguous input;
the MRTR end-to-end proofs (`client_server_mrtr_three_rounds`, `sep_2322_request_state_incomplete_then_complete`)
still pass, and every mechanism round 1 named is still intact at HEAD.

`make quality-gate` — the project's CI-equivalent gate — was run once, in full: **exit 0**.

### Escalation — items outside this verification's authority

Reported, not acted on, per the ground rules for this run:

1. **The `[~]` publication hold (11 requirements).** HTTP-01…08 and CLNT-01/02/05 remain `[~]`
   under Guy Ernest's recorded 2026-07-27 `hold` decision. Its re-verification obligation (re-run
   the `113-SPEC-RECHECK.md` checkpoint on or after 2026-07-28 — i.e. now) is **binding and still
   undischarged**, and a schema mismatch is a phase-reopening event. **No checkbox was flipped by
   this verification.** *Decision needed from the developer, not from a verifier.*
2. **Merge blockers not owned by Phase 113.** `make doc-check`'s 26 pre-existing rustdoc errors
   (D-113-W) and the Purity Gate's tooling drift stand in front of the org-required `gate`, which
   also needs a human push (Phase 113.1 D-20 / SC-1b, explicitly recorded as NOT discharged).
   Excluded from scope by this run's ground rules and **not** counted as Phase-113 gaps.
3. **Two warnings worth a follow-up, neither blocking:** the missing site-specific over-cap test on
   the CR-01 path, and the absence of a mechanical enumeration for HTTP-09's O(n) clause (as
   distinct from its well-guarded whole-body-read and accumulation clauses).

## Verification Metadata

**Verification approach:** Goal-backward, FORCE adversarial stance; re-verification round 2
**Must-haves source:** ROADMAP.md Success Criteria (5), plus REQUIREMENTS.md HTTP-01…09 / CLNT-01/02/05 (12)
**Verified at commit:** `c9944a65` (branch `fix/mcp-publisher-oidc-audience`)
**Automated checks:** 20 behavioral spot-checks, all passed; 115 targeted tests green across 6 suites; `make quality-gate` exit 0
**Independent re-implementations built by the verifier:** 2 (pre-fix `take_utf8_prefix`, pre-fix `drain_complete_lines` scan window) — used as negative controls to test guard falsifiability rather than trust the recorded measurements
**Human checks required:** 0 (infrastructure/foundation phase)
**Escalations raised:** 3 (publication hold; non-owned merge blockers; 2 non-blocking warnings)

---
*Verified: 2026-07-28T00:44:06Z*
*Verifier: Claude (gsd-verifier)*
