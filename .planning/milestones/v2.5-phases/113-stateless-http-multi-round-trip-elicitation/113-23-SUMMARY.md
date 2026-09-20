---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 23
subsystem: api
tags: [streamable-http, subscriptions, auth, mcp-2026-07-28, denial-of-service, spec-conformance]

# Dependency graph
requires:
  - phase: 113 (plans 10/13/14/18)
    provides: the `subscriptions/listen` route, `ListenRegistry`, `ListenKey { principal, request_id }`, and the two concurrency caps this plan makes reachable
  - phase: 113 (plans 02/03)
    provides: `resolve_mrtr_principal` and `MrtrPrincipal` — the fail-closed decision table the listen route now mirrors
provides:
  - a fail-closed principal on the `subscriptions/listen` route (D-113-N closed)
  - the DELIBERATE no-auth divergence from the MRTR ingress, written down with its reason at both sites and pinned by a test
  - a measurement-backed answer to addendum Finding 5 — pmcp's actual `subscriptionId` emission on all three listen frame classes AND on the off-stream case
affects: [113-28 (requirement wording checkpoint), 113 phase re-verification, HTTP-04, HTTP-07]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "the listen route's identity decision is a small free function next to the route, mirroring a named sibling in core.rs — not another inline branch in an already-cog-25-adjacent function"
    - "an off-stream notification is measured on a `Server::run` transport and re-encoded through `pmcp::shared::transport::serialize_message`, so the assertion is against the crate's own wire encoder rather than an ad-hoc re-serialization"

key-files:
  created: []
  modified:
    - src/server/streamable_http_server.rs
    - src/server/subscriptions.rs
    - tests/v2_subscriptions.rs
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK-ADDENDUM-2026-07-26.md

key-decisions:
  - "the listen route's `(None, has_auth_provider = true)` row REFUSES with AUTHENTICATION_REQUIRED, matching resolve_mrtr_principal; the two v2 ingress paths on one server no longer disagree about what an unauthenticated caller is"
  - "the `(None, false)` row deliberately KEEPS the per-request anon#N and does NOT collapse onto MRTR's shared ANONYMOUS_PRINCIPAL — MRTR needs a stable principal because it is AEAD AAD, a listen principal is only a concurrency key, and unifying would cap a no-auth server at 4 concurrent streams instead of 64"
  - "the refusal is placed AFTER both -32601 gates and BEFORE the params parse, so a v1/capability-less server still answers 'no such method' and a refused caller's body is never inspected"
  - "v2_status_for_code is untouched: -32003 answers at HTTP 200 with a JSON-RPC error body, like the three RATE_LIMITED listen refusals; remapping it to 401 would move every other emitter of that code on this transport"
  - "Finding 5 VERDICT: HTTP-07's CURRENT wording is CONFIRMED by measurement, not refuted — no wording change proposed and nothing routed to 113-28; the phrase Finding 5 flagged was the EARLIER phrasing, already corrected"
  - "HTTP-04 and HTTP-07 stay [~]; REQUIREMENTS.md was not edited and no checkbox was flipped (the STATE.md publication gate binds independently)"

patterns-established:
  - "Pattern: a deliberate divergence between two paths is recorded at BOTH sites with its reason AND pinned by a named regression test, so the next reader meets a decision rather than inferring an oversight"
  - "Pattern: a spec REQUIRED/OPTIONAL split is discharged by measuring emission on every frame class INCLUDING the negative (off-stream) case, with the verbatim wire frames written into the planning record"

requirements-completed: []  # HTTP-04, HTTP-07 remain [~] — the STATE.md publication gate forbids flipping them this round

# Metrics
duration: 27min
completed: 2026-07-27
---

# Phase 113 Plan 23: Fail-Closed Listen Principal + Finding 5 Emission Audit Summary

**`subscriptions/listen` now refuses an unauthenticated caller on an auth-configured server with `AUTHENTICATION_REQUIRED` before any permit is taken (D-113-N), keeping the deliberate `anon#N` behaviour on no-auth servers; and pmcp's `subscriptionId` emission is measured on all three listen frame classes plus the off-stream case, confirming HTTP-07's current wording.**

## Performance

- **Duration:** 27 min
- **Started:** 2026-07-27T08:35:38Z
- **Completed:** 2026-07-27T09:02:38Z
- **Tasks:** 3 (4 commits — one follow-up for a pedantic lint the plan's verification command does not run)
- **Files modified:** 4

## Accomplishments

- **D-113-N closed.** `assemble_subscriptions_listen` resolves its principal through a new `resolve_listen_principal`, which implements the SAME three-row table as `resolve_mrtr_principal`. An unauthenticated caller on a server whose auth provider admits unauthenticated requests is now refused `-32003` **before** `registry.register`, so it never takes a permit.
- **The starvation is reproduced and denied.** 68 unauthenticated attempts (past the 64-slot global cap) are all refused, and an authenticated subscriber registers successfully afterwards. Under the negative control the authenticated subscriber gets `application/json` — a refusal body — which is the harm D-113-N names, observed directly.
- **The no-auth divergence is a documented decision, not an accident.** `anon#N` is preserved for the genuinely-provider-less case, the reason (AEAD binding vs. concurrency accounting; 64 → 4 dev-capacity loss) is written at both sites, and a named test fails if anyone unifies the rows.
- **Finding 5 answered by measurement.** Four verbatim wire frames captured and recorded in the addendum: the tag is present and equal to the request id on all three listen classes, and completely absent from a notification delivered off-stream.

## Task Commits

1. **Task 1: Fail closed on the listen route when an auth provider is configured** — `cba463b4` (fix)
2. **Task 2: Prove the fix — refusal, negative control, budget-exhaustion regression** — `2a899fd6` (test)
3. **Task 3: Answer Finding 5 — pin the `subscriptionId` emission on all three frame classes** — `9196a55e` (test)
4. *(gate follow-up)* **Hoist `ATTEMPTS` above the first statement** — `b266c572` (style)

## Files Created/Modified

- `src/server/streamable_http_server.rs` — `ListenServerView` gains `has_auth_provider` (read ONCE in `listen_server_view` via the existing public `Server::get_auth_provider`); new `resolve_listen_principal` free function; the route refuses via `listen_rejection_response(era, id, AUTHENTICATION_REQUIRED, …)`; the route rustdoc's ordered rejection list renumbered 1–6 with the new case at position 3.
- `src/server/subscriptions.rs` — `anonymous_principal`'s rustdoc gains a **Reachability** section (now reached only on a provider-less server) and a **Why this is NOT the MRTR ingress's shared `ANONYMOUS_PRINCIPAL`** section. Behaviour and `anonymous_principals_are_never_shared` untouched.
- `tests/v2_subscriptions.rs` — `OptionalBearer` (an auth provider that ADMITS unauthenticated requests), `server_with_optional_auth`, five new tests, the `ProgressTool` + duplex off-stream probe, and a `#[path = "common/duplex.rs"] mod duplex;` include.
- `.planning/phases/…/113-SPEC-RECHECK-ADDENDUM-2026-07-26.md` — new `# Finding 5 — resolved by measurement (plan 113-23, 2026-07-27)` section with the frame table, the four verbatim frames, the verdict, and what it does **not** discharge.

## The refusal, as observed

| Property | Observed |
|---|---|
| JSON-RPC error code | `-32003` (`AUTHENTICATION_REQUIRED`, the named constant — no bare literal) |
| HTTP status | **200**, deliberately unremapped — `AUTHENTICATION_REQUIRED` is not in `v2_status_for_code`'s 400 arm, exactly like the three `RATE_LIMITED` listen refusals |
| Body | JSON-RPC error envelope; `content-type` is `application/json`, **not** `text/event-stream` |
| Id | the ORIGINAL request id, echoed |
| Permit taken | none — the refusal runs before `registry.register` |
| `v2_status_for_code` | **byte-unmodified** (verified: the only two occurrences in the diff are prose in the new comment saying so) |

## Negative control — verbatim

`resolve_listen_principal`'s `(None, true)` row was reverted to `Some(anonymous_principal())`, the three tests were run, and the row was restored and re-run green.

**Expected split (1 and 3 fail, 2 passes) — CONFIRMED:**

```
        FAIL [   0.018s] pmcp::v2_subscriptions one_unauthenticated_caller_cannot_exhaust_the_global_listen_budget
    thread '...' panicked at tests/v2_subscriptions.rs:1040:9:
    assertion `left == right` failed: unauthenticated attempt 0 must be REFUSED, never granted a private uncapped anon#N principal: {"jsonrpc":"2.0","method":"notifications/subscriptions/acknowledged","params":{"notifications":{"toolsListChanged":true},"_meta":{"io.modelcontextprotocol/subscriptionId":0}}}
      left: Null
     right: Number(-32003)

        FAIL [   0.018s] pmcp::v2_subscriptions unauthenticated_listen_is_refused_on_an_auth_configured_server
    thread '...' panicked at tests/v2_subscriptions.rs:920:5:
    assertion `left != right` failed: no stream body is opened for a refused caller
      left: Some("text/event-stream")
     right: Some("text/event-stream")

        PASS [   0.019s] pmcp::v2_subscriptions unauthenticated_listen_still_serves_on_a_server_with_no_auth_provider
────────────
     Summary [   0.020s] 3 tests run: 1 passed, 2 failed, 10 skipped
```

Test 2 passing under the control is what proves it guards the OTHER row rather than duplicating test 1.

**Supplementary control — the starvation itself.** Test 3 short-circuits at attempt 0, so its second half (the authenticated subscriber) is never reached. With the control still in place AND that per-attempt assertion temporarily disabled, the harm appears directly:

```
    thread '...' panicked at tests/v2_subscriptions.rs:1055:5:
    assertion `left == right` failed: the authenticated subscriber gets a real stream, not a refusal body
      left: Some("application/json")
     right: Some("text/event-stream")
```

That is D-113-N's stated failure — 64 anonymous streams held by one caller, an authenticated subscriber denied — reproduced on a live socket. Both the assertion and the production row were restored; the suite is green.

## Finding 5 — the measured answer

**All three listen frame classes carry the tag, equal to the request id (`77`):**

```
(a) {"jsonrpc":"2.0","method":"notifications/subscriptions/acknowledged","params":{"notifications":{"toolsListChanged":true},"_meta":{"io.modelcontextprotocol/subscriptionId":77}}}
(b) {"method":"notifications/tools/list_changed","jsonrpc":"2.0","params":{"_meta":{"io.modelcontextprotocol/subscriptionId":77}}}
(c) {"jsonrpc":"2.0","id":77,"result":{"_meta":{"io.modelcontextprotocol/subscriptionId":77,"io.modelcontextprotocol/serverInfo":{"name":"v2-subscriptions","version":"1.0.0"}},"resultType":"complete"}}
```

**An off-stream notification carries none:**

```
{"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":"off-stream","progress":1.0,"total":2.0,"message":"halfway"}}
```

No `_meta` at all; the key is asserted absent both structurally (`params._meta`) and as a whole-frame substring, so a tag smuggled into another position would also fail.

**VERDICT: HTTP-07's current wording is CONFIRMED, not refuted.** `.planning/REQUIREMENTS.md` already reads "every notification **delivered on a subscription stream** carries … (the key is REQUIRED on `SubscriptionsListenResultMeta` but OPTIONAL on `NotificationMetaObject` …)". Every clause of that sentence now has a captured frame behind it. The phrasing Finding 5 flagged — "every delivered notification carries `subscriptionId` tagging" — was the EARLIER wording, and it *would* have overstated the schema by reading as a property of the notification type rather than of the delivery path. **No wording change is proposed and nothing is routed to the 113-28 checkpoint from Finding 5.** `.planning/REQUIREMENTS.md` was not edited.

**What this does NOT discharge:** HTTP-07 is still not met. The publication gate binds, and Finding 9's substantive risk is unchanged — both HTTP-07 obligations are post-RC additions and open PR #3006 still targets this exact surface. Measuring conformance to today's draft says nothing about whether today's draft survives.

## Decisions Made

- **Refusal placed before the params parse, not after.** The plan pinned it after the era + advertisement gates and before `registry.register` but left the position relative to `resolve_agreed_filter` open. It runs *before*, so a refused caller's body is never deserialized. No existing test changes: the only auth-configured fixture (`BearerSubjects`) rejects a missing token at the transport, and every bad-params test runs on a provider-less server.
- **`OptionalBearer` written test-local.** The plan anticipated this. `BearerSubjects` returns `Err` for a missing token, so the 401 fires in `extract_and_validate_auth` and D-113-N's precondition is structurally unreachable through it. `Ok(None)` is the real configuration the defect needs, and the test rustdoc says so.
- **The off-stream probe uses the duplex transport, not HTTP.** On `StreamableHttpServer` the listen registry is the ONLY server→client notification sink — that transport never calls `Server::run`, so `notification_tx` stays `None`. A "non-listen delivery path" therefore does not exist on HTTP and had to be observed on a `Server::run` transport. `notifications/progress` is the probe because `subscription_kind_of` excludes it *structurally* from the fan-out.
- **`ATTEMPTS = 68`, not 5.** The plan said "more than `MAX_LISTEN_STREAMS_PER_PRINCIPAL`". Five would have made the negative control's second half pass vacuously (the global budget would not be exhausted, so the authenticated subscriber would register even pre-fix). 68 exceeds `MAX_LISTEN_STREAMS_TOTAL` (64) and is what makes the starvation reproducible.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `items_after_statements` failed `make quality-gate` after the task commits**

- **Found during:** Task 3 (the gate run)
- **Issue:** The plan's per-task verification is `cargo clippy --features "full" --lib --tests -- -D clippy::all`, which passed. `make lint` — what the gate and CI actually run — adds pedantic + nursery, and `clippy::items_after_statements` fired on `const ATTEMPTS: i64 = 68;` declared after `let (addr, handle) = …`. First gate run: `QUALITY_GATE_EXIT=2`.
- **Fix:** Hoisted the `const` above the first statement. No behaviour change.
- **Files modified:** `tests/v2_subscriptions.rs`
- **Verification:** `make quality-gate` re-run → `QUALITY_GATE_EXIT=0`.
- **Committed in:** `b266c572`

**2. [Rule 1 - Bug] Nextest `LEAK` warnings on the two multi-socket tests**

- **Found during:** Task 2
- **Issue:** `unauthenticated_listen_still_serves_…` and `one_unauthenticated_caller_…` intermittently reported nextest `LEAK` (still a pass, but noise that would look like flake to a future reader). Several concurrent SSE streams make tokio runtime teardown exceed nextest's 100 ms default leak timeout.
- **Fix:** Deterministic teardown — drop the sockets, abort the accept loop, then `let _ = handle.await;` — with an in-source comment saying why. Four consecutive full-suite runs clean afterwards.
- **Files modified:** `tests/v2_subscriptions.rs`
- **Verification:** 4× `cargo nextest run --test v2_subscriptions` → `13 tests run: 13 passed, 0 skipped`, zero `LEAK`.
- **Committed in:** `2a899fd6`

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 test-hygiene bug)
**Impact on plan:** Both are inside the plan's own files and necessary to leave the gate green and the suite non-flaky. No scope creep; no production behaviour changed by either.

## Issues Encountered

- **The plan's `<interfaces>` cites HEAD `502b4170`; the actual base was `f20c2221`.** Every cited symbol, line region and constant was re-verified against the real tree before editing and all matched (`resolve_mrtr_principal` at `core.rs:1489`, the fallback at `streamable_http_server.rs:2935`, `ListenServerView` at 2655, `get_auth_provider` at `mod.rs:720`, `AUTHENTICATION_REQUIRED = -32003` at `error_codes.rs:97`). No drift.
- **`InitializeRequest` and `CallToolRequest` are `#[non_exhaustive]`**, so the duplex probe builds them through their constructors (`InitializeRequest::new`, `CallToolRequest::new` + `_meta` field assignment) rather than struct literals — the forward-compatible form, and the one that compiles from an integration crate.
- **Two pre-existing PMAT cog-25 violations remain in `streamable_http_server.rs`** (`handle_post_fast_path` 30, `handle_post_with_middleware` 31 — D-113-F, unowned, out of fence). This plan added **no third**: `assemble_subscriptions_listen` does not appear in `pmat analyze complexity --max-cognitive 25`, which is why the decision went into a free function rather than another inline branch.

## Verification Results

| Check | Result |
|---|---|
| `cargo nextest run --features full --test v2_subscriptions` | **15 passed, 0 skipped** (10 pre-existing + 5 new) |
| `cargo nextest run --features full --test v2_subscriptions_client` | **8 passed** — untouched |
| `cargo nextest run --features full --lib -- subscriptions` | **85 passed** |
| Negative control | executed, expected split confirmed, reverted, re-run green |
| `cargo clippy --features full --lib --tests -- -D clippy::all` | clean |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks: 223 pass, 30 skip / no semver update required** |
| `make quality-gate` (background + log poll) | **`QUALITY_GATE_EXIT=0`** — "ALL TOYOTA WAY QUALITY CHECKS PASSED"; 4337 tests passed across all stages, 0 failed |
| `pmat analyze complexity --max-cognitive 25` | 2 pre-existing violations in this file, **no new one** |
| Diff scope | exactly the 4 files in `files_modified`; `v2_status_for_code` byte-unmodified; `.planning/REQUIREMENTS.md` untouched |
| SATD scan (`TODO\|FIXME\|HACK\|XXX`) in both `src/` files | **0** |

The `⚠ CB-1200 / CB-1201` pmat advisories (`pv` CLI not installed) are pre-existing and informational per Phase-109 D-07; the gate exits 0 with them present.

## Threat Model Disposition

| Threat ID | Disposition | Evidence |
|---|---|---|
| T-113-105 (DoS via the anonymous fallback) | **mitigated** | `resolve_listen_principal` refuses before any permit; `one_unauthenticated_caller_cannot_exhaust_the_global_listen_budget` proves the authenticated subscriber is un-starved, and the negative control proves it was starved before |
| T-113-106 (private uncapped identity per request) | **mitigated** | the `anon#N` mint is unreachable on an auth-configured server |
| T-113-107 (collapsing the no-auth row) | **accepted, guarded** | `unauthenticated_listen_still_serves_on_a_server_with_no_auth_provider` opens five concurrent anonymous streams — one past the per-principal cap — and asserts all are served |
| T-113-108 (refusal message leaks the auth configuration) | **accepted** | the refusal implies a provider by construction; the message names the method and nothing else — no subject, no provider identity |
| T-113-109 (HTTP-07 recorded as met against an obligation the schema does not impose) | **mitigated** | three-class emission pins + the off-stream absence check; verdict written into the addendum; no checkbox flipped |

## Next Phase Readiness

- **113-24 onward is unblocked.** Nothing in this plan changes a shared seam: `v2_status_for_code`, `ListenRejection`, `ListenRegistry` and every public type are byte-unchanged (semver 223/223, no update required).
- **HTTP-04 and HTTP-07 stay `[~]`.** The STATE.md publication gate is unchanged by this plan and `.planning/REQUIREMENTS.md` was not edited. Finding 5's ⚠ can be retired from HTTP-07's caveat block by whoever runs the 2026-07-28 re-verification — Finding 9's ⚠ cannot.
- **Still open and unowned (untouched here):** D-113-Q (`sse_optimized.rs:266` unbounded `reqwest::Response::text()`), D-113-R (`drain_complete_lines` quadratic — HTTP-09 cannot close without it), D-113-F, D-113-G, D-113-H, WR-01/02/04, UNAS-01.

## Known Stubs

None. Every code path added by this plan is wired end-to-end and exercised over a real socket (or, for the off-stream probe, a real `Server::run` transport). No placeholder values, no hardcoded empties, no unwired components.

## Self-Check: PASSED

- All 5 declared files exist on disk.
- All 4 commit hashes (`cba463b4`, `2a899fd6`, `9196a55e`, `b266c572`) resolve in `git log`.
- Every `must_haves.artifacts` `contains` marker verified present: `has_auth_provider` and `AUTHENTICATION_REQUIRED` in `streamable_http_server.rs`, `anonymous_principal` in `subscriptions.rs`, `SUBSCRIPTION_ID_META_KEY` in `tests/v2_subscriptions.rs`, the Finding 5 section in the addendum.
- Both `key_links` patterns verified: `get_auth_provider` (the `ListenServerView` plumbing) and `AUTHENTICATION_REQUIRED` (the refusal) are present in `streamable_http_server.rs`.

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-27*
