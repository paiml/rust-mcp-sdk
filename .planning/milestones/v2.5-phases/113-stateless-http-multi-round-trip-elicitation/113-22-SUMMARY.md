---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 22
subsystem: testing
tags: [http-09, complexity-guard, falsifiability, sse, utf8-decode, dos, timing-test]

requires:
  - phase: 113
    provides: "the O(n) `take_utf8_prefix` itself (`5f045086`), `SseParser::feed`'s unconditional pre-check (113-17), and 113-21's structural enumeration half of HTTP-09"
provides:
  - "a falsifiable wall-clock budget for `take_utf8_prefix` — 1 MiB under a 1 s ceiling, measured 31x under linear and 7.9x over quadratic"
  - "a 4x-step growth-ratio guard that self-disables loudly below a 50 us resolution floor instead of asserting on noise"
  - "a corrected rustdoc on the pre-existing guard, which documented a falsifiability it does not have"
  - "the <=3-byte retained-tail bound proven over arbitrary bytes by proptest, replacing three hand-picked fixtures"
  - "D-113-R: SseParser::feed is QUADRATIC over peer-chosen chunking — 833 ms per 256 KiB in a RELEASE build"
affects: [verify-phase-113, HTTP-09 closure, any future src/shared/sse_parser.rs work]

tech-stack:
  added: []
  patterns:
    - "complexity guard = absolute ceiling (load-bearing) + growth ratio (secondary, self-disabling), both sized from MEASURED opposite-side values"
    - "min-of-N, never mean: noise raises a minimum only by raising every sample, and a quadratic shape cannot get lucky"
    - "a timing guard emits its measurement on SUCCESS, so the margin is observable before the day it fails"
    - "pair every time budget with an OUTPUT assertion at the same size, so 'fast because it decoded less' cannot pass"

key-files:
  created: []
  modified:
    - src/shared/sse_parser.rs
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md

key-decisions:
  - "113-22: the pre-existing guard's falsifiability claim is DISPROVEN by execution, not by arithmetic — with the quadratic shape restored it passes in 0.54 s; the claim is corrected in source rather than deleted, so the next reader knows which guard is load-bearing"
  - "113-22: the absolute budget's margin is 31x over linear, not the plan's estimated 200x — the plan assumed a release-build 'sub-5 ms' figure while nextest builds this module at opt-level 0 (32 ms); the measured number is written into the source, not the estimate"
  - "113-22: min-of-N is the statistic (3 runs for the budgets, 5 for the ratio); the timed closure includes the buffer construction, which over-measures and is therefore conservative"
  - "113-22: the ratio guard skips loudly below 50 us rather than asserting on sub-resolution timings; the absolute budget stays load-bearing so the guard never degrades to nothing"
  - "113-22: FOUND (out of scope, recorded not fixed) D-113-R — SseParser::feed re-scans the whole retained buffer per call and a peer chooses the chunking; 833 ms per 256 KiB in a RELEASE build, same class as the CR-02 BLOCKER"
  - "113-22: the feed budget test is documented in-source as a CEILING, not a complexity proof, with its blind spot MEASURED (a per-chunk full-buffer copy moves it 6.7 ms -> 11.7 ms and still passes) rather than left for a reviewer to discover"
  - "113-22: no new fuzz target and no example — the reasons (D-113-G builds 0 of 17 targets; this plan ships no feature surface) are stated in the test module's rustdoc instead of satisfying the checklist with motion"
  - "113-22: no requirement checkbox flipped; HTTP-09's O(n) clause is explicitly NOT fully discharged while D-113-R is open"

patterns-established:
  - "Size a complexity guard from BOTH sides measured: run the defect, record what it costs, and put the ceiling between the two numbers with both margins written into the failure message"
  - "When a mandated test cannot fail on the threat it is assigned, measure the blind spot and say so in the test's own rustdoc — the failure mode being fixed is a guard nobody knew was inert"

requirements-completed: []

duration: 28min
completed: 2026-07-27
---

# Phase 113 Plan 22: HTTP-09 Linear-Time Budget Summary

**A wall-clock budget that actually fails when `take_utf8_prefix` goes quadratic — 1 MiB under a 1 s ceiling, sized from measurements on both sides (32 ms linear, 9.39 s quadratic) — plus the proof that the guard it replaces passes in 0.54 s on the exact defect it named, and a new BLOCKER-class finding (D-113-R) that `SseParser::feed` is itself quadratic over peer-chosen chunking.**

## Performance

- **Duration:** 28 min
- **Started:** 2026-07-27T08:01Z
- **Completed:** 2026-07-27T08:30Z
- **Tasks:** 2 of 2
- **Files modified:** 2 (`src/shared/sse_parser.rs` +112 test lines and one corrected rustdoc; `deferred-items.md` +76)

## Task Commits

1. **Task 1: falsifiable budget + growth ratio + feed ceiling + corrected rustdoc** — `46254173` (test)
2. **Task 2: retained-tail proptest + named companion + ALWAYS statement + gate** — `d6698a5d` (test)

## The headline: the old guard passes on the defect it names

The plan predicted this arithmetically. It was executed rather than asserted.

With the pre-`5f045086` quadratic shape restored in `take_utf8_prefix`:

| Test | Result | Measurement |
|---|---|---|
| `take_utf8_prefix_is_linear_over_a_large_invalid_run` (pre-existing) | **PASS** | 0.539 s |
| `take_utf8_prefix_stays_within_its_linear_time_budget` (new) | **FAIL** | 9.389 s vs 1 s ceiling |
| `take_utf8_prefix_cost_grows_linearly_not_quadratically` (new) | **FAIL** | 15.38x vs 8.0x ceiling |
| `sse_parser_feed_stays_within_its_linear_time_budget` (new) | PASS | 6.997 ms — correctly unaffected, `feed` takes already-decoded text |

The pre-existing test's rustdoc claimed its 256 KiB input size "makes a reintroduced quadratic shape hang the suite instead of passing it". It passes it, in half a second. That sentence is now corrected in source — kept and marked wrong rather than deleted, so the next reader learns which guard is load-bearing instead of re-trusting a claim this plan disproved.

## Measured numbers (all at `opt-level = 0`, the profile `cargo nextest` builds this module with)

| shape | 256 KiB | 1 MiB | ratio (4x step) |
|---|---|---|---|
| committed single-pass cursor scan | 9.18 ms | 31.8 ms | **3.46x** |
| restored pre-`5f045086` quadratic | 514.5 ms | 7.91 s (min of 5) / 9.39 s (min of 3) | **15.38x** |

The 1 s ceiling therefore sits **31x above linear** and **7.9x below quadratic**. Both margins, and the fact that both were measured, are written into the assertion's failure message.

## Deviations from Plan

### 1. [Measurement correction] The absolute budget's margin is 31x, not the plan's ~200x

The plan's `<interfaces>` derived its margin from "the single-pass shape is one `from_utf8` validation plus one `String` build: 1 MiB is sub-5 ms on any machine". That is a release-build intuition, and it is also wrong about the algorithm: over 1 MiB of `0xFF` the cursor scan performs **one million** loop iterations (one `from_utf8` call and one `push('\u{FFFD}')` per invalid byte), not one validation. At `opt-level = 0` that costs **31.8 ms**, so the real margin over linear is **31x**, not 200x.

31x is still ample and the ceiling was NOT moved: a machine would have to be 31x slower than this one *on every one of three runs* to fail spuriously, and the quadratic side still overshoots by 7.9x. The measured figure — not the estimate — is what went into the rustdoc, the failure message and the table above.

### 2. [Rule 2 — a mandated guard could not fail on its assigned threat] The `feed` budget's blind spot was measured and documented

The plan assigns T-113-102 ("an accidental `O(buffered)` copy per chunk in `SseParser::feed`") the mitigation "a 256-chunk retained-state budget test on the public entry point". Negative control: injecting exactly that defect (`std::hint::black_box(self.buffer.clone())` at the top of `feed`) moved the measurement from **6.717 ms to 11.702 ms** — a 1.74x constant-factor change against a 1 s ceiling. **The test passes on the threat it is assigned.**

That is the same failure mode this plan exists to end, so it was not left implicit. The test's rustdoc now carries a `# This is a CEILING, not a complexity proof` section stating the negative-control numbers, why no ceiling at 4 KiB chunking can separate the two shapes (both are memchr-class scans over at most 1 MiB), and where the falsifiable claim actually lives. The test is kept — it is a real ceiling against an egregious per-chunk regression, and it pins the retention contract (nothing dispatched, nothing overflowed, every byte retained) — but it no longer implies more than it proves.

### 3. [Rule 4 boundary — FOUND, recorded, NOT fixed] D-113-R: `SseParser::feed` is quadratic over peer-chosen chunking

Investigating why the `feed` budget could not discriminate surfaced the reason: `drain_complete_lines` runs `self.buffer.find('\n')` over the **whole** retained buffer on every call, re-scanning the prefix every earlier call already scanned. The `debug_assert!(!self.buffer.contains('\n'))` immediately above it states exactly why that re-scan is waste — the loop leaves the buffer newline-free on every return.

**A peer chooses the chunking.** Both incremental feeders call `feed` once per `hyper` body frame (`src/shared/http.rs:371-378`, `src/client/subscriptions.rs:248-255`), and a server chooses its HTTP chunked framing. Measured with `cargo nextest run --release`, so this is the **shipped** cost, feeding single-byte chunks:

| retained bytes | cost | vs. 16 KiB |
|---|---|---|
| 16 KiB | 5.61 ms | 1x |
| 64 KiB | 59.25 ms | 10.6x for 4x input |
| 256 KiB | 832.6 ms | **148x for 16x input** |

256 KiB is exactly `MAX_LISTEN_LINE_BYTES`. For comparison, review CR-02 — a **BLOCKER** — was 1.17 s for 400 KiB. On `connect_sse`'s 16 MiB `DEFAULT_HTTP_SSE_BUFFERED_BYTES` the same shape is 64x the input and ~4096x the work.

Note the perverse interaction with this phase's other work: every Phase-113 bound is a **byte** bound, and this cost is quadratic *in that bound*. Raising a memory ceiling makes this worse, which is why `connect_sse` is the severe case.

**Not fixed here, deliberately.** This plan's fence is test-only (its verification step expects `src/shared/sse_parser.rs` and only its `#[cfg(test)]` region plus one rustdoc). The fix is a production change to the line splitter — the function with the T-113-67 remote-panic history, where a byte-vs-character index confusion was a remote-triggerable client crash found by a property test rather than by review. It needs its own tests, fuzz run and review. This follows 113-21's precedent with D-113-Q: enumerate loudly, do not drive-by fix. The fix shape (a `search_from` cursor) is written out in `deferred-items.md`.

**Consequence for the requirement, stated plainly: HTTP-09's O(n) clause is NOT fully discharged.** `take_utf8_prefix` now has a falsifiable O(n) guard. `SseParser::feed`, the other scan over peer-chosen input on the same paths, does not have the property. The plan's must-have truth "`SseParser::feed` carries the same guarantee as the decoder that runs before it" is **disproven**, not established.

### 4. [Addition] Budget tests emit their measurement on success

Not in the plan. A wall-clock guard whose margin is invisible until the day it fails cannot be maintained, and this plan exists because a margin nobody had measured turned out to be zero. Each budget test `eprintln!`s its measurement and ceiling (visible with `--success-output=immediate` or `--nocapture`); nextest hides it on a passing run, so the default output is unchanged.

### 5. [Addition] A third negative control, on the property test

The plan mandated one negative control. A second (deviation 2) and a third were run. For the third, the incomplete-tail arm of `take_utf8_prefix` was changed to retain the whole remaining buffer; `property_take_utf8_prefix_retains_at_most_a_three_byte_tail` failed with `retained 2181 bytes`. That failure also exposed a readability defect in the assertion — it hex-dumped all 2181 bytes — so the message now prints the length plus the first 8 bytes.

## Negative controls — verbatim

Baseline before each: 28 (later 30) tests green in `sse_parser::tests`. Each control was applied to a scratch-backed copy of the file and restored byte-for-byte (SHA-256 verified for NC-1).

### NC-1 (mandated) — the pre-`5f045086` quadratic shape restored

Restored exactly as `take_utf8_prefix`'s own rustdoc describes it: re-validate `buffer` from index 0 each iteration, `buffer.drain(..valid_up_to + invalid_len)` per invalid run.

```
        PASS [   0.539s] pmcp shared::sse_parser::tests::take_utf8_prefix_is_linear_over_a_large_invalid_run

        FAIL [  28.348s] pmcp shared::sse_parser::tests::take_utf8_prefix_stays_within_its_linear_time_budget
    take_utf8_prefix: 1048576 invalid bytes in 9.388538375s (ceiling 1s)

    thread '...take_utf8_prefix_stays_within_its_linear_time_budget' panicked at src/shared/sse_parser.rs:867:9:
    take_utf8_prefix took 9.388538375s (minimum of 3 runs) over 1048576 bytes of invalid input; the ceiling is 1s.

    This excludes ONE shape: re-validating the buffer from index 0 each iteration and performing one `Vec::drain` per invalid run, which is O(n^2) byte moves. [...]

    A measurement in the upper region means the quadratic shape is back, not that the machine is slow — no machine that can run this suite needs a second to scan 1 MiB once. Do NOT raise this number to make the test pass; that converts the guard back into the unfalsifiable one this replaced (review CR-02, plan 113-22).

        FAIL [  46.917s] pmcp shared::sse_parser::tests::take_utf8_prefix_cost_grows_linearly_not_quadratically
    take_utf8_prefix growth: 262144 -> 514.519333ms, 1048576 -> 7.912875041s, ratio 15.38x (ceiling 8.0x)

    thread '...take_utf8_prefix_cost_grows_linearly_not_quadratically' panicked at src/shared/sse_parser.rs:944:9:
    take_utf8_prefix cost grew 15.4x for a 4x input step (262144 bytes -> 514.519333ms, 1048576 bytes -> 7.912875041s); the ceiling is 8.0x.

        PASS [   0.044s] pmcp shared::sse_parser::tests::sse_parser_feed_stays_within_its_linear_time_budget
    SseParser::feed: 256 x 4096 retained bytes in 6.996708ms (ceiling 1s)

     Summary [  46.919s] 5 tests run: 3 passed, 2 failed, 1590 skipped
```

**The point of the exercise, on the record:** `take_utf8_prefix_is_linear_over_a_large_invalid_run` — the guard whose rustdoc claimed a reintroduced quadratic shape would hang the suite — **passed, in 0.539 seconds.** Reverted (SHA-256 `7a8517ed…` restored) → 28 passed.

### NC-2 (added) — T-113-102's per-chunk full-buffer copy in `feed`

Injected `std::hint::black_box(self.buffer.clone());` at the top of `SseParser::feed`.

```
        PASS [   0.061s] pmcp shared::sse_parser::tests::sse_parser_feed_stays_within_its_linear_time_budget
    SseParser::feed: 256 x 4096 retained bytes in 11.701542ms (ceiling 1s)
     Summary [   0.062s] 1 test run: 1 passed, 1595 skipped
```

6.717 ms → 11.702 ms, still 85x under the ceiling. **The mandated mitigation does not fire on its assigned threat.** Recorded in the test's rustdoc. Reverted → green.

### NC-3 (added) — the retained-tail invariant broken

Changed the `error_len() == None` arm to return without draining, so the whole remaining buffer is retained.

```
    thread '...property_take_utf8_prefix_retains_at_most_a_three_byte_tail' panicked at src/shared/sse_parser.rs:1122:5:
    Test failed: retained 2181 bytes ([ea, 87, 63, 4c, bd, 6, 7a, bd, ...]) — the incomplete-character tail is at most 3, and a larger residual is unbounded accumulation across chunks, not a decode detail at src/shared/sse_parser.rs:1141.
    minimal failing input: bytes = [234, 135, 99, 76, 189, 6, ...]
     Summary [   0.223s] 1 test run: 0 passed, 1 failed, 1596 skipped
```

Reverted → green.

## What landed in `src/shared/sse_parser.rs`

All inside `#[cfg(test)] mod tests` except one corrected rustdoc paragraph, so 113-21's tripwire (which excludes `cfg(test)` regions per-item) is untouched — its `sse_parser.rs push_str( = 4` allowlist entry still matches.

| Item | Kind | What it proves |
|---|---|---|
| `min_elapsed(runs, body)` | helper | minimum of N runs; noise raises a minimum only by raising every sample |
| `take_utf8_prefix_stays_within_its_linear_time_budget` | UNIT | **load-bearing**: 1 MiB under 1 s, plus the output pinned at that size so "fast because it decoded less" cannot pass |
| `take_utf8_prefix_cost_grows_linearly_not_quadratically` | UNIT | secondary: 4x step under an 8x ratio ceiling; self-disables loudly below 50 us |
| `sse_parser_feed_stays_within_its_linear_time_budget` | UNIT | a ceiling on the public entry point + the retention contract; blind spot documented (D-113-R) |
| `take_utf8_prefix_retained_tail_is_documented_bound` | UNIT | the named companion: 3 bytes against a truncated U+1F600 |
| `property_take_utf8_prefix_retains_at_most_a_three_byte_tail` | PROPERTY | residual < 4, residual is a suffix of the input, and one ASCII byte always clears it (anti-wedge) |
| `take_utf8_prefix_is_linear_over_a_large_invalid_run` | UNIT (pre-existing) | its output assertions, kept; its falsifiability claim, corrected |

### ALWAYS requirements

Stated in the test module's own rustdoc rather than left to a reviewer:

- **PROPERTY** — the new proptest.
- **UNIT** — the budget, ratio, companion and pre-existing output tests.
- **FUZZ** — the **existing** `fuzz_listen_frames` target (113-16), which already drives this decoder through `decode_listen_chunks_for_fuzz`. No new target, and the reason is recorded: D-113-G notes the gate's fuzz stage builds 0 of 17 targets and swallows failures. Adding an 18th that also would not build is motion, not coverage; this plan does not adopt D-113-G.
- **EXAMPLE** — none. This plan ships no new feature surface, only guards on existing behaviour. Said plainly rather than inventing one.

## Verification

| Check | Result |
|-------|--------|
| `cargo nextest run --features full --lib -- sse_parser::tests` | **30 tests run: 30 passed** (was 28) |
| `make lint` (clippy `-D clippy::all -W pedantic -W nursery -W cargo`, `RUSTFLAGS=-D warnings`) | **exit 0, "✓ No lint issues"** |
| `cargo fmt --all -- --check` | clean |
| `make quality-gate` (background job, log polled) | **exit 0** (`QUALITY_GATE_EXIT=0`), "ALL TOYOTA WAY QUALITY CHECKS PASSED" |
| — test totals in that log | **246 `test result:` lines, 246 `ok`, 0 FAILED** |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks: 223 pass, 30 skip — no semver update required** |
| Negative controls | 1 mandated + 2 added; all three produced the expected failure; all three reverted |
| `git diff --stat` (this plan's commits) | `src/shared/sse_parser.rs` + `deferred-items.md` only |
| `.planning/REQUIREMENTS.md` | **untouched**; no checkbox flipped |

The rtk output-filtering problem 113-21 hit did **not** recur — this gate log carried all 246 `test result:` lines. Test totals were nonetheless taken through the absolute `cargo` binary throughout.

## On flakiness — measured, not tuned

Per the plan's own warning, the budget was **not** widened until it passed. It was sized once, from measurements on both sides, and never moved. Standing margins:

- **Absolute budget:** 31x headroom, against the **minimum of 3** runs. A transient stall would have to hit all three.
- **Ratio guard:** observed 3.46x against an 8.0x ceiling — 2.3x headroom, minimum of 5 at each size. This is the weaker of the two and it is labelled as such in source; it is why the absolute budget carries the requirement.
- **Feed ceiling:** 6.7 ms against 1 s — 149x. Never at risk, and never load-bearing either (deviation 2).

Every run during this plan — including runs concurrent with a full `make quality-gate` compiling in the background — landed within a few percent of these figures (31.76 ms / 31.82 ms across separate invocations). No instability was observed, and none is being papered over.

## Known Stubs

None. Every test added is live and asserted; `min_elapsed` is exercised by all four budget/ratio tests.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: denial-of-service | `src/shared/sse_parser.rs` (`drain_complete_lines`, behind `SseParser::feed`) | **D-113-R.** `find('\n')` re-scans the whole retained buffer per call while a peer chooses the chunking (one `feed` per `hyper` body frame). Release-build measurement: 833 ms per 256 KiB of single-byte chunks, quadratic — 148x for 16x input. Reachable on both incremental feeders, and *worsened* by this phase's larger byte ceilings (16 MiB on `connect_sse`). Same class as the CR-02 BLOCKER. |

Threat register dispositions from the plan:

| Threat ID | Disposition as landed | Proof |
|-----------|----------------------|-------|
| T-113-101 | **discharged** — 1 MiB / 1 s budget + 4x-step ratio, both falsifiable | NC-1: 9.389 s and 15.38x, both named failures |
| T-113-102 | **NOT discharged** — the mandated mitigation passes on the threat | NC-2: 6.7 ms → 11.7 ms, still PASS. Superseded by D-113-R, which is the real defect on this component |
| T-113-103 | **discharged** — proptest over `vec(any::<u8>(), 0..4096)`: residual < 4, suffix, and a second call always clears | NC-3: `retained 2181 bytes` |
| T-113-104 | **discharged** — the rustdoc claim is corrected with measured arithmetic, and the negative control recorded the old test passing on the defect | NC-1: PASS in 0.539 s |

## Requirement status — no checkbox flipped

**HTTP-09 remains `[ ]`**, for two independent reasons:

1. The STATE.md publication gate forbids flipping any of HTTP-01..09 / CLNT-01/02/05 this round. `.planning/REQUIREMENTS.md` was not opened for edit and `requirements mark-complete` was deliberately not run.
2. **Substantively, the O(n) clause is not fully satisfied.** D-113-R is an open, measured, peer-reachable quadratic scan on the same paths the requirement names.

## For the next agent

- **Do not close HTTP-09 without D-113-R.** The requirement's wording is "no scan over peer-chosen input is worse than O(n)". `take_utf8_prefix` now satisfies it and has a guard that fails when it stops. `SseParser::feed` does not satisfy it. The fix shape is in `deferred-items.md`; it is ~10 lines plus its own tests and a fuzz run.
- **Do not raise the 1 s ceiling.** Both failure messages say so and explain why. If it fires, the quadratic shape is back — the margin against linear is 31x on a *minimum of three runs*.
- **The feed budget test does not prove what its name suggests.** Read its `# This is a CEILING, not a complexity proof` section before relying on it.
- **`take_utf8_prefix_is_linear_over_a_large_invalid_run` is an OUTPUT test.** Its value is catching a decoder that got fast by decoding less — which is exactly why the new budget test duplicates its output assertions at 1 MiB. Do not delete either as redundant.

## Self-Check: PASSED

- `src/shared/sse_parser.rs` — FOUND (modified, 30 tests green)
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md` — FOUND (D-113-R present)
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-22-SUMMARY.md` — FOUND
- Commits `46254173`, `d6698a5d` — FOUND
- Negative-control edits — all reverted; `git diff src/` scoped to the intended file only
