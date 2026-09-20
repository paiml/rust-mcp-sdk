---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 20
subsystem: transport
tags: [streamable-http, sse, dos-hardening, semver, body-limits, gap-closure]

# Dependency graph
requires:
  - phase: 113
    provides: "113-17's `pub(crate) SseParser::feed_complete_body` bypass and both whole-body transport call sites already migrated onto it"
  - phase: 113
    provides: "113-17's `<config_surface_decision>` — the MEASURED `constructible_struct_adds_field` major break that rules out a `pub` config field"
provides:
  - "`DEFAULT_MAX_COLLECTED_BODY_BYTES` (16 MiB) + a PRIVATE `StreamableHttpTransport` field + the additive inherent `with_max_collected_body_bytes()` builder"
  - "`StreamableHttpTransport::collect_body_within_cap` — the ONE place a peer-controlled response becomes a buffer, enforcing a STREAMING bound (`Limited`) plus an early `Content-Length` refusal"
  - "`feed_complete_body`'s byte-cap precondition is an ESTABLISHED FACT naming both enforcing call sites, not a requirement on the caller"
  - "Zero `response.collect()` remain in `src/shared/streamable_http.rs` — all three whole-body reads are capped"
affects: [113-19, 114, 117]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A collected-body cap must be a STREAMING bound (`http_body_util::Limited`), never collect-then-measure — the latter performs exactly the allocation the cap exists to prevent"
    - "A peer-declared `Content-Length` is an OPTIMISATION that refuses early, never the authority; the delivered bytes are bounded independently"
    - "Configurable limits as: named `pub const` default + PRIVATE transport field + additive inherent builder method — never a `pub` field on an externally-constructible config struct (inherited from 113-17)"

key-files:
  created: []
  modified:
    - src/shared/streamable_http.rs
    - src/shared/sse_parser.rs
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md

key-decisions:
  - "Enforcement is `http_body_util::Limited`, not the plan's literal collect-then-check — collecting an over-cap body before measuring it would allocate the very thing the cap prevents, satisfying the letter of the must_have while defeating its stated purpose"
  - "ALL THREE whole-body reads are capped, not the two the plan names — the plan's premise that `StreamableHttpTransport` has two `response.collect()` sites is off by one (`jsonrpc_error_envelope` is a third)"
  - "The plan's two site labels are SWAPPED relative to the tree; `:498` is the `start_sse` GET and `:1019` is the POST response. Corrected in every doc and test name"
  - "The default is 16 MiB, matching `DEFAULT_HTTP_SSE_BUFFERED_BYTES`'s value but NOT its concept — both constants document that they are deliberately not unified"
  - "HTTP-04 deliberately NOT flipped to `[x]` — the STATE.md phase gate forbids it before the 2026-07-28 schema re-verification"

patterns-established:
  - "An over-cap test asserts BOTH on the returned `Err` and on the silence of the message channel — the second half is what proves the parser was never reached"
  - "Padding rides an SSE COMMENT line so a test body's BYTE COUNT varies while its parsed events do not"

requirements-completed: []

# Metrics
duration: 47min
completed: 2026-07-27
---

# Phase 113 Plan 20: Collected-Body Cap Summary

**`SseParser::feed_complete_body`'s byte-cap precondition is now an established fact: every whole-body read on `StreamableHttpTransport` goes through one `collect_body_within_cap` helper that refuses an over-cap `Content-Length` before reading a byte and bounds the bytes actually delivered with `http_body_util::Limited`, overridable through an additive inherent builder that adds no field to any public config struct.**

## Performance

- **Duration:** 47 min
- **Started:** 2026-07-27T01:38:09Z
- **Completed:** 2026-07-27T02:25:43Z
- **Tasks:** 1
- **Files modified:** 3

## Accomplishments

- **T-113-84 is discharged.** 113-17 routed both whole-body call sites through a `pub(crate)` bypass that performs no bound check and wrote the byte cap into the rustdoc as a *requirement on the caller* naming this plan. At that HEAD no caller met it. Both now do, plus a third the plan did not know about.
- **The bound is a STREAMING bound, not a post-hoc measurement.** `Limited::new(response.into_body(), max_bytes)` stops the read at the cap, so an over-cap body is never allocated whole. The peer-declared `Content-Length` is checked first and refuses before a single body byte is read, but it is explicitly an optimisation — a peer that understates or omits it gains nothing (T-113-93).
- **Zero `response.collect()` remain in the file.** `grep -n "response.collect()" src/shared/streamable_http.rs` returns nothing: the POST response, the `start_sse` GET stream and the v2 structured-error envelope all route through the one capped helper.
- **The milestone stays additive.** `cargo semver-checks` reports **223 checks: 223 pass, 30 skip — no semver update required**, with **no `constructible_struct_adds_field`**. No field was added to `StreamableHttpTransportConfig`, no `#[non_exhaustive]` was retrofitted, and the three rustdoc struct-literal examples at lines 64-96 are untouched (the diff's first hunk after the import line is at 279).
- **Each site is proven independently.** Four negative-control runs; run 1 in particular shows the POST site's over-cap test failing while the GET site's stays GREEN, which is the evidence that a single shared test would have passed with one site uncapped.

## Task Commits

1. **Task 1: Enforce an overridable collected-body cap at every `response.collect()` site** — `dea01b2a` (fix)

## Files Created/Modified

- `src/shared/streamable_http.rs` — `DEFAULT_MAX_COLLECTED_BODY_BYTES` (16 MiB) with rustdoc stating what it bounds, why it exists, how it differs from 113-17's SSE ceiling and what breaks at the boundary; the free fn `collected_body_over_cap`; a private `max_collected_body_bytes` field (initialised in `new_internal`, the sole real construction path behind `new` and `new_with_http2`) plus its `Debug` entry; the additive inherent `with_max_collected_body_bytes`; the `collect_body_within_cap` helper; enforcement at all three whole-body reads; both parser-call-site comments rewritten from "plan 113-20 is what makes it true" to "SATISFIED above"; a new `mod collected_body_cap` with 8 tests.
- `src/shared/sse_parser.rs` — `feed_complete_body`'s `# Precondition` heading changed from "a REQUIREMENT ON THE CALLER" to "SATISFIED by both call sites", naming `post_body`, `start_sse`, `collect_body_within_cap` and `DEFAULT_MAX_COLLECTED_BODY_BYTES`, and noting the bound is streaming so an over-cap body is never allocated whole. The prohibition on incremental feeders calling it is unchanged, and a new closing sentence keeps the obligation alive for any FUTURE caller.
- `.planning/.../deferred-items.md` — **D-113-K**, the recorded deferral of migrating the nominally-SSE GET path from `collect()` to incremental parsing.

## Pre/Post Test Counts

| Suite | At this plan's HEAD (`4b03aa12`) | After |
|---|---|---|
| `cargo test --lib --features full -- streamable_http` | **82 passed, 0 failed** | **90 passed, 0 failed** (strictly greater: +8) |

## Negative Control — four runs, per site

The acceptance criterion is per-site precisely because these are two independent
`collect()` call sites. "Removing the cap check" means replacing that site's
`collect_body_within_cap` call with the bare `response.collect()` it replaced.
All four runs used `cargo test --lib --features full -- streamable_http::tests::collected_body_cap`.

| Run | State | Result |
|---|---|---|
| 1 | Cap check REMOVED at the **POST-response** site (`post_body`) | **6 passed, 2 failed.** `post_response_one_byte_over_the_cap_is_refused_before_the_parser` FAILED (`a body over the cap must be refused: ()`) and `a_declared_content_length_over_the_cap_is_refused_early` FAILED with it — both drive that site. `start_sse_one_byte_over_the_cap_is_refused_before_the_parser` stayed **GREEN**. |
| 2 | POST site RESTORED | **8 passed, 0 failed.** |
| 3 | Cap check REMOVED at the **`start_sse` GET** site | **7 passed, 1 failed.** `start_sse_one_byte_over_the_cap_is_refused_before_the_parser` FAILED; every POST-site test stayed **GREEN**. |
| 4 | GET site RESTORED | **8 passed, 0 failed.** |

Runs 1 and 3 together are the evidence the two sites are independently capped:
each run leaves the *other* site's over-cap test green, so no single shared test
could have caught either regression.

## Escape Hatch

`raising_the_cap_admits_a_body_the_lower_one_refuses` — the exact `CAP + 1` body
refused by `post_response_one_byte_over_the_cap_is_refused_before_the_parser` is
accepted and parsed once the transport is built with
`with_max_collected_body_bytes(CAP * 4)`, and the parsed response arrives on the
message channel. **PASSED.**

Expressed at a scaled-down cap (512 → 2048 bytes) rather than at the 16 MiB
default, for the same reason 113-17 scaled its sibling: proving it literally at
the default would need a >16 MiB allocation in a unit test.
`every_constructor_defaults_the_cap_to_the_named_constant` closes that gap
separately — it asserts `new`, `new_with_http2` and the builder all agree with
`DEFAULT_MAX_COLLECTED_BODY_BYTES`, so "default = the named constant" and "the
builder overrides it" are both pinned without allocating 16 MiB.

## Verification Results

| # | Check | Result |
|---|---|---|
| 1 | `cargo test --lib --features full -- streamable_http` | **90 passed, 0 failed** (pre: 82) |
| 2 | Per-site negative control, four runs | **recorded above** |
| 3 | `cargo test --lib --features full -- sse_parser` | 24 passed, 0 failed (113-17's work untouched) |
| 3a | `cargo test --doc --features full -- sse_parser` | 10 passed, 0 failed |
| 4 | `cargo test --test v2_stateless_http --features full` | 23 passed, 0 failed (floor 23) |
| 5 | `cargo test --test v2_mrtr --features full` | 27 passed, 0 failed (floor 27) |
| 6 | `cargo test --test v2_subscriptions_client --features full` | 8 passed, 0 failed (floor 8) |
| 6a | `cargo test --test streamable_http_integration --features full` | 6 passed, 0 failed |
| 6b | `cargo test --test streamable_http_properties --features full` | 12 passed, 0 failed |
| 7 | `cargo build -p pmcp-team-servers --all-features` | exit 0 |
| 8 | `cargo build --lib --target wasm32-unknown-unknown` | exit 0 |
| 9 | `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks: 223 pass, 30 skip — no semver update required.** **NO `constructible_struct_adds_field`** (explicitly confirmed: the summary line is `no semver update required`, and no failure block was emitted). |
| 10 | `cargo public-api -p pmcp --features full diff 2.17.0` | **zero REMOVED items attributable to this plan.** Exactly 2 non-`zerocopy` `-` lines, both the pre-existing `ElicitRequestParams::deserialize<__D>` → `<D>` generic-parameter rename that 113-17 already documented and that is re-added on the `+` side of the same diff. This plan's additions are exactly two: `+pub const …DEFAULT_MAX_COLLECTED_BODY_BYTES: usize` and `+pub fn …StreamableHttpTransport::with_max_collected_body_bytes(self, usize) -> Self`. |
| 11 | `git diff src/shared/streamable_http.rs` — public config struct | **no field added** inside `pub struct StreamableHttpTransportConfig`, no `#[non_exhaustive]` retrofit, no change to `StreamableHttpTransportConfigBuilder::build`'s output. Every `StreamableHttpTransportConfig` occurrence in the diff is doc text or test code. Hunk list confirms the rustdoc examples at 64-96 are untouched (`@@ -11 +11 @@` then `@@ -279,0 +280,69 @@`). |
| 12 | `git diff --name-only -- Cargo.toml Cargo.lock` | **empty** |
| 13 | PMAT complexity | **nothing new.** Only the two known pre-existing D-113-F violations (`handle_post_fast_path` cog 30, `handle_post_with_middleware` cog 31) in `src/server/streamable_http_server.rs`. **Zero** violations in `src/shared/streamable_http.rs`. Used D-113-J's corrected query (`.summary.violations[] | select(.file …)`) because CLAUDE.md's documented `.violations[] | select(.path …)` is silently vacuous on pmat 3.15.0. |
| 14 | `make quality-gate` | **exit 0** (`GATE_EXIT=0`), **zero** `test result: FAILED` across the whole suite. The known pre-existing D-113-G behaviour reproduced exactly: 17 `failed to build fuzz script … -Zsanitizer=address` errors swallowed by the recipe's `|| echo`, gate still exits 0. Not a regression from this plan. D-113-I's `with_native_roots()` flakiness did NOT recur on this run. |
| 15 | Contract-first: `ls ../provable-contracts/contracts/` | `No such file or directory` (exit 1) — no contract YAML exists for these surfaces in this environment, as the plan predicted. Recorded rather than skipped silently. |

**Source acceptance greps (all pass):**

- `grep -c "feed_complete_body" src/shared/streamable_http.rs` → **exactly 2** (the tripwire is preserved: every new comment and rustdoc in that file deliberately says "the parser's complete-body entry point" instead of naming the identifier).
- `grep -c "max_collected_body_bytes" src/shared/streamable_http.rs` → **22** (floor 4: constant, seam, and each enforcement site).
- `grep -n "response.collect()" src/shared/streamable_http.rs` → **no match**. Every whole-body read is capped.
- Both enforcement sites reference the cap on a line PRECEDING their parser call: `collect_body_within_cap(response, self.max_collected_body_bytes)` at the top of each handler, parser call ~30 and ~140 lines later respectively.
- `grep -nE "TODO|FIXME|XXX|TBD" src/shared/streamable_http.rs` → **no match** (zero SATD).

## Threat Register Dispositions

| Threat ID | Disposition | Status after this plan |
|---|---|---|
| T-113-84 | mitigate | **DISCHARGED — this is the threat 113-17 deferred here.** A named cap constant, a private `StreamableHttpTransport` field, an additive builder seam, and enforcement at every whole-body read before the parser is reached. Proven per-site by an over-cap behaviour test asserting on the returned `TransportError`, and per-site by a negative control that fails when that site's check is removed. The disposition now describes code that exists. |
| T-113-92 | accept (residual) | An attacker can still make the client allocate up to the configured cap per request. Bounded per-request rather than by stream age, and operator-overridable. Same accepted shape as 113-17's T-113-83. |
| T-113-93 | mitigate | **Done.** The `Content-Length` hint refuses early and is documented in three places as a peer-controlled optimisation, never the authority. The delivered bytes are bounded independently by `Limited`, so understating or omitting the header gains nothing. Proven by the split test pair: the over-cap tests use `with_chunked_body` (NO `Content-Length` at all) and still refuse, while `a_declared_content_length_over_the_cap_is_refused_early` covers the header path and asserts the declared size appears in the message. |
| T-113-94 | accept, **RECORDED** | **D-113-K** in `deferred-items.md`. Capping bounds the allocation but does not make the nominally-long-lived SSE GET path incremental; the rewrite is a transport change (dispatch timing, abort handle, resumption callback, middleware shape), not a bound fix. The entry carries that rationale and a fix shape pointing at `HttpTransport::connect_sse`. |
| T-113-95 | mitigate | **Done.** No `pub` field on `StreamableHttpTransportConfig`; the cap lives on a private transport field with an additive inherent seam. Asserted twice — by the `git diff` criterion (row 11) and by `cargo semver-checks` naming `constructible_struct_adds_field` (row 9). |
| T-113-SC | mitigate | No package installed and no manifest touched — `git diff --name-only -- Cargo.toml Cargo.lock` is empty. `http_body_util::{Limited, LengthLimitError}` come from `http-body-util = "0.1"`, already a direct dependency and already imported in this file for `BodyExt`/`Full`. No package-legitimacy checkpoint was required. |

## Decisions Made

1. **Enforcement is a STREAMING bound (`http_body_util::Limited`), not the plan's literal collect-then-check.** The plan's action item 3 asks to enforce "on the actual collected length"; taken literally that collects the whole hostile body and *then* refuses it, performing exactly the unbounded allocation the objective names as the threat. `Limited` stops the read at the cap, so the allocation is bounded during the read. The plan's `must_have` ("refused … BEFORE any of it reaches the parser") is satisfied either way; only this reading also satisfies the objective's stated purpose. Cost: when `Content-Length` is absent the exact body size is unknowable, so the message says "delivered more than the cap (Content-Length absent or understated)" rather than inventing a total; when the header IS over the cap the declared size is named.
2. **All three whole-body reads are capped, not the two the plan enumerates.** `jsonrpc_error_envelope` (the D-113-E v2 structured-error reader) is a third `response.collect()` on the same trust boundary. Leaving it uncapped would have left a live instance of T-113-84 in the very file whose invariant this plan establishes, findable by anyone grepping `response.collect()`. It takes the cap as a parameter because it is a static fn; an over-cap envelope simply is not an envelope, so it returns `None` and the caller falls back to the status-only transport error — the same behaviour a malformed body already produced.
3. **The default is 16 MiB, the same VALUE as `DEFAULT_HTTP_SSE_BUFFERED_BYTES` but explicitly not the same CONCEPT.** The same base64 arithmetic drives both (~4/3 expansion; a 12 MiB binary is already 16 MiB encoded), but one bounds incremental in-flight retention on a long-lived `HttpTransport` reader and the other bounds a one-shot collected body on `StreamableHttpTransport`. Both constants' rustdoc says so, so nobody later "unifies" them (113-17 decision 6 upheld).
4. **The field is a plain `usize`, not an `Arc<AtomicUsize>`.** `StreamableHttpTransport` derives `Clone`, so a clone taken *before* the builder call keeps the old cap. Accepted: the builder is `self`-consuming and used at construction, exactly the shape `HttpTransport::with_sse_buffered_bytes` already ships.
5. **HTTP-04 was NOT flipped from `[~]` to `[x]`,** despite this plan's frontmatter listing it under `requirements`. The STATE.md phase gate forbids flipping HTTP-01..05 / CLNT-01..02 before the 2026-07-28 schema re-verification (today is 2026-07-26). `requirements mark-complete` was deliberately not run; `requirements-completed` in this summary's frontmatter is empty. Both prior wave-1 executors honoured the same gate.

## Deviations from Plan

### Executor-discretion decisions (no deviation rule invoked — no bugs were found)

**1. The plan's two call-site labels are SWAPPED relative to the tree**
- **Found during:** Task 1 (`<read_first>`, reading the two line ranges)
- **Issue:** `<interfaces>` and `key_links` label `~:498` as the "POST-response collect site" and `~:1012` as the "`start_sse` GET collect site". The tree at `4b03aa12` is the other way round: `:498` is inside `start_sse` (it handles `405 METHOD_NOT_ALLOWED`, inserts `Last-Event-ID`, and calls `apply_response_middleware("GET", …)`), and `:1019` is inside `post_body` (`apply_response_middleware("POST", …)`). 113-17's summary carries the same inversion ("POST-response `start_sse`").
- **Resolution:** Used the tree's reality. Both sites are capped either way, so nothing substantive changed; the labels are corrected in every comment, rustdoc, test name and table in this summary so the next reader is not misled.

**2. A THIRD `response.collect()` site was capped (Rule 2 — missing critical functionality)**
- **Found during:** Task 1 (`grep -n "response.collect()"` returned three hits, not two)
- **Issue:** `jsonrpc_error_envelope` at `:815` is an uncapped whole-body read of a peer-controlled body on the same trust boundary the plan's threat model describes. The plan's objective asserts there are two such sites.
- **Resolution:** Capped through the same helper, with the cap threaded as a parameter. Documented at the site and covered by `an_over_cap_v2_error_envelope_falls_back_to_the_status_error`. This strengthens rather than relaxes every acceptance criterion (the cap-identifier count criterion is `>= 4`; it is 22).

**3. `Limited` rather than collect-then-measure**
- **Found during:** Task 1 (item 3)
- **Issue / Resolution:** See Decisions Made #1. Recorded here because it is a deliberate departure from the action text's literal wording.

**4. The escape-hatch test is expressed at a scaled-down cap**
- **Found during:** Task 1 (item 6)
- **Issue:** "a body refused at the DEFAULT cap succeeds once the cap is raised" taken literally needs a >16 MiB allocation in a unit test, which the same action item forbids ("Use a small configured cap so the tests cost bytes, not megabytes"). Identical to 113-17 deviation #3.
- **Resolution:** `raising_the_cap_admits_a_body_the_lower_one_refuses` proves the wiring at 512 → 2048 bytes, and `every_constructor_defaults_the_cap_to_the_named_constant` separately pins that all three construction paths default from the named constant.

**5. The private field is initialised in `new_internal` only**
- **Found during:** Task 1 (item 2)
- **Issue:** The plan asks for initialisation "in all three construction paths (`new`, `new_with_http2`, `new_internal`)". `new` and `new_with_http2` are one-line delegations to `new_internal`; adding an initialiser to them is not expressible.
- **Resolution:** Initialised once in `new_internal`, and `every_constructor_defaults_the_cap_to_the_named_constant` asserts the default through `new` AND `new_with_http2` so the plan's actual requirement (all three paths carry the default) is proven rather than assumed.

---

**Total deviations:** 1 auto-added under Rule 2 (the third uncapped collect site); 4 documented executor-discretion decisions, all resolving an inconsistency between the plan's text and the tree or between two of the plan's own requirements.
**Impact on plan:** None on scope. Every `must_haves` truth and every artifact `contains` string is satisfied.

## Issues Encountered

- **`make quality-gate` exceeds the foreground tool timeout.** Re-run as a background job writing to a log with the exit code captured explicitly, as 113-17 did. Result: `GATE_EXIT=0`, zero `test result: FAILED`.
- **The gate's fuzz stage still builds 0 of 17 targets** and still exits 0 (D-113-G, pre-existing and unowned). Reconfirmed on this run: 17 `failed to build fuzz script … -Zsanitizer=address` errors. Not a regression from this plan, and this plan adds no fuzz-reachable surface (the cap lives above the parser, whose fuzz seam 113-19 owns).
- **CLAUDE.md's documented PMAT query is vacuous on pmat 3.15.0** (D-113-J, recorded by 113-18). Used the corrected query; recorded so the complexity result is falsifiable rather than a silent "no output".

## Known Stubs

None. No hardcoded empty values, placeholder text or unwired data paths were introduced. Every new symbol has a live production caller: the constant is read by `new_internal`, the field by all three enforcement sites, and the builder by four tests plus the documented escape hatch.

## Scope-Fence Compliance

- `src/shared/http.rs` — untouched (113-17's).
- `src/client/subscriptions.rs` — untouched (113-17's).
- `fuzz/fuzz_targets/` and `decode_listen_chunks_for_fuzz` — untouched (113-19's).
- `src/shared/sse_parser.rs` — touched ONLY for item 4's `feed_complete_body` rustdoc, as the plan permits. No code change: `git diff` on that file is doc-comment lines only.
- `src/server/streamable_http_server.rs` — untouched; D-113-F's two pre-existing cog-25 violations are not this plan's to fix.
- D-113-I (`with_native_roots().expect()` panics on an OS trust-store hiccup) lives in the file this plan owns but is NOT caused by this plan's changes, and converting a constructor panic into a `Result` is an architectural change (deviation Rule 4). Left recorded, not touched.

## Threat Flags

None. This plan adds no network endpoint, auth path, file access pattern or schema change. It narrows an existing trust boundary.

## Next Phase Readiness

- **Wave 2 is complete and 113-19 (the phase gate, wave 3) is unblocked.** Its `make quality-gate` and `cargo semver-checks` runs cover this work; both were already run green here.
- **Release-note flag.** `StreamableHttpTransport` now refuses any response body over 16 MiB with a `TransportError` naming the limit, where it previously accepted an unbounded one. Base64 `image`/`audio` content is materially affected (~4/3 expansion — a 12 MiB binary is already 16 MiB encoded, before the JSON envelope). `with_max_collected_body_bytes()` is the escape hatch. This composes with 113-17's parallel note about `HttpTransport::connect_sse`.
- **HTTP-04 remains `[~]` implemented-pending-final-schema.** The 2026-07-28 gate in STATE.md is unchanged by this plan.
- **D-113-K is the follow-on** for anyone who wants the GET path to be genuinely streaming rather than merely bounded.

## Self-Check: PASSED

- Files claimed, all present on disk: `src/shared/streamable_http.rs`, `src/shared/sse_parser.rs`, `.planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md`, `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-20-SUMMARY.md`.
- Commit claimed, reachable in `git log`: `dea01b2a`.

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-27*
