---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 16
subsystem: testing
tags: [fuzzing, libfuzzer, sse-parser, subscriptions-listen, dos, evidence, gap-closure]

# Dependency graph
requires:
  - phase: 113-stateless-http-multi-round-trip-elicitation
    provides: "plan 13's `subscription_listen_frames` target and the `decode_listen_chunk_for_fuzz` seam; plan 15's bounded `SseParser` (`with_max_buffer_size`, latching `overflowed()`) and the `listen_overflow` observer"
provides:
  - "a RUN 20 000-iteration libFuzzer campaign against `subscription_listen_frames` — exit 0, artifacts/ EMPTY — recorded reproducibly in `113-FUZZ-EVIDENCE.md`"
  - "`decode_listen_chunks_for_fuzz(chunks, id, max_buffer_size)`: the multi-chunk, explicitly-bounded fuzz seam that drives `SseParser::with_max_buffer_size` and reports the production `listen_overflow` observer per chunk"
  - "positive proof that the campaign covered plan 15's discard-and-latch branch, plus the measurement showing a single 64-byte bound covers it ZERO times in a 20 000-run budget"
affects: [113 verification gap item 5, any future campaign against this target]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "bound-under-test as a fuzz parameter: the seam takes `max_buffer_size` so a campaign can pick a bound its generated inputs actually reach, instead of fuzzing a production constant it can never exceed"
    - "branch-coverage proof from the retained corpus: count entries that satisfy the enforcement predicate by construction, rather than asserting 'the fuzzer probably hit it'"
    - "multi-bound fuzz loop: one pass per bound so the ordinary path and the enforcement path are both covered by every input"

key-files:
  created:
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/113-FUZZ-EVIDENCE.md
  modified:
    - fuzz/fuzz_targets/subscription_listen_frames.rs
    - src/client/subscriptions.rs
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md

key-decisions:
  - "A bounded SIBLING (`decode_listen_chunks_for_fuzz`) rather than a changed signature on the existing `#[doc(hidden)] pub fn` — the plan's stated preference, and it keeps the semver verdict trivially confirmable (223/223 pass, no update required)"
  - "The seam feeds a SEQUENCE of chunks carrying the SSE line buffer and the undecoded-UTF-8 tail across boundaries, mirroring `read_next_frame`; a single-chunk seam cannot reach mid-character or mid-line splits at all"
  - "The seam reports the PRODUCTION observer `listen_overflow(&parser).is_some()` per chunk, not `parser.overflowed()` directly, so the campaign drives the same predicate the live stream ends on"
  - "`decode_listen_chunk_for_fuzz` now delegates at `MAX_LISTEN_LINE_BYTES` (256 KiB), the bound a live listen stream uses — closing plan 15's hand-off note that it still built a default 1 MiB parser"
  - "TWO bounds `[64, 8]` per input, not one: MEASURED, a 64-only campaign covered the discard-and-latch branch zero times because libFuzzer's length ramp reached only 38-byte inputs in 20 000 runs (retained corpus max 53 bytes, 0 entries over 64)"
  - "Coverage is guaranteed by the TARGET, not by invocation flags or a seeded corpus — `fuzz/.gitignore` ignores `corpus`, so a seed would not survive for the next reader, and `-max_len`/`-len_control` would live only in a command line"
  - "The cross-delivery assertion is skipped for inputs containing a backslash (verification finding WR-08): a `\\u`-escaped id decodes to the SAME id, so asserting there would have minted a spurious crash artifact in the very campaign this plan exists to record"
  - "D-113-G was NOT fixed (scope fence): the campaign was invoked directly and the evidence names the bypass reason, with the gate's 17 swallowed build failures re-confirmed"

patterns-established:
  - "Evidence-over-assertion for fuzz claims: commit SHA + toolchain + seed + counters + an artifacts-empty proof naming the command, so 'we fuzzed it' is checkable (T-113-77)"
  - "Prove the fuzzer reached the branch: derive reachability from the enforcement predicate over the retained corpus and report the count, plus a single-input replay"

requirements-completed: [HTTP-04]

# Metrics
duration: 49min
completed: 2026-07-26
---

# Phase 113 Plan 16: libFuzzer Campaign for `subscription_listen_frames` Summary

**A 20 000-iteration libFuzzer campaign has actually RUN against the `subscriptions/listen` client frame decoder — exit 0, zero crash artifacts — and it demonstrably reached plan 15's discard-and-latch branch, which took discovering that the plan's own 64-byte bound covers that branch zero times inside a 20 000-run budget.**

## Performance

- **Duration:** 49 min
- **Started:** 2026-07-26T18:24:20Z
- **Completed:** 2026-07-26T19:13:20Z
- **Tasks:** 2
- **Files modified:** 4 (1 created)

## What Shipped

### Task 1 — the target drives the BOUNDED parser (`e4983a1f`, refined in `e37c381a`)

Plan 13's target called `decode_listen_chunk_for_fuzz`, which built its parser with
`SseParser::new()` — the 1 MiB default. Plan 15's executor flagged this in its hand-off note:
a campaign through that seam fuzzes a bound no listen stream uses and can never reach the
overflow path.

Added `decode_listen_chunks_for_fuzz(chunks, subscription_id, max_buffer_size)` in
`src/client/subscriptions.rs` (`#[doc(hidden)]`, the same internal-support-surface convention
as its single-chunk sibling). It:

- builds `SseParser::with_max_buffer_size(max_buffer_size)` — the key link this plan required;
- feeds a SEQUENCE of chunks, carrying both the SSE line buffer and the undecoded-UTF-8 tail
  across boundaries exactly as `read_next_frame` does, so mid-character and mid-line splits
  are reachable (they are not, with one chunk);
- returns `(outcomes, overflowed_per_chunk)` where each flag is
  `listen_overflow(&parser).is_some()` — the PRODUCTION observer, not a reconstruction;
- deliberately keeps feeding past the first overflow (a live stream ends there) so the LATCH
  itself is observable.

`decode_listen_chunk_for_fuzz` now delegates to it at `MAX_LISTEN_LINE_BYTES` (256 KiB),
signature and behaviour unchanged — the listen path's real bound, not the shared default.

The target asserts three invariants per input: no panic (T-113-67), no cross-delivery
(T-113-66), and the overflow latch never clears (T-113-73).

Three in-tree tests were added alongside, so none of this depends on a fuzz run to stay
correct: the latch test (`the_chunked_fuzz_seam_latches_overflow_and_never_clears_it`), a
cross-chunk reassembly test, and a proptest holding the campaign's own invariants.

### Task 2 — the campaign, and the evidence (`9a803e38`)

```
cargo +nightly fuzz run subscription_listen_frames -- -runs=20000
#20000  DONE  cov: 232  ft: 823  corp: 182/3497b  lim: 63  rss: 103Mb
Done 20000 runs in 0 second(s)                                    -> exit 0
```

at commit `e37c381a`, on `rustc 1.97.0-nightly (bf4fbfb7a 2026-04-11)`, from an EMPTY corpus,
with `fuzz/artifacts/subscription_listen_frames/` containing **0 entries** afterwards (the
directory EXISTS — `-artifact_prefix` creates it — so "absent" was not the observed case).

`113-FUZZ-EVIDENCE.md` records commands, toolchain table, commit SHA, libFuzzer seed
(`872967294`), counters, the artifacts-empty proof with the exact command, the branch-coverage
proof, a reproduction recipe, and the D-113-G bypass reason.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The plan's 64-byte bound covers the overflow branch ZERO times**

- **Found during:** Task 2, first campaign run
- **Issue:** Plan Task 1 specified a 64-byte bound so "libFuzzer's short inputs reach the
  overflow branch". They do not. libFuzzer ramps its length limit (`len_control`) and inside a
  20 000-run budget only reached 38-byte inputs; the retained corpus topped out at 53 bytes
  with **0 entries over 64**. An input that short can never push a 64-byte buffer past its
  bound, so the single riskiest new branch — the one this plan exists to fuzz — was never
  executed. The first campaign was green and meaningless.
- **Fix:** every input is now decoded once per bound, `[64, 8]`. 64 keeps fuzzing the ordinary
  tokenize / incremental-UTF-8 / classify path; 8 is tripped by any newline-free chunk of 9+
  bytes, i.e. by nearly every generated input from the first run. Coverage rose 226 → 232.
  Chosen over `-max_len`/`-len_control=0` flags or a seeded corpus because coverage then
  depends on how the target is INVOKED, and `fuzz/.gitignore` ignores `corpus` — neither would
  survive for the next reader.
- **Files modified:** `fuzz/fuzz_targets/subscription_listen_frames.rs`
- **Commit:** `e37c381a`

**2. [Rule 2 - Missing correctness] The cross-delivery assertion could mint a spurious crash**

- **Found during:** Task 1
- **Issue:** verification finding WR-08 (Info): the target asserted that a delivered
  notification implies the RAW bytes contain the subscription id literal. A JSON string of
  `\u`-escaped code points decodes to the same id without those bytes appearing, so a
  sufficiently lucky input would have produced a crash artifact in the very campaign this plan
  exists to record — and the "finding" would have been the oracle, not the decoder.
- **Fix:** the check is skipped when the input contains a backslash. An input with no backslash
  cannot carry such an escape, so the literal check applies exactly where it is sound; an
  escape-spelled id is the SAME id anyway, not the cross-tag escape T-113-66 is about.
- **Files modified:** `fuzz/fuzz_targets/subscription_listen_frames.rs`
- **Commit:** `e4983a1f`

### Out-of-scope discoveries (recorded, NOT fixed)

**D-113-H — a pre-existing untriaged crash artifact for the `auth_flows` target.** Found while
proving `fuzz/artifacts/` empty: `fuzz/artifacts/auth_flows/crash-e29e9da4b8b23e9e...`, 8 bytes,
dated **2025-09-12** — ten months before Phase 113, a different target, squarely outside this
plan's scope fence. `fuzz/.gitignore` ignores `artifacts`, so it was never committed and no CI
job has ever seen it; D-113-G is why nothing has flagged it since. Logged in
`deferred-items.md` with a replay command for whoever owns it.

**D-113-G stays open.** The scope fence forbade editing the Makefile. Re-confirmed rather than
assumed: the final gate log carries exactly **17** `failed to build fuzz script …
-Zsanitizer=address` errors and still exits 0.

**rtk output corruption on the artifacts proof.** `ls -A … | wc -l` through the repo's `rtk`
shell proxy returned a spurious `1` for a demonstrably empty directory. Every proof command in
the evidence file uses absolute binary paths (`/bin/ls`, `/usr/bin/wc`) for this reason; a
future re-verifier should too.

## Verification

Run in the plan's stated order:

| # | Check | Result |
|---|-------|--------|
| 1 | `cargo fuzz build --sanitizer=none subscription_listen_frames` (stable) | **exit 0** |
| 2 | `cargo +nightly fuzz build subscription_listen_frames` (ASan) | **exit 0** |
| 3 | `cargo +nightly fuzz run subscription_listen_frames -- -runs=20000` | **exit 0** — `#20000 DONE cov: 232 ft: 823 corp: 182/3497b` |
| 4 | `/bin/ls -A fuzz/artifacts/subscription_listen_frames/ \| /usr/bin/wc -l` | **0** (directory exists, empty) |
| 5 | `test -f …/113-FUZZ-EVIDENCE.md` | **true** |
| 6 | `cargo test --lib --features full -- client::subscriptions` | **30 passed, 0 failed** |
| 7 | `cargo test --test v2_subscriptions_client --features full` | **7 passed, 0 failed** |
| 8 | `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks: 223 pass, 30 skip — no semver update required** |
| 9 | `git diff --name-only -- Cargo.toml Cargo.lock fuzz/Cargo.toml` | **empty** |
| 10 | `make quality-gate` | **exit 0** |

Acceptance greps:

| Grep | Required | Actual |
|------|----------|--------|
| `grep -c 'overflowed' fuzz/fuzz_targets/subscription_listen_frames.rs` | ≥ 2 | **5** |
| `grep -c 'with_max_buffer_size\|max_buffer_size' fuzz/fuzz_targets/subscription_listen_frames.rs` | ≥ 1 | **5** |
| `grep -c 'runs=20000' 113-FUZZ-EVIDENCE.md` | ≥ 1 | **5** |
| evidence contains `git rev-parse HEAD` at run time (`e37c381a…`) | yes | **2 occurrences** |
| evidence contains the nightly version string | yes | **2 occurrences** |

### The branch-coverage proof (the claim most worth checking)

Any corpus entry whose first ≤16-byte chunk exceeds 8 bytes and contains no `\n` executes the
discard-and-latch branch at the 8-byte bound, by the enforcement condition in
`SseParser::feed`. Over the retained corpus:

```
corpus entries on disk: 180
entries driving the overflow branch on chunk 1: 50
entries larger than 64 bytes: 0 | max entry size: 61
```

`entries larger than 64 bytes: 0` is the same measurement stated twice: the 8-byte bound was
necessary, and a 64-only campaign of this size covers nothing of the overflow path. A
single-input replay of one such entry against the built binary exits 0.

## Requirements

HTTP-04 remains `[~]` implemented-pending-final-schema under the `113-SPEC-RECHECK.md` recorded
exception. **No requirement checkbox was flipped** — the plan's scope fence forbids it, and the
phase-level blocker (spec verdict PENDING until on/after 2026-07-28) is unchanged by this plan.

## Follow-ups

- **D-113-G** (gate's fuzz stage builds 0 of 17 targets and swallows failures) — still unowned.
- **D-113-H** (pre-existing `auth_flows` crash artifact) — new, unowned; replay command recorded.
- The corpus produced here is gitignored, so the next campaign also starts from empty. If this
  target is ever run in CI, a committed seed corpus (or a `-max_len`) would make it cheaper —
  but the target no longer NEEDS either to reach its enforcement branch.

## Self-Check: PASSED

- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-FUZZ-EVIDENCE.md` — FOUND
- `fuzz/fuzz_targets/subscription_listen_frames.rs` — FOUND
- `src/client/subscriptions.rs` — FOUND
- commit `e4983a1f` — FOUND
- commit `e37c381a` — FOUND
- commit `9a803e38` — FOUND
