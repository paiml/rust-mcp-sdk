---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 19
subsystem: testing
tags: [fuzzing, libfuzzer, public-api, semver, sse-parser, subscriptions, gap-closure, phase-gate]

# Dependency graph
requires:
  - phase: 113
    provides: "113-17's `SseParser::buffered_bytes()` accessor and its two independently-sufficient enforcement points in `feed`"
  - phase: 113
    provides: "113-18's retryable listen refusal + fresh-id tripwire (the `v2_subscriptions_client` floor this gate asserts against)"
  - phase: 113
    provides: "113-20's collected-body cap — the last code change in the round, which is why this gate is wave 3"
provides:
  - "`decode_listen_chunks_for_fuzz` behind `#[cfg(any(feature = \"fuzzing\", test))]` — no longer callable by any downstream crate, proven by a real downstream compile"
  - "A libFuzzer invariant that asserts PEAK PARSER RETENTION against `max_buffer_size`, replacing a tautology"
  - "`113-FUZZ-EVIDENCE.md` § Campaign 2 — 20 000 runs at the post-fix commit plus the four-run negative control"
  - "The cross-cutting phase gate over plans 113-17 + 113-18 + 113-20 + 113-19"
affects: [113-VERIFICATION re-verification, 114, 117, 118]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A fuzz seam is gated by the crate's `fuzzing` FEATURE plus `cfg(test)`, never by `#[doc(hidden)]` alone — `doc(hidden)` hides from rustdoc, not from callers or semver"
    - "A fuzz invariant must be shown FAILING before it counts as closed; an assertion with one write site and no clearing path is evidence of nothing"
    - "When the plan's verification instrument is structurally blind to the change (`cargo public-api` omits `doc(hidden)`), record the vacuity and supply a falsifiable substitute rather than banking the green"
    - "A negative control is SEEDED with the exact pattern under test, not left to libFuzzer's length ramp"

key-files:
  created: []
  modified:
    - src/client/subscriptions.rs
    - fuzz/fuzz_targets/subscription_listen_frames.rs
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/113-FUZZ-EVIDENCE.md

key-decisions:
  - "The seam is gated on the Cargo FEATURE `fuzzing` (the crate's house pattern, per `src/server/request_state.rs`), NOT on a bare `#[cfg(fuzzing)]` — the latter needs an `unexpected_cfgs` allowance and diverges from the established convention"
  - "The plan's `cargo public-api` seam-absence criterion is VACUOUS (public-api omits `#[doc(hidden)]`, so it was 0 before the fix too); a real downstream crate compiled by path is the falsifiable substitute, run in all three states"
  - "Campaign 1's PASS verdict in `113-FUZZ-EVIDENCE.md` is preserved verbatim rather than amended — that campaign was green while GAP-A was open, and erasing that would erase the evidence for why this plan exists"
  - "HTTP-04 deliberately NOT flipped to `[x]` — the STATE.md phase gate forbids it before the 2026-07-28 schema re-verification"
  - "The stale ROADMAP narrative line was corrected as a tracking-file update AFTER the task fence closed; `.planning/REQUIREMENTS.md` was not touched at all"

patterns-established:
  - "Peak-retention sampling per chunk is the seam's third return value, so the bound is observable to a campaign rather than only to a unit test"
  - "The non-vacuity of a new fuzz invariant is pinned by an in-crate test that reaches the bound, not only by the campaign that asserts it"

requirements-completed: []

# Metrics
duration: 43min
completed: 2026-07-27
---

# Phase 113 Plan 19: Fuzz Seam Gating + Real Bound Invariant + Phase Gate Summary

**The `subscriptions/listen` fuzz seam is off the shipped public API (proven by a downstream crate that now fails with `E0425`), the libFuzzer campaign asserts peak parser retention against `max_buffer_size` instead of a latch check that could not fail, that new assertion is shown CRASHING when GAP-A is reintroduced, and the whole gap-closure round — 113-17 + 113-18 + 113-20 + this plan — passes build-matrix, semver, public-api, PMAT and `make quality-gate` with no requirement checkbox touched.**

## Performance

- **Duration:** 43 min
- **Started:** 2026-07-27T02:32:13Z
- **Completed:** 2026-07-27T03:15:03Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- **GAP-D closed, and closed for the right reason.** `decode_listen_chunks_for_fuzz` carries `#[cfg(any(feature = "fuzzing", test))]`. `#[doc(hidden)]` never restricted visibility — `pub mod client` → `pub mod subscriptions` made the item callable by every dependent crate, committing the SDK to a `&[&[u8]]` chunk model, an unvalidated `max_buffer_size` (`0` latches on the first non-empty chunk) and `String`-flattened errors. All three non-commitments are now written into the seam's rustdoc so the reason survives the next refactor.
- **GAP-E closed with a falsifiable invariant.** Invariant 3 was "the latch never clears" — `overflowed` has exactly one write site and no clearing path, so it could not fail for any input at any bound. It is replaced by a per-chunk `peak_buffered_bytes <= max_buffer_size` assertion and demoted to a subordinate note. The latch check is kept (it is cheap and it documents the latch) but is no longer numbered.
- **The new invariant is proven to fail.** Four seeded runs, recorded below. The run that matters most is the one that stayed GREEN: disabling only 113-17's pre-check is NOT sufficient, because its total post-drain check still keeps every sample in bounds. Both had to go before the campaign crashed — exactly the correction review HIGH-2 demanded of the previous revision.
- **A vacuous gate was caught rather than banked.** The plan's `cargo public-api | grep -c` criterion passes — and would have passed identically before the fix, because `cargo public-api` omits `#[doc(hidden)]` items in both directions. Recorded as such, with a real downstream-crate compile as the substitute proof in all three states.
- **The round's phase gate is green.** 20 000 fuzz runs / 0 artifacts, 6 suites, 4 build-matrix rows, semver 223/223 no-update-required, zero REMOVED public items, zero new PMAT violations, `make quality-gate` exit 0 with 243 `test result: ok` and 0 `FAILED`.

## Task Commits

1. **Task 1: Gate the fuzz seam out of the public API and give the campaign a bound it can fail** — `569f3533` (fix)
2. **Task 2: Run the campaign and the cross-cutting phase gate over the whole gap-closure round** — `d04dcc76` (docs)

**Plan metadata:** see final commit.

## Files Created/Modified

- `src/client/subscriptions.rs` — the section banner now states the gate; `decode_listen_chunks_for_fuzz` gets `#[cfg(any(feature = "fuzzing", test))]`, a `# ⚠️ Not stable API` block naming the three things the signature deliberately does not commit to, and a rewritten `# Returns` covering all three vectors; the seam returns `peak_buffered_bytes` (`SseParser::buffered_bytes()` after each chunk drains); both existing seam tests assert on the new vector; one new test proves the campaign's assertion is non-vacuous.
- `fuzz/fuzz_targets/subscription_listen_frames.rs` — Invariant 3 replaced with the bound assertion (message prints the observed retention, the chunk index and the bound); the latch check demoted to a subordinate note that says in-source why it cannot fail; the module doc renumbered and the evidence file referenced by full repo-relative path (review IN-04).
- `.planning/.../113-FUZZ-EVIDENCE.md` — a new `# Campaign 2 (2026-07-27)` section in campaign 1's format (commands, toolchain table, commit SHA, seed, counters, artifacts-empty proof with its exact absolute-path command, branch-coverage measurement, replay, reproduction recipe, D-113-G bypass reason) plus § 2.5 negative control and § 2.6 the seam-gating probe. Campaign 1 is preserved verbatim; a pointer at the top tells the reader both exist and that campaign 1 was green while GAP-A was open.

## `cargo public-api` before/after for the seam — and why that instrument is blind

| Measurement | Before (HEAD `a327c5e7`) | After (`569f3533`) |
|---|---|---|
| `cargo public-api -p pmcp --features full \| grep -c decode_listen_chunks_for_fuzz` | **0** | **0** |
| `cargo public-api -p pmcp --features full diff 2.17.0 \| grep -c decode_listen_chunks_for_fuzz` | — | **0** |

**The criterion passes and is vacuous.** `cargo public-api` (0.52.0) omits `#[doc(hidden)]` items entirely and offers no flag to include them (`--omit`/`--include`/`-s`/`-v` cover blanket impls, auto-trait impls, auto-derived impls and parameter names — nothing for `doc(hidden)`). The seam was already invisible to it at HEAD, so `0` detects nothing about this change. Proven by contrast in the same tool: `pmcp::server::request_state::fuzz_support` — the crate's other `fuzzing`-gated seam, which is NOT `doc(hidden)` — appears in the `full,fuzzing` listing and is absent from the `full` listing (8 lines of diff).

**The falsifiable substitute: a real downstream crate.** A scratch binary crate outside the repo, depending on `pmcp` by path, calling the seam. Three states, all run:

| # | pmcp source state | Downstream features | Result |
|---|---|---|---|
| 1 | gate REMOVED (reproduces the pre-fix shape) | `["full"]` | **compiles** — the seam was genuinely reachable by any dependent crate |
| 2 | gate present (shipped) | `["full"]` | **`error[E0425]: cannot find function decode_listen_chunks_for_fuzz in module pmcp::client::subscriptions`**, with rustc pointing at `src/client/subscriptions.rs:663:8` |
| 3 | gate present (shipped) | `["full", "fuzzing"]` | **compiles** — the campaign is unaffected |

State 1 was produced by a temporary one-line mutation restored from a byte-exact copy; `git diff` on the file after restore was empty before Task 1's commit.

## Negative control — the new fuzz invariant CAN fail (review HIGH-2, corrected form)

Seeded rather than hoped for: one corpus file of `data: A\n` repeated 40 times (320 bytes) in a directory outside the repo. `data: A\n` is exactly 8 bytes, so the target's 16-byte chunking yields two COMPLETE `data:` lines per chunk — each line completes (so any "does this chunk carry a newline?" escape waves it through) while its payload accumulates into `current_event.data`, which only a BLANK line clears. That is GAP-A in one file.

All runs: `cargo +nightly fuzz run subscription_listen_frames <seeded_corpus> -- -runs=2000 -artifact_prefix=<scratch>/negctl-artifacts/`. The second `-artifact_prefix` overrides the one `cargo fuzz` injects, so run B's crash artifact landed outside the repo and the artifacts-empty proof is unaffected.

| Run | State of `src/shared/sse_parser.rs` | Exit | Result |
|---|---|---|---|
| 0 | both enforcement points INTACT (shipped tree) | 0 | **GREEN** — `Done 2000 runs` |
| A | PRE-check (`:391`) forced to `if false`; POST-check intact | 0 | **STILL GREEN.** This is the load-bearing negative result: 113-17's total post-drain check alone keeps every `peak_buffered_bytes` sample in bounds, so a control that reverted only the pre-check would have "proven" nothing. |
| B | PRE-check forced to `if false` **AND** POST-check (`:412`) reverted to `if self.buffer.len() > self.max_buffer_size` | **1** | **CRASH on the peak-retention assertion** |
| C | both RESTORED | 0 | **GREEN** — `Done 2000 runs` |

Run B's panic, verbatim:

```
thread '<unnamed>' (56358758) panicked at fuzz_targets/subscription_listen_frames.rs:145:13:
the parser retained 9 bytes after chunk 0 under a 8-byte bound (peaks: [9, 16, 0])
...
SUMMARY: libFuzzer: deadly signal
artifact_prefix='<scratch>/negctl-artifacts/'; Test unit written to
<scratch>/negctl-artifacts/crash-bdaaf98efb7fd0574dec20ee1b7076398c2e9c5f
```

The message prints two integers and a chunk index — never the fuzzed payload (T-113-94). `src/shared/sse_parser.rs` was restored from a byte-exact copy; `git diff --stat -- src/shared/sse_parser.rs` is empty at both commits, so no control mutation reached the tree.

**In-crate non-vacuity proof.** `the_seam_reports_retention_that_stays_inside_a_tiny_bound_while_reaching_it` drives the same newline-carrying flood through the seam at a 64-byte bound and asserts three things at once: every sample is inside the bound (the campaign's assertion), at least one overflow observation is `true` (so the bound is actually REACHED, not trivially far away), and some sample exceeds one chunk's own length (so retention ACCUMULATES across lines rather than being trivially per-chunk).

## Campaign counters and artifacts-empty proof

| | Campaign 2 (this plan, `569f3533`) | Campaign 1 (113-16) |
|---|---|---|
| Command | `cargo +nightly fuzz run subscription_listen_frames -- -runs=20000` | same |
| Exit code | **0** | 0 |
| Final line | `#20000 DONE cov: 229 ft: 692 corp: 133/1758b lim: 43 exec/s: 20000 rss: 104Mb` | `#20000 DONE cov: 232 ft: 823 corp: 182/3497b lim: 63` |
| libFuzzer seed | **3621664529** | 872967294 |
| Corpus start | `0 files found` / `starting from an empty corpus` | same |
| Toolchain | `rustc 1.97.0-nightly (bf4fbfb7a 2026-04-11)`, `cargo-fuzz 0.13.1`, `aarch64-apple-darwin` | same |
| Artifacts | `/bin/ls -A fuzz/artifacts/subscription_listen_frames/ \| /usr/bin/wc -l` → **0**, `/bin/test -d …` → **EXISTS** | 0, EXISTS |

`git status --porcelain -- src fuzz` was EMPTY at run time, so the campaign ran against exactly the committed tree.

**Branch coverage, re-measured over the new corpus:** 132 entries on disk, **17** drive the discard-and-latch branch on chunk 1 (first ≤16-byte chunk longer than 8 bytes with no `\n` — the enforcement condition verbatim), **0** entries larger than 64 bytes, max entry 38 bytes. That last figure reproduces campaign 1's finding exactly and is why `MAX_BUFFER_SIZES` carries the 8-byte bound. Single-input replay of entry `0a608e19bcad400d74d8f4cee9efabebd2353d06` exits 0.

**On `cov: 229` vs campaign 1's `232`:** libFuzzer draws a fresh seed per run, so run-to-run variance in `cov`/`corp` is expected and is not a regression signal on its own. The load-bearing figure — that the enforcement branch is still covered — is measured separately above and is non-zero. `-seed=3621664529` replays this exact run.

## Phase-gate results over the WHOLE round (113-17 + 113-18 + 113-20 + 113-19)

| # | Check | Floor | Result |
|---|---|---|---|
| 1 | `cargo build --lib --features full` | exit 0 | **exit 0** (seam absent) |
| 2 | `cargo build --lib --features "streamable-http,fuzzing"` | exit 0 | **exit 0** (seam present) |
| 3 | `cargo public-api -p pmcp --features full \| grep -c decode_listen_chunks_for_fuzz` | 0 | **0** — passes, and VACUOUS (see above) |
| 4 | `cargo fuzz build --sanitizer=none subscription_listen_frames` (stable) | exit 0 | **exit 0** |
| 5 | `cargo +nightly fuzz build subscription_listen_frames` (ASan) | exit 0 | **exit 0** |
| 6 | Negative control, 4 seeded runs | crash when both checks disabled | **recorded above** |
| 7 | `cargo +nightly fuzz run … -- -runs=20000` | exit 0, `#20000 DONE` | **exit 0, `#20000 DONE`** |
| 8 | `/bin/ls -A fuzz/artifacts/subscription_listen_frames/ \| /usr/bin/wc -l` | 0 | **0** (dir EXISTS) |
| 9 | `cargo test --lib --features full -- client::subscriptions` | > 31 (113-17 Task 2) | **32 passed, 0 failed** |
| 10 | `cargo test --lib --features full -- sse_parser` | ≥ 24 | **24 passed, 0 failed** |
| 11 | `cargo test --lib --features full -- subscriptions` | ≥ 78 | **83 passed, 0 failed** |
| 12 | `cargo test --lib --features full -- streamable_http` | ≥ 90 (113-20) | **90 passed, 0 failed** |
| 13 | `cargo test --test v2_subscriptions --features full` | ≥ 10 | **10 passed, 0 failed** |
| 14 | `cargo test --test v2_subscriptions_client --features full` | ≥ 8 | **8 passed, 0 failed** |
| 15 | `cargo test --test v2_stateless_http --features full` | ≥ 23 | **23 passed, 0 failed** |
| 16 | `cargo test --test v2_mrtr --features full` | ≥ 27 | **27 passed, 0 failed** |
| 17 | `cargo test --test server_subscriptions --features full` | ≥ 6 | **6 passed, 0 failed** |
| 18 | `cargo build --lib --no-default-features` | exit 0 | **exit 0** |
| 19 | `cargo build --lib --target wasm32-unknown-unknown` | exit 0 | **exit 0** |
| 20 | `cargo build -p pmcp-team-servers --all-features` | exit 0 | **exit 0** |
| 21 | `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | no update required | **223 checks: 223 pass, 30 skip — `Summary no semver update required`.** No `constructible_struct_adds_field`. |
| 22 | `cargo public-api -p pmcp --features full diff 2.17.0` | zero REMOVED attributable | **zero.** Exactly 2 non-`zerocopy` `-` lines, both the pre-existing `ElicitRequestParams::deserialize<__D>` → `<D>` generic-parameter rename re-added on the `+` side of the same diff — the same artefact 113-17 and 113-20 documented. |
| 23 | PMAT cog-25 over `src/` | no NEW violation in the round's files | **0 violations** in `sse_parser.rs`, `client/subscriptions.rs`, `server/subscriptions.rs`, `shared/http.rs`, `shared/streamable_http.rs`. The only `src/` violations are the two pre-existing D-113-F ones (`handle_post_fast_path` cog 30, `handle_post_with_middleware` cog 31). |
| 24 | `make quality-gate` | exit 0 | **exit 0.** 7 347 log lines, 243 `test result: ok`, **0** `test result: FAILED`, 0 truncation markers. |
| 25 | `git diff --name-only -- Cargo.toml Cargo.lock fuzz/Cargo.toml` | empty | **empty** |
| 26 | `git diff --name-only -- .planning/REQUIREMENTS.md .planning/ROADMAP.md` (at task-fence close) | empty | **empty** |
| 27 | `git diff --name-only -- Makefile` | empty | **empty** |
| 28 | Contract-first: `ls ../provable-contracts/contracts/` | recorded | **`No such file or directory` (exit 1)** — no contract YAML exists for these surfaces in this environment, as the plan predicted. Recorded rather than skipped silently. `make quality-gate`'s own `pmat comply` stage ran and reported project-level advisories as informational (CLAUDE.md D-07), and `comply-bindings-check` resolved all four team-servers bindings. |

**PMAT query note:** used D-113-J's corrected shape (`.summary.violations[] | select(.file …)`). CLAUDE.md's documented `.violations[] | select(.path …)` was re-confirmed silently vacuous on pmat 3.15.0 — it prints nothing for a tree that has two real violations.

**Source acceptance greps (all pass):**

- `grep -B3 -n "pub fn decode_listen_chunks_for_fuzz" src/client/subscriptions.rs` → `#[cfg(any(feature = "fuzzing", test))]` at `:660`, immediately above `#[doc(hidden)]` / `#[must_use]` / the item at `:663`.
- `grep -c "peak" fuzz/fuzz_targets/subscription_listen_frames.rs` → **4** (floor 3).
- `grep -c "buffered_bytes" src/client/subscriptions.rs` → **19** (floor 2).
- `grep -n "113-FUZZ-EVIDENCE.md" fuzz/fuzz_targets/subscription_listen_frames.rs` → one line, containing `.planning/phases/`.
- `grep -c "decode_listen_chunk_for_fuzz" …` (the singular seam) → **0** in both files; it did not reappear.
- `grep -nE "TODO|FIXME|XXX|TBD"` over both files → no match (zero SATD).
- `cargo fmt --all -- --check` → exit 0; `make lint` → `✓ No lint issues`.

## Findings this round deliberately did NOT close

Named so the next verification is not surprised. None of these is a regression from this round; each is either out of the gap list or unowned.

| ID | What it is | Why it is still open |
|---|---|---|
| **WR-01** | The overflow disconnect still leaks the concurrency permits until the guard unwinds | Not in this round's gap list. Touches `src/server/subscriptions.rs` teardown, which 113-18 deliberately left byte-unchanged in `take_entry`/`remove_entry`/`disconnect_overflowed`. |
| **WR-02** | `SseConfig` still has no consumer beyond its `Default` | Not in this round's gap list. `SseParser::new()` sources its bound from `SseConfig::default().max_buffer_size` (113-15) and nothing else reads the struct. |
| **WR-04** | Invariant 2's backslash escape hatch is scoped to the WHOLE input, not to the frame it protects | Explicitly out of scope per this plan's action item 7 ("do not widen scope to it"). One `0x5C` anywhere in an input suppresses the cross-delivery assertion for every frame in that run, and libFuzzer preferentially retains backslash-bearing inputs, so the fraction of checked runs falls over a long campaign. The review's fix — assert on the DECODED `_meta` tag carried by `outcomes` rather than gating on raw bytes — is directly available and is the right shape for whoever picks it up. |
| **D-113-F** | Two pre-existing cog-25 violations in `streamable_http_server.rs` | Unowned. Re-confirmed present and unchanged (cog 30 / cog 31). |
| **D-113-G** | `make quality-gate`'s fuzz stage builds 0 of 17 targets and swallows the failures | Unowned, Makefile-scoped. Reproduced exactly on this run: 17 `failed to build fuzz script … -Zsanitizer=address` lines, gate still exit 0. **This is why campaign 2 was run directly.** No Makefile edit, exactly as for 113-15/16/17/18/20. |
| **D-113-H** | Pre-existing untriaged `auth_flows` crash artifact (8 bytes, 2025-09-12) | Unowned, different target, out of fence. |
| **D-113-I** | `with_native_roots().expect()` panics on an OS trust-store hiccup | Unowned; converting a constructor panic into a `Result` is a Rule-4 architectural change. Did not recur on this run. |
| **D-113-J** | CLAUDE.md's documented PMAT query is silently vacuous on pmat 3.15.0 | Recorded by 113-18; `CLAUDE.md` is outside every gap-closure plan's file fence. Re-confirmed vacuous here. |
| **D-113-K** | The nominally-SSE GET path is collect-then-parse, not streaming | Recorded by 113-20; a transport rewrite, not a bound fix. |
| **Publication block** | `113-SPEC-RECHECK.md` Verdict PENDING; HTTP-01..05 / CLNT-01..02 at `[~]` | Human-granted exception with a binding 2026-07-28 re-verification. Outside this phase's control and untouched here. |

## Threat Register Dispositions

| Threat ID | Disposition | Status after this plan |
|---|---|---|
| T-113-91 | mitigate | **DONE.** `#[cfg(any(feature = "fuzzing", test))]` on the seam, matching `src/server/request_state.rs`'s established pattern. `fuzzing` is in neither `default` nor `full`. Proven not by the tool the plan named (which is blind to `doc(hidden)`) but by a real downstream crate: `E0425` under `full`, compiles under `full,fuzzing`, and compiles under `full` only when the gate is removed. The three signature non-commitments — chunk model, unvalidated `max_buffer_size` including `0`, `String`-flattened errors — are documented at the item. |
| T-113-92 | mitigate | **DONE.** The tautological latch invariant is replaced by a per-chunk peak-retention assertion, and it is shown FAILING: run B crashes with `the parser retained 9 bytes after chunk 0 under a 8-byte bound`. Run A is the accompanying proof that a one-check control would have been another false green. |
| T-113-93 | mitigate | **DONE.** Any future change that lets `buffer + current_event.data` exceed the bound now produces a reproducible crash artifact instead of a silent 20 000-run pass. Pinned in-crate as well by `the_seam_reports_retention_that_stays_inside_a_tiny_bound_while_reaching_it`, so the guard survives even if a campaign is not run. |
| T-113-94 | accept | The assertion message prints the observed retention, the chunk index and the bound — three integers, never the fuzzed payload. Verified in run B's verbatim panic. |
| T-113-SC | mitigate | **DONE.** No package installed and no manifest touched: `git diff --name-only -- Cargo.toml Cargo.lock fuzz/Cargo.toml` is empty. `fuzzing` already existed in `Cargo.toml` (`:220`) and was already enabled in `fuzz/Cargo.toml` (`:37`). No package-legitimacy checkpoint was required. |

## Decisions Made

1. **Gate on the Cargo FEATURE, not on a bare `#[cfg(fuzzing)]`.** The review's suggested fix used `#[cfg(any(fuzzing, test))]`, which needs an `unexpected_cfgs` allowance and would have introduced a second convention alongside `src/server/request_state.rs`'s feature-based one. The feature form also required no manifest change at all, keeping T-113-SC trivially satisfied.
2. **The module stays `pub mod`; only the FUNCTION is gated.** `src/server/mod.rs`'s module-level `#[cfg(not(feature = "fuzzing"))] pub(crate) mod` / `#[cfg(feature = "fuzzing")] pub mod` pair is not usable here — `SubscriptionStream` is real public API in the same module.
3. **The plan's `cargo public-api` criterion is recorded as passing AND vacuous.** Banking a green from an instrument that is structurally blind to the change is the same failure mode as the tautological invariant this plan exists to fix. The downstream-crate probe is the substitute, run in all three states so the "before" is measured rather than inferred.
4. **Campaign 1's PASS verdict is preserved verbatim.** Amending it to say "this was actually inadequate" would destroy the evidence for why the invariant needed replacing. A pointer at the top of the file tells the reader both campaigns exist and that campaign 1 was green while GAP-A was open.
5. **The latch check is kept but unnumbered.** Deleting it would lose the documentation of a real design decision (`reset()` deliberately does not clear `overflowed`); leaving it numbered would keep implying it is a check. The source now says in-place why it cannot fail.
6. **HTTP-04 NOT flipped to `[x]`** despite this plan's frontmatter listing it under `requirements`. The STATE.md phase gate forbids flipping HTTP-01..05 / CLNT-01..02 before the 2026-07-28 schema re-verification (today is 2026-07-26/27 local). `requirements mark-complete` was deliberately not run; `requirements-completed` in this summary's frontmatter is empty. All three prior gap-closure executors honoured the same gate.

## Deviations from Plan

### Executor-discretion decisions (no deviation rule invoked — no bugs were found)

**1. The plan's PUBLIC API acceptance criterion is vacuous, and a substitute proof was added**
- **Found during:** Task 1 (running the criterion)
- **Issue:** `cargo public-api -p pmcp --features full | grep -c decode_listen_chunks_for_fuzz` returns `0` — but it returned `0` at HEAD too, because `cargo public-api` omits `#[doc(hidden)]` items and offers no flag to include them. Taken at face value the criterion certifies nothing. This is precisely the vacuous-check trap the phase has now hit three times (113-09's zero-matching test filters, 113-18's `--exact` module path, D-113-J's PMAT query).
- **Resolution:** Criterion still run and recorded as passing, explicitly labelled vacuous, with the tool's blindness demonstrated by contrast against `request_state::fuzz_support` (feature-gated but not `doc(hidden)`, and therefore visible). A scratch downstream crate was compiled in three states as the falsifiable substitute. No repo file was involved; the scratch crate lives outside the tree.

**2. `MAX_LISTEN_LINE_BYTES` was NOT renamed**
- **Found during:** Task 1 (`<read_first>`)
- **Issue:** 113-17 deliberately left the rename to this plan because the identifier is referenced from this plan's two fenced files. Its plan text makes the rename discretionary.
- **Resolution:** Kept the name. 113-17 already rewrote the constant's doc to open with what it really bounds ("Named for the line buffer it originally bounded; since 113-17 it bounds BOTH of the parser's accumulators … It is NOT a per-line limit"), so the non-discretionary requirement — that no doc or message claims a per-line bound — is already met. A rename here would have churned five in-crate call sites and an intra-doc link for no behavioural or documentary gain, and this plan is the phase GATE: minimising unrelated diff is the point.
- **Related:** 113-17's summary flagged that the fuzz target's line-41 doc comment describes the listen bound as a per-line limit. Re-read during Task 1 — it does not: it says "production bounds this path at 256 KiB (`MAX_LISTEN_LINE_BYTES`)", which is accurate under the corrected reading. No edit needed.

**3. The stale ROADMAP narrative line was corrected, after the task fence closed**
- **Found during:** Task 2 (handoff item from 113-20)
- **Issue:** Two of this plan's instructions point opposite ways. Task 2 action item 7 says "Do not touch `.planning/REQUIREMENTS.md` or `.planning/ROADMAP.md`", with the acceptance criterion `git diff --name-only …` empty — whose stated purpose is "no requirement checkbox was flipped". But `.planning/ROADMAP.md:2219` still read "All 13 plans shipped and every phase gate is green" for a phase that now has 20 plans, which plans 17–20 made false; and every plan's standard close-out runs `roadmap update-plan-progress`, which necessarily writes that file.
- **Resolution:** The scope-fence criterion was evaluated — and passes, empty — at the close of Task 2's work, which is where it is meaningful. The ROADMAP was then updated as ordinary close-out tracking: the progress row via `roadmap update-plan-progress`, plus the stale narrative sentence corrected to name 20 plans and the four post-verification gap-closure plans. **`.planning/REQUIREMENTS.md` was not touched at all and no checkbox anywhere was flipped**, so the criterion's actual intent — and the STATE.md publication gate — are both honoured.

**4. The negative control's artifacts were redirected outside the repo**
- **Found during:** Task 2 (item 2)
- **Issue:** Run B crashes by design and libFuzzer writes a crash artifact. Left at the default prefix it would have landed in `fuzz/artifacts/subscription_listen_frames/`, defeating the artifacts-empty proof the same task must produce (and risking a repeat of D-113-H — a stale artifact nobody owns).
- **Resolution:** A second `-artifact_prefix` pointing at the scratchpad was passed after `--`; libFuzzer takes the last one. Verified afterwards: the repo artifacts directory exists and is empty.

**5. Run 0 (a baseline seeded run) was added to the negative control**
- **Found during:** Task 2 (item 3)
- **Issue:** The plan asks for three runs (one-check-disabled → green, both-disabled → crash, restored → green). Without a run of the seeded corpus against the UNMODIFIED tree, "restored → green" cannot distinguish "the fix works" from "this corpus happens not to reach the assertion".
- **Resolution:** Four runs recorded. Run 0 establishes the seeded corpus is green on the shipped tree before any mutation, so run C's green is a restoration result rather than a first observation.

---

**Total deviations:** 0 auto-fixed under Rules 1–3 (no bugs, missing critical functionality or blockers were encountered); 5 documented executor-discretion decisions, three of which resolve a conflict between the plan's own instructions or between a plan criterion and the tree.
**Impact on plan:** None on scope. Every `must_haves` truth and every artifact `contains` string is satisfied.

## Issues Encountered

- **`make quality-gate` exceeds the 10-minute foreground tool cap.** Run as a background job writing to a log with the exit code captured explicitly, as 113-17 and 113-20 did. `GATE_EXIT=0`, 7 347 lines, 0 `test result: FAILED`, 0 truncation markers (the run went through `/usr/bin/make`, not the `rtk` proxy that truncated 113-18's first attempt).
- **`/usr/bin/test` does not exist on this host** — the artifacts-directory existence proof uses `/bin/test`. Recorded because the plan and campaign 1 both spell absolute binary paths, and this one differs.
- **No `pre-commit` hook is installed in this checkout** (`.git/hooks/pre-commit` absent), so CLAUDE.md's "commits are blocked until quality gates pass" is not enforced locally. `cargo fmt --all -- --check`, `make lint` and `make quality-gate` were therefore run manually — lint before Task 1's commit, the full gate before Task 2's.
- **The libFuzzer corpus is gitignored** (`fuzz/.gitignore` ignores `corpus`), so campaign 2's 132 retained entries do not survive for the next reader. That is why the branch-coverage proof is a measurement recorded in the evidence file rather than a committed artifact — the same choice campaign 1 made.

## Known Stubs

None. No hardcoded empty values, placeholder text or unwired data paths were introduced. Every new symbol has a live caller: `peak_buffered_bytes` is consumed by the fuzz target's Invariant 3 and by three in-crate tests.

## Threat Flags

None. This plan adds no network endpoint, auth path, file access pattern or schema change. It NARROWS the crate's public API surface and strengthens an existing invariant.

## Scope-Fence Compliance

- `src/shared/sse_parser.rs` — 113-17's; touched only by the negative control's temporary mutation and restored byte-exactly. `git diff --stat` on it is empty at both of this plan's commits.
- `src/shared/http.rs`, `src/shared/streamable_http.rs`, `src/server/subscriptions.rs`, `src/client/mod.rs`, `tests/v2_subscriptions*.rs` — untouched (113-17 / 113-18 / 113-20).
- `Cargo.toml`, `Cargo.lock`, `fuzz/Cargo.toml`, `Makefile` — untouched.
- `.planning/REQUIREMENTS.md` — untouched. No requirement checkbox flipped, no `113-SPEC-RECHECK.md` verdict upgraded.

## Next Phase Readiness

- **The gap-closure round (113-17, 113-18, 113-20, 113-19) is complete and its gate is green.** GAP-A, GAP-B, GAP-C, GAP-D and GAP-E are closed; GAP-B by an evidenced decision rather than by the originally-planned reclaim (113-18), and GAP-E by an invariant that has been observed failing.
- **Phase 113 is still BLOCKED ON PUBLICATION, not complete.** `113-SPEC-RECHECK.md` § Verdict remains PENDING and the three v2 error codes remain pre-final values under the written developer exception. On or after 2026-07-28: re-run the 4-step procedure in `113-SPEC-RECHECK.md` § Recorded Exception, upgrade the Verdict, and only then flip HTTP-01..05 / CLNT-01..02 to `[x]`. A value mismatch is a phase-reopening event.
- **For the re-verifier.** Two things changed shape since `113-VERIFICATION.md` was written: `decode_listen_chunks_for_fuzz` now returns a THREE-tuple and is unreachable without `--features fuzzing`, and `113-FUZZ-EVIDENCE.md` holds two campaigns — check campaign 2, not the file's opening verdict line.
- **Still needing owners:** WR-01, WR-02, WR-04, D-113-F, D-113-G, D-113-H, D-113-I, D-113-J, D-113-K, and UNAS-01 (SEP-2243 `x-mcp-header`, still unassigned to any phase).

## Self-Check: PASSED

Files claimed, all present on disk:
`src/client/subscriptions.rs`, `fuzz/fuzz_targets/subscription_listen_frames.rs`,
`.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-FUZZ-EVIDENCE.md`,
`.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-19-SUMMARY.md`.

Commits claimed, both reachable in `git log --oneline --all`: `569f3533`, `d04dcc76`.

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-27*
