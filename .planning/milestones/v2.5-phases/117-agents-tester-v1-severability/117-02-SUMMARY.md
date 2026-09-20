---
phase: 117-agents-tester-v1-severability
plan: 02
subsystem: testing
tags: [byte-identity, golden-fixtures, streamable-http, sse, session-lifecycle, last-event-id, severability]

# Dependency graph
requires:
  - phase: 113-v2-transport-http
    provides: "the session / resumability era gates (`sessions_active`, `resumability_active`) whose v1 side these goldens freeze, and `tests/common/v2.rs`, the live-socket harness"
  - phase: 115-json-schema-2020-12-structured-output-caching-hints
    provides: "`tests/v1_lists_golden.rs` — the byte-identity discipline (width-preserving normalizer, `V1 WIRE BREAK` framing) restated here"
provides:
  - "`tests/v1_byte_identity_after_cut.rs` — 9 pre-cut golden fixtures pinning the v1 initialize response, `Mcp-Session-Id` emission, a session-carrying follow-up POST, `Last-Event-ID` replay, and the v1 GET/DELETE verb answers"
  - "A BOUNDED, frame-counting SSE reader local to the test file, safe against the long-lived v1 `text/event-stream` that `common::v2::get` cannot read"
  - "Header identity asserted as an explicit name-to-value block, SEPARATE from every body-byte claim"
  - "A capture anchor commit (624e89b7) that any later 'the v1 wire is unchanged' claim is made against"
affects: [117-09, 117-12, 117-13, 117-14, v1-sunset, streamable-http-severance]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Field-line-anchored width-preserving normalizer (`key: VALUE`), serving SSE frame fields and rendered header blocks with one instrument"
    - "Bounded SSE reader: frame count is the SUCCESS bound, timeout is the FAILURE bound — never a success path"
    - "Header identity and body identity asserted as two independent claims, proven independent by a source-mutation negative control"

key-files:
  created:
    - tests/v1_byte_identity_after_cut.rs
  modified: []

key-decisions:
  - "Captured against the UNMODIFIED tree at 624e89b7 — goldens taken after a refactor prove only that the refactor is self-consistent"
  - "`spawn_default_config`, never the build-time stateless spawn: a stateless server has no `session_id_generator`, so a stateless fixture would be vacuously green"
  - "The stateless spawn helper's NAME is deliberately absent from the file so `grep` for it is a working detector rather than a hit on a comment"
  - "The replay cursor is a well-formed but UNKNOWN event id, because `InMemoryEventStore` resolves an unknown cursor to position 0 and replays the whole stream — that reach is today's v1 answer and is what T-117-05 needs frozen"
  - "The bounded SSE reader is LOCAL to the test file; `tests/common/v2.rs` serves other suites and was not modified"
  - "`Structural` cross-checks are for the readable message only; the RAW-string comparison carries key order, whitespace and SSE framing"

patterns-established:
  - "Pattern: capture a golden by running it against a deliberately-wrong literal and pasting the bytes out of the failure message — the capture run IS the first negative control"
  - "Pattern: every golden file states, in its failure message, that re-recording the golden is the WRONG remedy"
  - "Pattern: anti-vacuity guard per fixture family (`the_replay_fixture_actually_replayed_something`, `the_substitution_is_width_preserving_and_shape_checked`)"

requirements-completed: [SMPL-02]

# Metrics
duration: 42min
completed: 2026-08-07
---

# Phase 117 Plan 02: v1 Byte-Identity Goldens (Pre-Cut) Summary

**Nine pre-cut golden fixtures pinning the v1 session lifecycle and SSE resumability wire bytes — bodies AND headers — read through a bounded frame-counting SSE reader, anchored to commit 624e89b7.**

## Performance

- **Duration:** ~42 min
- **Started:** 2026-08-07T22:31:00Z (approx)
- **Completed:** 2026-08-07T23:13:00Z (approx)
- **Tasks:** 2
- **Files modified:** 1 created, 0 modified

## Accomplishments

- `tests/v1_byte_identity_after_cut.rs` (1078 lines) exists and is green against the unmodified tree: **9 tests, 0 failed, suite wall time 0.01 s** (well under the 60 s bound).
- The full v1 session lifecycle is frozen byte-for-byte BEFORE a single line of `src/server/streamable_http_server.rs` moves: initialize, session-header emission, a session-carrying follow-up POST, `Last-Event-ID` replay, GET SSE headers, DELETE teardown, and post-teardown session reuse.
- All three required negative controls were EXECUTED, produced the expected failure, and were REVERTED (output recorded verbatim below).
- `make quality-gate` exits 0. `tests/common/v2.rs` and `src/server/streamable_http_server.rs` are byte-identical to their pre-plan state (`git status --porcelain` clean for both).

## Capture Anchor

```
git rev-parse HEAD  ->  624e89b7cbed639b7d4e14b88a82d76fd99438da
```

Every literal in `tests/v1_byte_identity_after_cut.rs` was captured from a live loopback round trip against the tree at **624e89b7**, with no edit to `src/server/streamable_http_server.rs` in the working tree. This is the commit any later "the v1 wire is unchanged" claim is made against.

## Task Commits

1. **Task 1: Pin the v1 initialize response and `Mcp-Session-Id` emission** — `f342e613` (test)
2. **Task 2: Pin v1 `Last-Event-ID` replay and the v1 GET/DELETE verb answers** — `7da7d4e5` (test)

## Files Created/Modified

- `tests/v1_byte_identity_after_cut.rs` (created, 1078 lines) — the pre-cut v1 golden suite, its field-line width-preserving normalizer, and its bounded SSE reader.

## The Bounded SSE Reader

`common::v2::get` (`tests/common/v2.rs:835`) reads the response body to EOF. The v1 GET endpoint (`handle_get_sse`, `src/server/streamable_http_server.rs:4441-4503`) returns a **long-lived `text/event-stream`** that never reaches EOF, so driving an SSE fixture through it would block until a timeout and return nothing to compare. `get` is therefore **not imported at all** by this file.

**Signature:**

```rust
async fn read_bounded_sse(
    addr: SocketAddr,
    extra: &[(String, String)],
    want_frames: usize,
) -> SseCapture
```

`SseCapture { status: u16, headers: Vec<(String, String)>, raw: String, frames: usize }` — RAW bytes, not parsed structs, plus the status and headers so header identity can be asserted independently of the body.

**Bounds:**

| Constant | Value | Role |
|----------|-------|------|
| `REPLAY_FRAME_COUNT` (**N**) | `2` | SUCCESS bound — the reader returns as soon as N complete frames are parsed |
| `SSE_READ_TIMEOUT` | `Duration::from_secs(5)` | FAILURE bound — a hung stream FAILS the test, never a quiet success |
| `FRAME_TERMINATOR` | `"\n\n"` | how a complete frame is counted |

N is 2 because `store_response_event` writes one event per POST carrying a response session id, and `prime_replayable_events` issues exactly two such POSTs (`initialize` id 10, `tools/list` id 11).

## Fixtures Pinned — BODY, HEADERS, or Both

Plans 117-09 / 117-12 / 117-13 / 117-14 must keep every row below green.

| # | Test | Pins | What exactly |
|---|------|------|--------------|
| 1 | `v1_initialize_response_body_bytes_are_pinned` | **BODY** + status | The whole v1 `initialize` JSON-RPC frame. Pinned as **plain JSON, not SSE-framed** — `build_response` selects framing from the RAW INBOUND `Mcp-Session-Id`, which an `initialize` does not carry. No dynamics; compared verbatim. |
| 2 | `v1_initialize_emits_the_mcp_session_id_header` | **HEADERS** + status | The `mcp-session-id` header: NAME exact, VALUE by v4-UUID shape predicate, rendered as a `name: value` block and normalized width-preservingly. |
| 3 | `v1_session_carrying_follow_up_post_bytes_are_pinned` | **BODY** + status | A session-carrying `tools/list` POST. Pinned **SSE-framed**, including the `id` / `event` / `data` field order and the terminating blank line. Only the per-frame SSE event id is normalized. |
| 4 | `v1_get_sse_response_headers_are_pinned` | **HEADERS** + status | Status `200` plus an explicit name-to-value set: `content-type: text/event-stream`, `mcp-session-id: <session-id>`, `cache-control: no-cache, no-transform`, `connection: keep-alive` — i.e. `content-type` plus everything `attach_sse_response_headers` (`:4426-4439`) emits. |
| 5 | `v1_last_event_id_replay_frame_bytes_are_pinned` | **BODY** | The two replayed frames' RAW bytes, in `event_order`, each re-framed with a fresh SSE event id. Pins that an UNKNOWN `Last-Event-ID` replays the stream **from the beginning** (T-117-05). |
| 6 | `v1_delete_session_answer_is_pinned` | **BODY** + status | `200` and the bare `{"status":"ok"}` — not a JSON-RPC frame, and that is itself pinned. |
| 7 | `v1_delete_then_reusing_the_session_is_rejected_as_today` | **BODY** + status | `404` and `{"jsonrpc":"2.0","error":{"code":-32600,"message":"Unknown session ID"},"id":null}` — key order `jsonrpc`/`error`/`id` and the explicit `"id":null` included. |
| 8 | `the_replay_fixture_actually_replayed_something` | anti-vacuity | Frame count `> 0` and `== REPLAY_FRAME_COUNT`, with a `FAILURE MODE: … / WHAT TO DO: …` message. |
| 9 | `the_substitution_is_width_preserving_and_shape_checked` | anti-vacuity | The same-width pass preserves length and key-occurrence count; the canonical pass writes the bare token. |

**No v2 behavior is pinned in this file.** The v2 GET/DELETE `405` rejection is plan 117-13's verb split and already has in-file coverage; this file's subject is exclusively the v1 answers.

## Negative Controls (all EXECUTED, then REVERTED)

### 1 — One character mutated inside a pinned literal

Mutation: `INITIALIZE_BODY`'s `"version":"1.0.0"` → `"version":"1.0.1"`.

```
thread 'v1_initialize_response_body_bytes_are_pinned' panicked at tests/v1_byte_identity_after_cut.rs:332:5:
assertion `left == right` failed: v1 session-lifecycle wire bytes changed. This is a V1 WIRE BREAK, not a stale fixture — the Phase-117 severance of the v1 session / SSE machinery from the v2 path is the likely cause, so FIX THE CUT and make the change v2-only instead of re-recording the golden. Raw capture was: {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{"listChanged":false},"prompts":{"listChanged":false}},"serverInfo":{"name":"v1-byte-identity","version":"1.0.0"}}}
test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

Reverted; re-run green.

### 2 — `Last-Event-ID` header removed from the replay request

Mutation: the `header(LAST_EVENT_ID, UNKNOWN_LAST_EVENT_ID)` entry commented out of `replay_capture`.

```
thread 'v1_last_event_id_replay_frame_bytes_are_pinned' panicked at tests/v1_byte_identity_after_cut.rs:503:5:
FAILURE MODE: the v1 SSE stream produced 0 complete frames, not the 2 this fixture requires, within 5s. WHAT TO DO: a timeout is NOT a golden — fix the v1 replay path rather than relaxing the frame count, because a fixture that compares an empty capture passes over nothing. Capture so far: ""
test result: FAILED. 6 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.01s
```

This is the load-bearing proof that the replay fixture cannot pass over an empty stream: with no `Last-Event-ID`, `replay_sse_events_from_header` returns before it ever looks at the store, the stream stays silent, and the reader FAILS on its frame bound rather than asserting on `""`. Reverted; re-run green.

### 3 — One SSE hardening header's VALUE mutated in the server source

Mutation: `attach_sse_response_headers` (`src/server/streamable_http_server.rs:4434`) `"no-cache, no-transform"` → `"no-cache, no-transform, no-store"`.

```
thread 'v1_get_sse_response_headers_are_pinned' panicked at tests/v1_byte_identity_after_cut.rs:345:5:
assertion `left == right` failed: v1 session-lifecycle wire bytes changed. This is a V1 WIRE BREAK, not a stale fixture — the Phase-117 severance of the v1 session / SSE machinery from the v2 path is the likely cause, so FIX THE CUT and make the change v2-only instead of re-recording the golden. Raw capture was: content-type: text/event-stream
mcp-session-id: 18f1077d-488c-4c1c-ac8d-576ef178df17
cache-control: no-cache, no-transform, no-store
connection: keep-alive

  left: "content-type: text/event-stream\nmcp-session-id: <session-id>\ncache-control: no-cache, no-transform, no-store\nconnection: keep-alive\n"
 right: "content-type: text/event-stream\nmcp-session-id: <session-id>\ncache-control: no-cache, no-transform\nconnection: keep-alive\n"
test result: FAILED. 8 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

**Exactly one test failed — the HEADER fixture. The body fixture `v1_last_event_id_replay_frame_bytes_are_pinned` stayed green**, which is the point: header identity and body identity are genuinely independent claims (T-117-07). Reverted via `git checkout -- src/server/streamable_http_server.rs`; re-run green.

## Acceptance Criteria Evidence

| Criterion | Result |
|-----------|--------|
| `cargo test --test v1_byte_identity_after_cut --features "full"` | `9 passed; 0 failed`, 0.01 s suite / 3.4 s wall — under the 60 s bound |
| `grep -c 'V1 WIRE BREAK'` | 3 (≥ 1) |
| `grep -c 'spawn_default_config'` | 3 (≥ 1) |
| `grep -c 'spawn_stateless_config'` | **0** — the name is deliberately absent so the grep is a working detector |
| `grep -c 'teardown'` vs `spawn_` call sites | 7 `teardown` occurrences vs 7 `spawn_` occurrences; every one of the 7 spawn call sites is followed by `teardown` (D-113-T drop→abort→await) |
| width-preserving length assertion | present, message contains `width-preserving` (7 occurrences of the phrase) |
| `grep -c 'timeout'` | 6 (≥ 1) |
| `grep -c 'LAST_EVENT_ID'` | 3, resolving to `pmcp::shared::http_constants::LAST_EVENT_ID` (proven by the file compiling) |
| `the_replay_fixture_actually_replayed_something` with `FAILURE MODE` | present (2 `FAILURE MODE` occurrences) |
| `grep -c 'common::v2::get\|use common::v2::{[^}]*get'` | **0** — `get` is never imported |
| `git status --porcelain tests/common/v2.rs` | empty — shared harness unmodified |
| `grep -c 'TODO\|FIXME\|XXX'` | 0 |
| `min_lines: 250`, `contains: "V1 WIRE BREAK"` | 1078 lines, phrase present |
| `make quality-gate` | exit 0 |

## Decisions Made

1. **The JSON-key substitution form from `v1_lists_golden.rs` was deliberately NOT restated.** No capture in this file carries a dynamic JSON string VALUE — the session id travels in a header, the SSE event id in a frame field — so a second substitution path would be dead weight no fixture could prove correct. Substitution is field-line anchored (`key: VALUE`), which serves SSE frame fields and rendered header blocks with one instrument.
2. **Headers are rendered into the same field-line shape as SSE frames** so the single width-preserving normalizer covers both the header claim and the body claim, with the length + occurrence-count invariants applying to both.
3. **The `Last-Event-ID` cursor is a well-formed-but-unknown v4 UUID** (`00000000-0000-4000-8000-000000000000`). `InMemoryEventStore::replay_events_after` resolves an unknown cursor to position 0 (`position(...).map_or(0, |pos| pos + 1)`), so it replays the whole stream. That is deterministic, and it is precisely the reach an attacker-supplied replay cursor has on v1 — the thing T-117-05 needs frozen across the cut.
4. **The GET header fixture also sends `Last-Event-ID`.** The v1 GET headers are the same either way, and reusing the primed replay setup means no fixture in this file reads a silent stream, so no fixture can pass by timing out.
5. **`spawn_resumable` names `session_id_generator` and `event_store` explicitly** (the two v1-only config fields this phase later gates) and reaches everything else through `..Default::default()`, so plan 117-13 has one obvious edit point and no field-by-field literal to maintain.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Dead `Form::JsonString` variant broke the `-D warnings` lint gate**

- **Found during:** Task 1
- **Issue:** The plan asked for `DynamicField` to be ported from `v1_lists_golden.rs`, whose normalizer is JSON-key anchored. This file has no dynamic JSON string value, so the ported `Form::JsonString` variant and its `substitute_json_string` arm were never constructed — `dead_code` under the project's `RUSTFLAGS=-D warnings` lint gate, which lints `--tests`.
- **Fix:** Removed the `Form` enum and the JSON-string substitution path entirely; substitution is field-line anchored only. A doc comment states why the JSON-key form is deliberately not restated, so the omission reads as a decision rather than an oversight.
- **Files modified:** `tests/v1_byte_identity_after_cut.rs`
- **Verification:** `make lint`-equivalent clippy invocation clean; `make quality-gate` exit 0.
- **Committed in:** `f342e613`

**2. [Rule 3 - Blocking] `clippy::map_unwrap_or` on `SseCapture::header`**

- **Found during:** Task 2
- **Issue:** `.map(...).unwrap_or_else(|| panic!(...))` is denied by `-D clippy::all` under the project lint set.
- **Fix:** Rewrote as a `let ... else { panic!(...) }`, preserving the "a header going ABSENT is a V1 WIRE BREAK too" message verbatim.
- **Files modified:** `tests/v1_byte_identity_after_cut.rs`
- **Verification:** clippy clean with the full project lint set; `make quality-gate` exit 0.
- **Committed in:** `7da7d4e5`

**3. [Rule 2 - Missing critical functionality] `spawn_stateless_config` removed from prose**

- **Found during:** Task 1
- **Issue:** The plan's acceptance criterion requires `grep -c 'spawn_stateless_config'` to be 0, but the plan's own action text asked the file to say "use `spawn_default_config`, NOT `spawn_stateless_config`" — which would leave the name in a comment and turn the grep into a false positive. `v1_lists_golden.rs` already establishes the opposite discipline for the supported-protocol-versions extender ("its name is deliberately absent from this whole file so a plain `grep` for it stays a working detector").
- **Fix:** Rephrased the doc comment to "never the harness's build-time stateless spawn helper — whose name is deliberately absent from this whole file so a plain `grep` for it is a working detector". The rationale is preserved in full; only the literal identifier is gone.
- **Files modified:** `tests/v1_byte_identity_after_cut.rs`
- **Verification:** `grep -c 'spawn_stateless_config' tests/v1_byte_identity_after_cut.rs` → 0.
- **Committed in:** `f342e613`

---

**Total deviations:** 3 auto-fixed (2 × Rule 3, 1 × Rule 2)
**Impact on plan:** All three were required to satisfy the project's own quality gate or the plan's own acceptance criteria. No scope creep — no production source was changed, no dependency added, and the shared harness was not touched.

## Issues Encountered

**`common::v2::get` cannot read the v1 GET.** Confirmed by reading `tests/common/v2.rs:835-850` before writing a single GET fixture, exactly as the plan instructed: `send()` calls `response.text().await`, which reads to EOF. Resolved with the local bounded reader described above; `tests/common/v2.rs` was not modified.

**`build_response` framing is inbound-header-dependent.** The v1 `initialize` reply is PLAIN JSON while the session-carrying follow-up is SSE-framed, because `build_response` (`src/server/streamable_http_server.rs:1922-1941`) selects framing from the RAW INBOUND `Mcp-Session-Id`, which an `initialize` request does not carry. Both framings are now pinned as-is; that asymmetry is part of the v1 answer.

**Pre-existing, out of scope:** `make quality-gate` prints `cargo fuzz` build errors (`the option 'Z' is only accepted on the nightly compiler`) for every fuzz target. This is an environment condition — the local default toolchain is stable — is unrelated to this plan's changes, and the gate still exits 0. Not fixed, per the SCOPE BOUNDARY rule.

## Threat Model Coverage

| Threat ID | Disposition | How this plan discharges it |
|-----------|-------------|------------------------------|
| T-117-04 | mitigate | Raw-byte goldens captured BEFORE the cut at anchor `624e89b7`; the failure message names re-recording as the wrong remedy; negative control 1 proves the assertion fires on a one-character change. |
| T-117-05 | mitigate | The v1 `Last-Event-ID` replay reach — including that an unknown cursor replays from position 0 — is pinned byte-for-byte, so the cut cannot change WHICH events an attacker-supplied cursor reaches. The v2 side (T-113-29/30) remains plan 117-13's. |
| T-117-06 | mitigate | Bounded LOCAL reader with `REPLAY_FRAME_COUNT = 2` as the success bound and `SSE_READ_TIMEOUT = 5 s` as the failure bound; every one of the 7 spawn sites is torn down with `teardown`'s drop→abort→await order. |
| T-117-07 | mitigate | Header identity asserted as a separate explicit name-to-value set; negative control 3 mutated a header in the SERVER source and failed ONLY the header fixture, proving the two claims independent. |
| T-117-SC | mitigate | Zero external packages added. One test file using existing dependencies (`reqwest`, `uuid`, `tokio`, `serde_json`) — no `cargo add`, no manifest change. |

## Next Phase Readiness

Ready. The regression net the whole D-03 cut is executed against now exists and is green on an unmodified tree.

**What plans 117-09 / 117-12 / 117-13 / 117-14 must keep green:** all 9 tests in the "Fixtures Pinned" table. Concretely:

- The `Mcp-Session-Id` header must keep the name `mcp-session-id` and a v4-UUID-shaped value on v1.
- `attach_sse_response_headers` must keep emitting `mcp-session-id`, `cache-control: no-cache, no-transform` and `connection: keep-alive`, and the GET must keep answering `text/event-stream` with status 200.
- `replay_sse_events_from_header` must keep replaying from position 0 on an unknown cursor, and must keep re-framing each replayed message with a fresh SSE event id while preserving its ORIGINAL JSON-RPC id.
- The v1 DELETE must keep answering `200 {"status":"ok"}` and a post-teardown reuse must keep answering `404` with `-32600 Unknown session ID` and `"id":null`.
- `build_response`'s inbound-header-dependent framing (plain JSON for `initialize`, SSE for a session-carrying POST) must not change on v1.

Turning any of these red means the cut leaked into v1. Fix the cut; do not re-record the golden.

---
*Phase: 117-agents-tester-v1-severability*
*Completed: 2026-08-07*

## Self-Check: PASSED

- `tests/v1_byte_identity_after_cut.rs` — FOUND
- `.planning/phases/117-agents-tester-v1-severability/117-02-SUMMARY.md` — FOUND
- Commit `f342e613` — FOUND
- Commit `7da7d4e5` — FOUND

## Requirement Status Correction (post-summary)

`requirements mark-complete SMPL-02` was run per the executor protocol (the plan's
frontmatter carries `requirements: [SMPL-02]`) and then **reverted**, because it
would have made a false claim.

SMPL-02 reads: *"The v2 code path carries no session/SSE-resumability baggage, and a
simplification pass removes code the v2 model obsoletes wherever v1 compatibility
permits."* Six plans in this phase carry it — 117-02, 117-06, 117-09, 117-12, 117-13
and 117-14 — and this plan delivers only the **pre-cut regression net**, not the cut.
The severance itself is plan 117-13's.

`.planning/REQUIREMENTS.md` is therefore left with `SMPL-02` **unchecked / Pending**.
The last SMPL-02 plan to land (117-14) should mark it complete.
