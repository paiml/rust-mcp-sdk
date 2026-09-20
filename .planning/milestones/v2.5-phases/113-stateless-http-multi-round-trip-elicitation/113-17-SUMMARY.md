---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 17
subsystem: transport
tags: [sse, sse-parser, http, streamable-http, subscriptions, dos-hardening, semver, base64]

# Dependency graph
requires:
  - phase: 113
    provides: "113-15's in-parser bound + latching `overflowed()` flag and the two free-function overflow observers (`listen_overflow`, `report_sse_line_overflow`)"
  - phase: 113
    provides: "113-13's `subscriptions/listen` client, `SseParser` CRLF byte-index guard, and `take_utf8_prefix`"
provides:
  - "`SseParser::feed` bounds `buffer + current_event.data + chunk` UNCONDITIONALLY — the `!data.contains('\\n')` escape is gone"
  - "`SseParser::buffered_bytes()` (pub(crate)) makes the bound observable and is the post-return invariant"
  - "`SseParser::feed_complete_body()` (pub(crate)) — the explicit whole-body bypass, used by exactly the two collected-body transport call sites"
  - "`DEFAULT_HTTP_SSE_BUFFERED_BYTES` (16 MiB) + a PRIVATE `HttpTransport` field + `HttpTransport::with_sse_buffered_bytes()` — a configurable ceiling with zero public-config-struct change"
  - "newline-carrying regression tests and doctests on every feeder (review IN-03 closed)"
affects: [113-20, 113-18, 113-19, 114, 117, 118]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Two independently-sufficient enforcement points (pre-check on `retained + chunk`, post-check on the residual) so a negative control must disable BOTH to reproduce the defect"
    - "Configurable limits as: named `pub const` default + PRIVATE transport field + additive inherent builder method — never a `pub` field on an externally-constructible config struct"
    - "A `pub(crate)` bypass with its precondition stated as a REQUIREMENT ON THE CALLER, naming the plan that will satisfy it, rather than as an established fact"

key-files:
  created: []
  modified:
    - src/shared/sse_parser.rs
    - src/shared/streamable_http.rs
    - src/shared/http.rs
    - src/client/subscriptions.rs

key-decisions:
  - "The bound is `retained state + THIS CHUNK`, not `one in-progress event` — accepted (T-113-86) because evaluating it post-split needs the unbounded parse first; the cost is a documentation obligation honoured in all three modules"
  - "`feed_complete_body` is `pub(crate)`, and its rustdoc states the byte-cap precondition as a caller obligation naming plan 113-20 — it never claims the boundary is already capped"
  - "The `connect_sse` SSE ceiling is CONFIGURABLE (private `HttpTransport` field + additive builder), not a fixed constant and not an `HttpConfig` field, which is a measured MAJOR semver break"
  - "`MAX_LISTEN_LINE_BYTES` deliberately NOT renamed: its name is referenced by the 113-19-fenced fuzz seam rustdoc and fuzz target; the doc states what it really bounds instead"

patterns-established:
  - "Bound tests must feed NEWLINE-CARRYING input — the newline-free flood is the artificial case and is why a green suite coexisted with GAP-A"
  - "A tripwire test that fires when a deliberate change lands is UPDATED in lockstep with the change, never deleted"

requirements-completed: [HTTP-04]

# Metrics
duration: 43min
completed: 2026-07-27
---

# Phase 113 Plan 17: Bound Everything SseParser Retains Summary

**`SseParser::max_buffer_size` is now an unconditional bound on `buffer + current_event.data + the incoming chunk`, closing GAP-A (a peer streaming ordinary newline-terminated `data:` lines could grow a pmcp client's heap without limit), with a `pub(crate)` `feed_complete_body` bypass for the two whole-body transport call sites and a configurable 16 MiB `connect_sse` ceiling that adds no field to any public config struct.**

## Performance

- **Duration:** 43 min
- **Started:** 2026-07-26T23:54:22Z
- **Completed:** 2026-07-27T00:37:34Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- **GAP-A closed.** Both of the verifier's independently reproduced measurements are inverted into passing assertions. `feed`'s pre-check drops the `!data.contains('\n')` term entirely and now covers `buffered_bytes() + data.len()`, so a peer's framing choices can no longer bypass the limit.
- **The invariant is now stated and observable.** `buffered_bytes() <= max_buffer_size` holds on return from every `feed`; the post-drain residual check was widened from `buffer.len()` to `buffered_bytes()` so both accumulators are covered, and the tightened proptest asserts the invariant directly instead of the loose `buffer.len() <= max(8, chunk.len())` form that let the defect through.
- **The whole-body path got an explicit, crate-private escape hatch.** `feed_complete_body` shares one `drain_complete_lines` implementation with `feed` (so the two entry points cannot drift in tokenization, and the 113-13 CRLF byte-index guard moved across unchanged), and its rustdoc phrases the byte-cap precondition as an obligation on the caller naming plan 113-20.
- **`connect_sse`'s ceiling became a deliberate, configurable choice.** `DEFAULT_HTTP_SSE_BUFFERED_BYTES` (16 MiB) + a private `HttpTransport` field + `with_sse_buffered_bytes()`. `cargo semver-checks` still reports 223 pass / no update required, with no `constructible_struct_adds_field`.
- **Both overflow messages now describe the real condition** — retained state plus the incoming chunk — and each feeder gained a newline-carrying regression test driving the PRODUCTION predicate (`drain_sse_payloads` + `listen_overflow`; `feed` + `report_sse_overflow`).

## Task Commits

1. **Task 1: Bound the accumulated event, not only the line — and give the whole-body call sites an explicit bypass** — `57d7a211` (fix)
2. **Task 2: Make both incremental feeders report the bound they actually enforce** — `25ae4a17` (fix)

## Files Created/Modified

- `src/shared/sse_parser.rs` — unconditional in-flight bound in `feed`; `buffered_bytes()`; `feed_complete_body()`; extracted `drain_complete_lines()`; four corrected doc blocks (field doc, `with_max_buffer_size`, `overflowed`, `DEFAULT_MAX_BUFFER_SIZE`/`SseConfig::max_buffer_size`); a newline-carrying doctest; three new tests plus the tightened proptest and the IN-02 fix.
- `src/shared/streamable_http.rs` — both whole-body call sites (POST-response `start_sse` at ~528, the `text/event-stream` POST branch at ~1150) migrated to `feed_complete_body`, each with a comment naming 113-20 as the owner of the byte cap.
- `src/shared/http.rs` — `DEFAULT_HTTP_SSE_BUFFERED_BYTES`; private `HttpTransport::sse_buffered_bytes` + `with_sse_buffered_bytes()`; `sse_reader_parser(usize)`; `report_sse_line_overflow` → `report_sse_overflow` with reworded doc and log; the tripwire updated to `connect_sse_uses_its_own_named_bound`; boundary, escape-hatch, base64-expansion and newline-flood tests.
- `src/client/subscriptions.rs` — `listen_overflow`'s message and doc rewritten to retained-state-plus-chunk; `MAX_LISTEN_LINE_BYTES`'s doc corrected; `read_next_frame`'s comment corrected; new `a_newline_carrying_flood_ends_the_stream_too`.

## Before/After Measurements (both reproductions)

| Reproduction | Before (this plan's negative control, both checks disabled) | After |
|---|---|---|
| `with_max_buffer_size(64)`, `feed("data: AAAAAAAA\n")` × 100 000 | **899 999 bytes** retained, `overflowed() == false` | `overflowed() == true`, `buffered_bytes() <= 64` |
| `with_max_buffer_size(64).feed("data: " + "B"×1 000 000 + "\n\n")` | **1 event of 1 000 000 bytes** returned, `overflowed() == false` | empty event vector, `overflowed() == true` |
| Same body through `feed_complete_body` | (did not exist) | exactly 1 event, `data.len() == 1 000 000`, `overflowed() == false` |

Both "before" figures were produced by this plan's own negative control, not quoted from the
verification document — they match the verifier's independently reproduced numbers exactly.

## Negative Control (review HIGH-2 corrected form)

Three runs of `cargo test --lib --features full -- sse_parser`, recorded in order:

| Run | State | Result |
|---|---|---|
| 1 | PRE-check forced to `false`; POST-check intact | **23 passed, 1 failed.** `a_newline_carrying_flood_cannot_grow_the_event_past_the_bound` stayed **GREEN** — the total post-check caught it. `an_oversized_complete_line_is_refused_not_emitted` FAILED: `got 1 event(s), the first of 1000000 bytes`. |
| 2 | PRE-check forced to `false` AND POST-check reverted to `self.buffer.len() > self.max_buffer_size` | **22 passed, 2 failed.** Flood test: `100,000 newline-terminated `data:` lines accumulated 899999 bytes without tripping the 64-byte bound`. Oversized-line test: `got 1 event(s), the first of 1000000 bytes`. |
| 3 | both restored | **24 passed, 0 failed.** |

Run 1 is the evidence the two checks are **not redundant**: each catches something the other
does not. The post-check alone stops the accumulating flood but cannot stop a single oversized
COMPLETE line (that line never becomes a residual — it drains straight into a dispatched
event), and the pre-check alone would have been the only guard on that path. Run 2 is the
required both-disabled reproduction.

## `connect_sse` Bound Decision and Rationale

**Decision:** `DEFAULT_HTTP_SSE_BUFFERED_BYTES = 16 * 1024 * 1024`, held in a **private**
`HttpTransport` field and overridable through the additive inherent builder
`HttpTransport::with_sse_buffered_bytes(usize)`. `sse_reader_parser` takes the configured
value; `SseParser::new()` no longer appears in `src/shared/http.rs` at all.

**Why configurable rather than fixed (review HIGH-4).** The earlier "media is unaffected"
claim is arithmetically false and is now WITHDRAWN in the source itself. MCP `image`/`audio`
content is unconstrained base64 and base64 expands ~4/3, so a 12 MiB binary is *already*
exactly 16 MiB once encoded — before the JSON envelope, the `data: ` prefix and the MIME type.
`base64_expansion_puts_a_12_to_16_binary_over_the_ceiling` pins that arithmetic
(`encoded.len() == raw_len.div_ceil(3) * 4`, `encoded.len() == ceiling`, and then
`frame.len() > ceiling` once framed), scaled down by 2^10 so the test costs kilobytes.

**Why not an `HttpConfig` field.** Measured, not assumed: a scratch `pub` field on `HttpConfig`
made `cargo semver-checks` report `constructible_struct_adds_field` → major bump required. Both
`HttpTransport`'s fields and `StreamableHttpTransport`'s are already entirely private, so a new
private field there is invisible to semver and an added inherent method is additive. The
milestone stays a 2.x MINOR.

**What breaks at the boundary (T-113-85, accepted with an escape hatch).** A single JSON-RPC
payload whose in-flight bytes exceed the configured ceiling is discarded and ENDS the reader
task, where previously it accumulated without limit and was delivered. Documented plainly on
the constant and the builder method.

**Tripwire updated, not deleted.** `connect_sse_keeps_the_shared_default_bound` existed
precisely to fail when this site was tightened. It became
`connect_sse_uses_its_own_named_bound`, which asserts that `HttpTransport::new` defaults its
ceiling from the named constant AND that `sse_reader_parser` built from that value carries it —
so any future change to either site still fails here.

## Verification Results

| # | Check | Result |
|---|---|---|
| 1 | `cargo test --lib --features full -- sse_parser` | **24 passed, 0 failed** (live baseline 21; floor 24) |
| 2 | `cargo test --doc --features full -- sse_parser` | **10 passed, 0 failed** (floor 8) |
| 3 | `cargo test --lib --features full -- client::subscriptions` | **31 passed, 0 failed** (floor 30) |
| 4 | `cargo test --lib --features full -- shared::http` | **25 passed, 0 failed** (floor 21) |
| 5 | `cargo test --test sse_parser_tests --features full` | 15 passed, 0 failed |
| 6 | `cargo test --test streamable_http_integration --features full` | 6 passed, 0 failed |
| 7 | `cargo test --test streamable_http_properties --features full` | 12 passed, 0 failed |
| 8 | `cargo test --test v2_subscriptions_client --features full` | 7 passed, 0 failed (floor 7) |
| 9 | `cargo test --test v2_subscriptions --features full` | 10 passed, 0 failed (floor 10) |
| 10 | `cargo test --test v2_stateless_http --features full` | 23 passed, 0 failed (floor 23) |
| 11 | `cargo test --test v2_mrtr --features full` | 27 passed, 0 failed (floor 27) |
| 12 | `cargo build -p pmcp-team-servers --all-features` | exit 0 |
| 13 | `cargo run --example s49_v2_subscriptions_client --features full` | exit 0 |
| 14 | `cargo run --example t08_simd_parsing_performance --features full` | exit 0 |
| 15 | `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks: 223 pass, 30 skip — no semver update required.** No `constructible_struct_adds_field`. |
| 15a | field additions to `HttpConfig` / `StreamableHttpTransportConfig`, `#[non_exhaustive]` retrofits | none (`git diff` shows no `pub` field added inside either struct) |
| 16 | `cargo build --lib --target wasm32-unknown-unknown` | exit 0 |
| 17 | `git diff --name-only -- Cargo.toml Cargo.lock` | empty |
| 18 | `make quality-gate` | **exit 0** (fmt OK, "No lint issues", 0 `test result: FAILED`). The known pre-existing D-113-G fuzz-stage behaviour reproduced: the nightly-only `-Zsanitizer` fuzz builds fail and the gate still exits 0 — not a regression from this plan. |
| 19 | contract-first: `ls ../provable-contracts/contracts/` | `No such file or directory` (exit 1) — no contract YAML exists for these surfaces in this environment, as the plan predicted. Recorded rather than skipped silently. |

**`cargo public-api -p pmcp --features full diff 2.17.0`** — zero REMOVED items attributable to
this plan. The two non-`zerocopy` `-` lines are a generic-parameter rename
(`ElicitRequestParams::deserialize<__D>` → `<D>`) from plan 113-02's hand-written serde impl,
re-added on the `+` side in the same diff. The ~2 895 remaining `-` lines are `zerocopy` blanket
impls of that transitive dependency's own private invariant traits — dependency-resolution drift
in the shared lockfile, not pmcp API. This plan's additions are exactly two:
`+pub const pmcp::shared::http::DEFAULT_HTTP_SSE_BUFFERED_BYTES: usize` and
`+pub fn pmcp::shared::http::HttpTransport::with_sse_buffered_bytes(self, usize) -> Self`.
Both new parser methods are `pub(crate)` and so invisible to both tools.

**Source acceptance greps (all pass):**

- `grep -n "contains('\n')" src/shared/sse_parser.rs` — two matches, neither in the
  bound-enforcement block: the permitted `debug_assert!` on `self.buffer` and one test doc
  comment describing the deleted escape.
- `grep -c "feed_complete_body" src/shared/streamable_http.rs` → **exactly 2** (the two call
  sites; the accompanying comments deliberately avoid re-naming the identifier so this tripwire
  stays exact).
- `grep -n "feed_complete_body" src/shared/http.rs src/client/subscriptions.rs` → **no match**
  (no incremental feeder reaches the bypass).
- `grep -c "buffered_bytes" src/shared/sse_parser.rs` → 12 (floor 6).
- `grep -niE "a single (subscriptions/listen )?line" src/client/subscriptions.rs src/shared/http.rs`
  → no match.
- `grep -rniE "one (undelivered|in-progress) event" src/shared/sse_parser.rs src/shared/http.rs src/client/subscriptions.rs`
  → no match (MEDIUM-1 consistency).
- `grep -c "SseParser::new()" src/shared/http.rs` → 0; `grep -c "DEFAULT_MAX_BUFFER_SIZE" src/shared/http.rs`
  → 0; `grep -c "connect_sse_keeps_the_shared_default_bound" src/shared/http.rs` → 0.
- `grep -nE "TODO|FIXME|XXX|TBD"` over all four files → no match (zero SATD).

## Threat Register Dispositions

| Threat ID | Disposition | Status after this plan |
|---|---|---|
| T-113-79 | mitigate | **Done.** `feed`'s pre-check covers `buffer + current_event.data + chunk` unconditionally and discards both accumulators on trip. Proven by the 100 000-iteration newline-carrying flood test and its negative control (run 2). |
| T-113-80 | mitigate | **Done.** The `contains('\n')` escape is deleted; a 1 000 000-byte line under a 64-byte bound is refused, not emitted. |
| T-113-81 | mitigate | **Done.** `feed_complete_body` is `pub(crate)` (no downstream crate can reach it) and the tripwire grep confirms neither incremental feeder references it. |
| T-113-84 | mitigate — **in plan 113-20** | **Deliberately NOT claimed here.** Both call sites still use a bare uncapped `response.collect()`. The bypass's rustdoc states the byte cap as a REQUIREMENT ON THE CALLER and names 113-20; no source comment asserts the boundary is capped. |
| T-113-82 | accept | Both messages name only `parser.max_buffer_size()` and the peer's behaviour. Asserted: the listen flood test checks the message contains `64` and contains no `'A'` from the fed payload. |
| T-113-83 | accept (residual) | An attacker can still pin the configured ceiling per stream (256 KiB per listen stream; the configured `HttpTransport` ceiling per reader task) plus a small constant multiple for the non-accumulating `id`/`event` fields — documented precisely on `buffered_bytes()` (review LOW). Memory is now a function of one payload, not of stream age. |
| T-113-85 | accept, with an escape hatch | The ceiling is configurable via `with_sse_buffered_bytes`, with below/at/above boundary tests (pinning that the comparison is `>`, so exactly the ceiling is admitted), an escape-hatch test, and the base64 test that withdraws the "media unaffected" claim. |
| T-113-86 | accept | The bound is `retained + this chunk`. Documented as such in `feed`'s comment, `listen_overflow`'s doc and message, `report_sse_overflow`'s doc and log, and `MAX_LISTEN_LINE_BYTES`'s doc. Nothing anywhere claims "one undelivered event". |
| T-113-SC | mitigate | No package installed, no manifest touched — `git diff --name-only -- Cargo.toml Cargo.lock` is empty. No package-legitimacy checkpoint was required. |

## Decisions Made

1. **The bound covers `retained state + THIS CHUNK`, not "one in-progress event"** (T-113-86, review MEDIUM-1). A chunk carrying many small COMPLETE events is refused on the chunk total, so behaviour depends partly on transport framing. Accepted rather than restructured: evaluating the bound only over post-split retained state requires parsing the whole unbounded chunk first, i.e. performing exactly the allocation the bound prevents. The cost is a documentation obligation, honoured in all three modules.
2. **The post-drain residual check is retained even though the unconditional pre-check makes it unreachable in the shipped configuration.** Negative-control run 1 shows the two are independently sufficient against different inputs; keeping both means a future change that weakens one must still satisfy the other.
3. **`feed_complete_body` is `pub(crate)`, not `pub`** — a public unbounded parser entry point is an attractive nuisance, and crate-privacy keeps the semver verdict trivially additive.
4. **The SSE ceiling is a private transport field, not an `HttpConfig` field** — the latter is a measured MAJOR semver break (`constructible_struct_adds_field`), and `Default` does not help because a downstream struct literal enumerating every field is still legal.
5. **`MAX_LISTEN_LINE_BYTES` was NOT renamed** (see Deviations #2).
6. **The two limits are deliberately NOT unified.** This plan's ceiling bounds INCREMENTAL in-flight retention on a long-lived reader; 113-20's cap bounds a ONE-SHOT collected body. They share no config surface (`grep -c HttpConfig src/shared/streamable_http.rs` is 0) and are two different concepts.

## Deviations from Plan

### Executor-discretion decisions (no deviation rule invoked — no bugs were found)

**1. Constant named `DEFAULT_HTTP_SSE_BUFFERED_BYTES`, not `MAX_HTTP_SSE_BUFFERED_BYTES`**
- **Found during:** Task 2 (item 4)
- **Issue:** The plan is internally inconsistent — its frontmatter `must_haves`, `artifacts[].contains` and Task 2's action text all say `DEFAULT_HTTP_SSE_BUFFERED_BYTES`, while one acceptance-criterion line says `MAX_HTTP_SSE_BUFFERED_BYTES`.
- **Resolution:** Used `DEFAULT_HTTP_SSE_BUFFERED_BYTES` — it is the artifact contract, it appears three times to the other's one, and `DEFAULT_` is the accurate prefix for an overridable value.
- **Committed in:** `25ae4a17`

**2. `MAX_LISTEN_LINE_BYTES` deliberately not renamed**
- **Found during:** Task 2 (item 2)
- **Issue:** The plan makes the rename discretionary. Renaming would require editing the intra-doc link inside `decode_listen_chunks_for_fuzz`'s rustdoc and the name reference at `fuzz/fuzz_targets/subscription_listen_frames.rs:41` — both explicitly fenced to plan 113-19, which is executing against this same working tree in this wave (`workflow.use_worktrees` is `false`). Renaming and leaving those stale would break a rustdoc intra-doc link.
- **Resolution:** Kept the name; rewrote its doc to open with what it really bounds ("Named for the line buffer it originally bounded; since 113-17 it bounds BOTH of the parser's accumulators … It is NOT a per-line limit, and no message derived from it may say it is"). The non-discretionary requirement — that no doc or message claims a per-line bound — is met, and both greps confirm it.
- **Committed in:** `25ae4a17`

**3. The escape-hatch test is expressed at a scaled-down ceiling**
- **Found during:** Task 2 (item 7)
- **Issue:** The acceptance criterion asks for "a payload refused at the DEFAULT ceiling [that] succeeds once the ceiling is raised". Taken literally that needs a >16 MiB allocation in a unit test, which the plan itself forbids for the sibling base64 test.
- **Resolution:** `raising_the_ceiling_admits_a_payload_the_lower_one_refuses` proves the same wiring at 256 → 1024 bytes, and `connect_sse_uses_its_own_named_bound` separately asserts that `HttpTransport::new` defaults from `DEFAULT_HTTP_SSE_BUFFERED_BYTES` and that `sse_reader_parser` carries that value. Together they establish "default = the named constant", "the builder overrides it", and "the parser is built from the configured value" without allocating 16 MiB.
- **Committed in:** `25ae4a17`

**4. SEMVER / public-api verified once, after Task 2**
- **Found during:** Task 1
- **Issue:** Both checks are listed under Task 1's acceptance criteria, but Task 1 adds no public surface at all (both new parser methods are `pub(crate)`; everything else is doc text and a call-site swap).
- **Resolution:** Ran both once after Task 2, covering both commits. Result: 223/223 pass, no update required, no `constructible_struct_adds_field`.

**5. The two `streamable_http.rs` call-site comments avoid re-naming `feed_complete_body`**
- **Found during:** Task 1 (item 7)
- **Issue:** The first draft's comments each named the identifier, making `grep -c "feed_complete_body" src/shared/streamable_http.rs` return 4 and breaking the plan's "exactly 2" tripwire.
- **Resolution:** Reworded both comments to "Deliberately the COMPLETE-body entry point rather than `feed`: …". The comments still carry the full rationale and the 113-20 pointer; the tripwire is now exact at 2.
- **Committed in:** `57d7a211`

---

**Total deviations:** 0 auto-fixed under Rules 1–3 (no bugs, missing critical functionality or blockers were encountered); 5 documented executor-discretion decisions, all within the plan's stated latitude or resolving an internal plan inconsistency.
**Impact on plan:** None on scope. Every `must_haves` truth and every artifact `contains` string is satisfied.

## Issues Encountered

- **`make quality-gate` exceeds the 10-minute foreground tool timeout.** Re-run as a background job writing to a log; exit code captured explicitly. Result: `GATE_EXIT=0`.
- **The gate's fuzz stage fails to build ~17 targets** (`-Zsanitizer=address` is nightly-only and the gate invokes it under stable) and the gate still exits 0. This is the pre-existing, plan-anticipated D-113-G behaviour, not a regression from this plan. It does mean the fuzz targets — including `subscription_listen_frames`, whose seam is bounded by `SseParser::with_max_buffer_size` — were NOT rebuilt against the new bound by the gate. Plan 113-19 owns that seam.

## Known Stubs

None. No hardcoded empty values, placeholder text or unwired data paths were introduced.

## Scope-Fence Compliance

- `src/server/subscriptions.rs` — untouched (plan 113-18).
- `fuzz/fuzz_targets/subscription_listen_frames.rs` and `decode_listen_chunks_for_fuzz` — untouched (plan 113-19). Note for 113-19: that fuzz target's line-41 doc comment describes the listen bound as a per-line limit; the production doc it mirrors was corrected here, so the comment is now stale prose in a fenced file.
- The collected-body cap at both `response.collect()` sites — untouched (plan 113-20), by design.

## Next Phase Readiness

- **113-20 is unblocked and now load-bearing.** `feed_complete_body`'s precondition is stated but not yet satisfied: both call sites still collect an uncapped body. Until 113-20 lands, T-113-84 remains a real (documented) unbounded surface upstream of the parser — the plan deliberately moved the honesty into the rustdoc rather than claiming a cap that does not exist.
- **HTTP-04 / Success Criterion 3 ("memory-bounded long-lived stream") is now verifiable** on both incremental feeders: `overflowed()` latches and each feeder's per-chunk poll ends its stream.
- **Behaviour change to flag for release notes:** `HttpTransport::connect_sse` now discards and ends the reader task on a payload exceeding 16 MiB of in-flight bytes. `with_sse_buffered_bytes()` is the escape hatch. Base64 media is materially affected (~4/3 expansion), contrary to the earlier withdrawn claim.

## Self-Check: PASSED

All claimed files exist on disk (`113-17-SUMMARY.md`, `src/shared/sse_parser.rs`,
`src/shared/streamable_http.rs`, `src/shared/http.rs`, `src/client/subscriptions.rs`) and all
three claimed commits are reachable in `git log --oneline --all`: `57d7a211`, `25ae4a17`,
`253a7db8`.

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-27*
