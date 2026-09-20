---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 21
subsystem: testing
tags: [http-09, tripwire, memory-bounds, source-scanning, sse, streamable-http, dos]

requires:
  - phase: 113
    provides: "the capped reads themselves — collect_body_within_cap on both transports (113-20, CR-03), SseParser's unconditional pre-check (113-17), the O(n) take_utf8_prefix (5f045086)"
provides:
  - "tests/v2_bounded_reads_tripwire.rs — the mechanical, enumerable half of HTTP-09"
  - "a structural Limited-in-statement rule that fails when a NEW unbounded whole-body read appears AND when an EXISTING bound is deleted"
  - "runtime src/shared/ discovery, so a new file is in scope without anybody remembering to add it"
  - "a 9-entry accumulation allowlist with individually written, enforced justifications, failing on both addition and deletion"
  - "D-113-Q: a previously-unnamed unbounded reqwest whole-body read in OptimizedSseTransport, found BY the tripwire"
affects: [113-22, verify-phase-113, any future src/shared transport work]

tech-stack:
  added: []
  patterns:
    - "source-scanning integration test: strip comments/literals with a byte-to-line map, then match needles over the stripped text"
    - "structural property check over the enclosing STATEMENT rather than a site counter"
    - "count+justification allowlist keyed by (file, needle) — never by line number"

key-files:
  created:
    - tests/v2_bounded_reads_tripwire.rs
  modified:
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md

key-decisions:
  - "113-21: the whole-body check is STRUCTURAL (Limited::new must appear in the same statement), not a site counter — proven by NC-2, which fails with the site count unchanged"
  - "113-21: the accumulation check is documented in-source as a CHANGE DETECTOR, not a proof of boundedness — no lexical scan can see the drain downstream of an append, so the reasoning lives in written per-entry justifications"
  - "113-21: allowlist keys are (file, needle, count), never line numbers — line numbers churn on unrelated edits and would make the gate a nuisance, while a count change is exactly the reviewable event"
  - "113-21: cfg(test) exclusion is PER-ITEM with brace matching over stripped text, not truncate-at-first-marker — streamable_http_server.rs has two cfg(test) fns at 1094/1191 ahead of mod tests at 3839, and truncating there would drop ~2700 production lines"
  - "113-21: cfg predicates are excluded only when they REQUIRE test (bare `test`, or `test` as a top-level all(...) conjunct); `any(feature = \"fuzzing\", test)` ships and stays in scope"
  - "113-21: FOUND (Rule 2) — the plan's needle families covered hyper/axum/std/tokio but not reqwest; adding .text()/.bytes()/.json() surfaced an unbounded response.text().await in OptimizedSseTransport::connect_sse on the first run"
  - "113-21: that site is ENUMERATED in WHOLE_BODY_ALLOWLIST with an honest 'NOT BOUNDED' justification rather than left invisible, and the exemption list length is pinned at 1 so a second exemption needs a human decision"
  - "113-21: no requirement checkbox flipped — HTTP-09 stays [ ] pending 113-22's O(n) half, and the STATE.md publication gate binds"

patterns-established:
  - "Negative controls run against the PREBUILT test binary: a source-scanning test reads src/ at runtime, so an injected defect need not compile for the control to be valid — which is also what makes NC-5 (an undeclared, never-compiled file) possible"
  - "Anti-vacuity guard alongside every scanning check: pin the known-good site population so an over-stripping scanner fails loudly instead of reporting success over an empty set"

requirements-completed: []

duration: 82min
completed: 2026-07-27
---

# Phase 113 Plan 21: HTTP-09 Bounded-Reads Tripwire Summary

**A source-scanning test that mechanically enumerates every peer-controlled whole-body read and byte accumulation in HTTP-09's scope, fails by `path:line` when a new one appears or an existing bound is deleted — and which found a fourth-round unnamed site (`OptimizedSseTransport`'s unbounded `reqwest` body read) on its first run.**

## Performance

- **Duration:** 82 min
- **Started:** 2026-07-27T06:34Z
- **Completed:** 2026-07-27T07:56Z
- **Tasks:** 3 of 3
- **Files created:** 1 (`tests/v2_bounded_reads_tripwire.rs`, 1120 lines)

## Accomplishments

- **Replaced "the reviewer must notice" with "the suite fails by name."** HTTP-09 reopened three times because each round bounded the sites that round's findings happened to enumerate. The check now enumerates them mechanically, and all five mandated negative controls were executed and produced the expected named failure.
- **Made the check structural, not a counter.** NC-2 deletes `Limited::new(...)` from an existing capped read; the site count is unchanged and the suite still fails, naming `src/shared/streamable_http.rs:545`. A count-based check — the shape every prior round used implicitly — cannot see that.
- **Made coverage automatic.** `src/shared/` is walked with `read_dir` at test time. NC-5 drops a new `.rs` file with no `mod` declaration (so it is never compiled) and the suite fails on it, proving the exact miss-by-omission failure mode that reopened this requirement cannot recur.
- **Found a real, previously-unnamed defect.** See "Discovered defect" below.

## Task Commits

1. **Task 1: Scanner core — runtime discovery, stripping with line mapping, cfg-test exclusion** — `c8b84fca` (test)
2. **Task 2: Structural whole-body-read check** — `0a0cf8af` (test)
3. **Task 3: Accumulation allowlist with justifications + five negative controls** — `3241aee8` (test)

## Files Created/Modified

- `tests/v2_bounded_reads_tripwire.rs` — the tripwire: runtime scope discovery, a comment/literal stripper with a byte-to-line map, per-item `cfg(test)` exclusion, the structural whole-body rule, the justified accumulation allowlist, and 8 unit tests over inline fixtures for the scanner itself. 13 tests total.
- `.planning/phases/.../deferred-items.md` — added **D-113-Q** for the discovered `OptimizedSseTransport` defect.

**No `src/` file was modified.** `git status` over `src/` is clean; all five negative-control edits were reverted with targeted `git checkout -- <file>` / `rm`.

## Discovered defect (Rule 2 deviation)

**D-113-Q — `src/shared/sse_optimized.rs:266` performs an unbounded whole-body read.**

`OptimizedSseTransport::connect_sse` does `response.text().await`. `reqwest::Response::text()` takes no limit argument, so a peer chooses the allocation. This is the same defect class the phase capped three separate times elsewhere; it survived every round because every round's needle set was hyper/axum-shaped and this transport uses `reqwest`.

The plan's `WHOLE_BODY_NEEDLES` list (`.collect().await`, `read_to_end`, `read_to_string`, `body::to_bytes(`) would have shipped a tripwire that was **provably blind to a live site inside HTTP-09's own stated scope**. Shipping that would have been a false green — precisely the failure this plan exists to end. So the reqwest family (`.text().await`, `.bytes().await`, `.json().await`, `.json::<`) was added, and it fired on the first run.

**Handling:** the plan's scope fence is test-only (its verification step requires no `src/` modification), so the site is **enumerated, not hidden**: `WHOLE_BODY_ALLOWLIST` carries it with a written `NOT BOUNDED` justification naming the transport and the owner, `every_whole_body_exemption_carries_a_substantive_justification` pins the list length at 1, and D-113-Q records the fix shape. The tripwire's dead-entry rule does **not** cover the whole-body list, so whoever bounds the read must delete the entry by hand — noted in D-113-Q.

**Deviation classification:** Rule 2 (auto-add missing critical functionality). The needle-family gap was a correctness defect *in the check itself*.

## The allowlist as landed

Counts were **measured** by running the check with a deliberately empty allowlist (verbatim report below), not copied from the plan's `<interfaces>` inventory.

### Whole-body reads

| Path | Needle | Count | Status |
|------|--------|-------|--------|
| `src/shared/http.rs` | `.collect().await` | 1 | **bounded** — `Limited::new(response.into_body(), max_bytes)` |
| `src/shared/streamable_http.rs` | `.collect().await` | 1 | **bounded** — same shape |
| `src/server/streamable_http_server.rs` | `body::to_bytes(` | 1 | **bounded by construction** — `axum::body::to_bytes(body, max_bytes)`, no `usize::MAX` |
| `src/shared/sse_optimized.rs` | `.text().await` | 1 | **UNBOUNDED — allowlisted, D-113-Q** |

Measured `.collect().await` site count outside `cfg(test)` regions: **exactly 2**, both `Limited`-wrapped. Pinned by `the_two_known_capped_whole_body_reads_are_found_and_classified_bounded`.

### Accumulations (9 entries, 25 sites)

| Path | Needle | Count | Justification (abridged — full text in source) |
|------|--------|-------|-----------------------------------------------|
| `src/client/subscriptions.rs` | `extend_from_slice(` | 2 | one hyper frame appended then fully drained by `take_utf8_prefix` each iteration; residual is the ≤3-byte incomplete-character tail; downstream bounded by `SseParser::feed`'s unconditional retained+chunk pre-check (113-17) under the 256 KiB listen ceiling |
| `src/client/subscriptions.rs` | `.extend(` | 2 | collects payloads `drain_sse_payloads` just COMPLETED from one chunk; each pending payload is yielded and removed before the next poll |
| `src/client/subscriptions.rs` | `push_str(` | 1 | `truncate_frame` copies at most `MAX_ECHOED_FRAME` chars into a pre-sized String — it IS a bound, not a consumer of one |
| `src/shared/http.rs` | `extend_from_slice(` | 1 | `connect_sse`'s reader appends one frame, `take_utf8_prefix` drains it same iteration; parser bounded by 113-17 under `DEFAULT_HTTP_SSE_BUFFERED_BYTES` |
| `src/shared/simd_parsing.rs` | `extend_from_slice(` | 2 | `parse_chunk` drains every complete event; residual is one unterminated event. **Carries no ceiling of its own** — exported utility with zero in-crate callers outside its own test module, so nothing feeds it a peer stream today; wiring one up needs the same pre-check `SseParser::feed` carries |
| `src/shared/sse_optimized.rs` | `push_str(` | 1 | `parse_sse_event` builds one event from a `split_to()` slice already cut at the event boundary; `allow(dead_code)`, no caller — the live path in this file is the separately-enumerated reqwest read |
| `src/shared/sse_parser.rs` | `push_str(` | 4 | `take_utf8_prefix` output bounded by the caller's buffer length (each byte appended once, single-exit drain — also what makes it linear); both parser buffers bounded by `feed`'s unconditional pre-check (113-17, T-113-86); `feed_complete_body` states the cap as a caller precondition, discharged by `collect_body_within_cap` (113-20, T-113-84) |
| `src/shared/uri_template.rs` | `push_str(` | 11 | server-authored template + bounded variable map; **not a peer byte stream** — in scope only because it lives under `src/shared/` |
| `src/shared/wasm_http.rs` | `push_str(` | 1 | no incremental reader; the browser fetch API hands over one already-materialised response String and this reassembles the first event's data lines out of that existing allocation |

Every justification is ≥ 40 characters and pairwise distinct, enforced by `every_allowlist_justification_is_substantive`.

### Corrections to the plan's starting inventory

The plan stated these as orientation, not truth; re-measurement moved three of them.

| Plan's `<interfaces>` said | Measured | Why |
|---|---|---|
| `uri_template.rs` 9 `push_str` sites | **11** (adds 480, 497) | plan enumerated a partial list |
| `streamable_http_server.rs:4945` is a production accumulation | **excluded** | line 4945 sits inside `#[cfg(test)] mod tests` (3839 → EOF, the file's last item); the per-item `cfg` rule correctly removes it. This file therefore contributes **zero** accumulation sites |
| whole-body needles cover the scope | **they did not** | `sse_optimized.rs:266` (D-113-Q) |

## Negative controls — verbatim output

All five were executed against the **prebuilt** test binary (`target/debug/deps/v2_bounded_reads_tripwire-2ee7c1b44982baa8`). The check reads `src/` from disk at runtime, so an injected defect need not compile for the control to be valid — which is also the only way NC-5 (a file with no `mod` declaration, hence never compiled) is possible at all. Baseline before each control: `13 passed; 0 failed`.

### NC-1 — new unbounded `.collect().await` in a production fn (`src/shared/http.rs`)

Injected `let _probe = response.into_body().collect().await;` into `HttpTransport::send_request`.

```
---- no_unbounded_whole_body_read_over_peer_supplied_bytes stdout ----

thread 'no_unbounded_whole_body_read_over_peer_supplied_bytes' (57642437) panicked at tests/v2_bounded_reads_tripwire.rs:659:5:
HTTP-09: unbounded whole-body read(s) over peer-supplied bytes:
  src/shared/http.rs:453 — unbounded `.collect().await`
    statement: ...let_probe=response.into_body()

Required action: wrap the read in `http_body_util::Limited` with the transport's configured cap, exactly as `collect_body_within_cap` does in src/shared/http.rs and src/shared/streamable_http.rs. If this site genuinely cannot be bounded, add a WHOLE_BODY_ALLOWLIST entry with a written justification and get it reviewed. Deleting the needle is not a fix.
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- the_two_known_capped_whole_body_reads_are_found_and_classified_bounded stdout ----

thread 'the_two_known_capped_whole_body_reads_are_found_and_classified_bounded' (57642446) panicked at tests/v2_bounded_reads_tripwire.rs:683:13:
src/shared/http.rs:453 is a shipped whole-body read that is NOT Limited-wrapped


failures:
    no_unbounded_whole_body_read_over_peer_supplied_bytes
    the_two_known_capped_whole_body_reads_are_found_and_classified_bounded

test result: FAILED. 11 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

Reverted with `git checkout -- src/shared/http.rs` → `13 passed; 0 failed`.

### NC-2 — existing `Limited` bound deleted, site COUNT unchanged (`src/shared/streamable_http.rs`)

Replaced `Limited::new(response.into_body(), max_bytes).collect().await` with `response.into_body().collect().await` in `collect_body_within_cap`. The number of `.collect().await` sites in scope is still 2.

```
---- no_unbounded_whole_body_read_over_peer_supplied_bytes stdout ----

thread 'no_unbounded_whole_body_read_over_peer_supplied_bytes' (57643321) panicked at tests/v2_bounded_reads_tripwire.rs:659:5:
HTTP-09: unbounded whole-body read(s) over peer-supplied bytes:
  src/shared/streamable_http.rs:545 — unbounded `.collect().await`
    statement: ...matchresponse.into_body()

Required action: wrap the read in `http_body_util::Limited` with the transport's configured cap, exactly as `collect_body_within_cap` does in src/shared/http.rs and src/shared/streamable_http.rs. If this site genuinely cannot be bounded, add a WHOLE_BODY_ALLOWLIST entry with a written justification and get it reviewed. Deleting the needle is not a fix.
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- the_two_known_capped_whole_body_reads_are_found_and_classified_bounded stdout ----

thread 'the_two_known_capped_whole_body_reads_are_found_and_classified_bounded' (57643330) panicked at tests/v2_bounded_reads_tripwire.rs:683:13:
src/shared/streamable_http.rs:545 is a shipped whole-body read that is NOT Limited-wrapped


failures:
    no_unbounded_whole_body_read_over_peer_supplied_bytes
    the_two_known_capped_whole_body_reads_are_found_and_classified_bounded

test result: FAILED. 11 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

**This is the control that distinguishes this design from a counter.** The anti-vacuity test failed on its `Limited`-wrapped assertion, which runs *before* its count assertion — so the count assertion was never reached, confirming the population was still 2. Reverted → `13 passed; 0 failed`.

### NC-3 — new accumulation site (`src/client/subscriptions.rs`)

Injected `probe.extend_from_slice(b"nc3");` into `open_event_stream`'s production path.

```
---- every_peer_byte_accumulation_is_reviewed stdout ----

thread 'every_peer_byte_accumulation_is_reviewed' (57644122) panicked at tests/v2_bounded_reads_tripwire.rs:949:5:
HTTP-09: the reviewed accumulation population changed:
  COUNT ROSE: src/client/subscriptions.rs `extend_from_slice(` — allowlisted 2, observed 3 at line(s) [136, 252, 708]
    One of those lines is new. Raising the number to match reality WITHOUT a justification is the failure mode this test exists to prevent.

This check is a CHANGE DETECTOR, not a proof of boundedness — it exists so that a new append over peer-supplied bytes cannot enter this scope without somebody writing down what bounds it.
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    every_peer_byte_accumulation_is_reviewed

test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

Names the file, the needle, allowlisted-vs-observed counts, and **every** candidate line so the new one is findable. Reverted → `13 passed; 0 failed`.

### NC-4 — allowlisted site deleted (`src/shared/uri_template.rs`)

Rewrote one `pattern.push_str("([^/]+)")` as `write!(pattern, "([^/]+)").unwrap()`, taking the count from 11 to 10.

```
---- every_peer_byte_accumulation_is_reviewed stdout ----

thread 'every_peer_byte_accumulation_is_reviewed' (57644935) panicked at tests/v2_bounded_reads_tripwire.rs:949:5:
HTTP-09: the reviewed accumulation population changed:
  DEAD allowlist entry: src/shared/uri_template.rs `push_str(` — allowlisted 11, observed 10.
    Delete the entry (or lower its count). A site was removed and its justification was not.

This check is a CHANGE DETECTOR, not a proof of boundedness — it exists so that a new append over peer-supplied bytes cannot enter this scope without somebody writing down what bounds it.
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    every_peer_byte_accumulation_is_reviewed

test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

This is the anti-rot half: a stale entry is how a real new site could hide under a number set for a site since deleted. Reverted → `13 passed; 0 failed`.

### NC-5 — brand-new, never-compiled file under `src/shared/`

Created `src/shared/zz_tripwire_probe.rs` containing an unbounded `.collect().await` with **no `mod` declaration anywhere**, so it is not part of the crate and the build is undisturbed.

```
---- no_unbounded_whole_body_read_over_peer_supplied_bytes stdout ----

thread 'no_unbounded_whole_body_read_over_peer_supplied_bytes' (57652971) panicked at tests/v2_bounded_reads_tripwire.rs:659:5:
HTTP-09: unbounded whole-body read(s) over peer-supplied bytes:
  src/shared/zz_tripwire_probe.rs:10 — unbounded `.collect().await`
    statement: ...letcollected=body

Required action: wrap the read in `http_body_util::Limited` with the transport's configured cap, exactly as `collect_body_within_cap` does in src/shared/http.rs and src/shared/streamable_http.rs. If this site genuinely cannot be bounded, add a WHOLE_BODY_ALLOWLIST entry with a written justification and get it reviewed. Deleting the needle is not a fix.
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- the_two_known_capped_whole_body_reads_are_found_and_classified_bounded stdout ----

thread 'the_two_known_capped_whole_body_reads_are_found_and_classified_bounded' (57653050) panicked at tests/v2_bounded_reads_tripwire.rs:683:13:
src/shared/zz_tripwire_probe.rs:10 is a shipped whole-body read that is NOT Limited-wrapped
```

Note the probe file's own module doc mentions `mod` and `collect` in prose; the stripper removed it, and only the real code site fired. `rm src/shared/zz_tripwire_probe.rs` → `13 passed; 0 failed`, `git status src/` clean.

### Bonus control — the measured inventory (empty allowlist)

The run that produced the allowlist counts, recorded because it is also the report a future contributor will see when they add a site:

```
HTTP-09: the reviewed accumulation population changed:
  NEW accumulation site(s): src/client/subscriptions.rs `.extend(` at line(s) [254, 708]
  NEW accumulation site(s): src/client/subscriptions.rs `extend_from_slice(` at line(s) [250, 706]
  NEW accumulation site(s): src/client/subscriptions.rs `push_str(` at line(s) [339]
  NEW accumulation site(s): src/shared/http.rs `extend_from_slice(` at line(s) [375]
  NEW accumulation site(s): src/shared/simd_parsing.rs `extend_from_slice(` at line(s) [301, 401]
  NEW accumulation site(s): src/shared/sse_optimized.rs `push_str(` at line(s) [302]
  NEW accumulation site(s): src/shared/sse_parser.rs `push_str(` at line(s) [185, 193, 490, 561]
  NEW accumulation site(s): src/shared/uri_template.rs `push_str(` at line(s) [281, 443, 446, 449, 452, 455, 471, 474, 477, 480, 497]
  NEW accumulation site(s): src/shared/wasm_http.rs `push_str(` at line(s) [117]
```

(`Bound it, or add an ALLOWLIST entry naming the mechanism that bounds it.` follows each line; elided here for width.)

## What each check proves — and what it does not

Stated in the file's own module rustdoc so the next reader does not have to infer it:

- **Whole-body reads: a structural property check.** Every whole-body read in scope must be bounded *within its own statement*. Fails on addition (NC-1, NC-5) and on bound removal (NC-2).
- **Accumulations: a change detector, not a proof of boundedness.** Whether an append is bounded depends on the drain downstream of it, which no lexical scan can see. The reasoning therefore lives in the written justifications; the test only detects that the population moved, in either direction (NC-3, NC-4).
- **The scanner cannot pass vacuously.** 8 unit tests over inline fixtures cover comment/literal stripping (line, doc, inner-doc, nested block, string, raw string, byte-raw string), lifetime-vs-char-literal disambiguation, rustfmt-broken chain matching with first-line attribution, the four `cfg` predicate shapes, per-item `cfg(test)` exclusion (including the "production code after a `cfg(test)` fn is still scanned" case), body-less `cfg(test)` items, and runtime scope discovery. Plus a positive anti-vacuity assertion pinning the two known capped sites.

## Verification

| Check | Result |
|-------|--------|
| `cargo nextest run --features full --test v2_bounded_reads_tripwire` | **13 tests run: 13 passed, 0 skipped** |
| `cargo clippy --features full --lib --tests` with the full house lint set (`-D clippy::all -W pedantic -W nursery -W cargo`, `RUSTFLAGS=-D warnings`) | **exit 0, no issues** |
| `cargo fmt --all -- --check` | clean |
| Five negative controls | **all five produced the expected named failure; all five reverted** |
| `make quality-gate` (background job, log polled) | **exit 0** (`QUALITY_GATE_EXIT=0`) |
| `cargo nextest run --features full` (whole suite, unfiltered) | **Summary [26.355s] 2201 tests run: 2201 passed (1 leaky), 2 skipped** |
| `git status src/` | clean — no `src/` file modified |
| `.planning/REQUIREMENTS.md` | **untouched**; no checkbox flipped |

**Note on the `make quality-gate` log.** The log captured 237 `Running tests/...` lines and **zero** `test result:` lines, and make's `@echo` banners appear out of order. This is the known rtk shell-proxy output filtering recorded in prior phases, not a test-harness anomaly. The exit code is unaffected and authoritative (`make` aborts on the first failing recipe), and the full-suite total above was captured separately through the absolute `cargo` binary to bypass the proxy.

## Deviations from Plan

### Auto-fixed / auto-extended

**1. [Rule 2 — Missing critical functionality] `WHOLE_BODY_NEEDLES` extended with the reqwest family**

- **Found during:** Task 3
- **Issue:** The plan's four needle families cover hyper, axum, std and tokio but not `reqwest`, which is the HTTP client `OptimizedSseTransport` uses. The tripwire would have shipped provably blind to `src/shared/sse_optimized.rs:266` — a live unbounded whole-body read inside HTTP-09's own stated scope. A tripwire that is known to miss a site converts an open gap into a false green, which is worse than no tripwire.
- **Fix:** Added `.text().await`, `.bytes().await`, `.json().await`, `.json::<` with a `bound_in_scope` rule of "no bounded form exists — every occurrence needs a reviewed exemption". The site is enumerated in `WHOLE_BODY_ALLOWLIST` with an honest `NOT BOUNDED` justification; the list length is pinned at 1.
- **Files modified:** `tests/v2_bounded_reads_tripwire.rs`, `.planning/phases/.../deferred-items.md` (D-113-Q)
- **Commit:** `3241aee8`

**2. [Plan expectation corrected] `WHOLE_BODY_ALLOWLIST` is not empty at HEAD**

- The plan expected it empty. It holds exactly one entry, because of finding 1. This is recorded rather than quietly satisfied: the `len() == 1` assertion makes any growth a decision somebody has to make on the record, and the healthy direction is stated in the constant's rustdoc as "should shrink, never grow".

**3. [Measurement correction] Allowlist counts differ from the plan's `<interfaces>` inventory**

- `uri_template.rs` has 11 `push_str` sites, not 9; `streamable_http_server.rs:4945` is inside `#[cfg(test)] mod tests` and contributes nothing. Both are consistent with the plan's own instruction to re-measure at execution time. Details in "Corrections to the plan's starting inventory".

**4. [Method] Negative controls run against the prebuilt test binary**

- The plan did not specify how. Because the check reads `src/` from disk at runtime, injected defects need not compile — which is what makes NC-5 (an undeclared, never-compiled file) a valid control at all. Baseline green was re-confirmed before and after every control.

### Not deviations

- No authentication gates were encountered.
- No architectural (Rule 4) decisions arose.

## Known Stubs

None. Every constant, allowlist and check in the file is live and exercised by a test; the `WHOLE_BODY_ALLOWLIST` mechanism is exercised by a real (non-placeholder) entry.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: denial-of-service | `src/shared/sse_optimized.rs:266` | `OptimizedSseTransport::connect_sse` buffers a peer-chosen SSE body whole via `reqwest::Response::text()`, which has no limit argument. Not on the v2 streamable-HTTP path and with no in-crate consumer, but exported from `shared::` and so reachable in a shipped build. Recorded as D-113-Q; enumerated in `WHOLE_BODY_ALLOWLIST` rather than left unnamed. |

Threat register dispositions from the plan, all `mitigate`, all discharged:

| Threat ID | Mitigation as landed | Proof |
|-----------|----------------------|-------|
| T-113-95 | structural `Limited`-in-statement check fails by `path:line` | NC-1 |
| T-113-96 | check is structural, not a counter | NC-2 (site count unchanged, still fails) |
| T-113-97 | scanner unit tests + positive pin on the two capped sites | 8 fixture tests + `the_two_known_capped_..._bounded` |
| T-113-98 | runtime `read_dir` discovery | NC-5 (undeclared file still scanned) |
| T-113-99 | ≥40-char unique justification per entry; dead entries fail | NC-4 + `every_allowlist_justification_is_substantive` |
| T-113-100 | comments and literal contents stripped before matching | `line_comments_doc_comments_and_block_comments_are_not_scanned`, `string_and_raw_string_contents_are_not_scanned` |

## Requirement status — no checkbox flipped

**HTTP-09 remains `[ ]`.** This plan lands the *enumeration* half; the O(n) clause is 113-22. The STATE.md publication gate independently forbids flipping HTTP-01..09 / CLNT-01/02/05 for this whole round. `.planning/REQUIREMENTS.md` was not opened for edit and `requirements mark-complete` was deliberately not run.

## For the next agent

- **113-22 owns the O(n) half.** `take_utf8_prefix`'s single-exit drain is already linear (`5f045086`); the remaining work is the mechanical check, not the fix.
- **Do not "fix" a tripwire failure by raising a number.** Every failure message says so explicitly. The three failure modes are: unreviewed site, count rose, dead entry.
- **D-113-Q needs an owner.** Bounding `OptimizedSseTransport` means streaming via `bytes_stream()` with a running total (reqwest has no `http_body_util::Limited` equivalent) — or retiring the transport. Deleting the `WHOLE_BODY_ALLOWLIST` entry is part of that fix; the dead-entry rule does not cover that list.
- **Adding a file to `src/shared/` puts it in scope automatically.** That is by design and it is proven (NC-5). If your new file needs a peer-byte read, bound it before you commit.

## Self-Check: PASSED

- `tests/v2_bounded_reads_tripwire.rs` — FOUND
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-21-SUMMARY.md` — FOUND
- `src/shared/zz_tripwire_probe.rs` (NC-5 probe) — correctly ABSENT
- Commits `c8b84fca`, `0a0cf8af`, `3241aee8`, `df95796d` — all FOUND
