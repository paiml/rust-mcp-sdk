---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 24
subsystem: api
tags: [streamable-http, mrtr, elicitation, denial-of-service, mcp-2026-07-28, security-bound]

# Dependency graph
requires:
  - phase: 113 (plan 06)
    provides: the D-15 verdict table (`MrtrIngest`, `route_mrtr_verdict`, `MrtrIngest::apply`) and the `Verdict` -> route mapping this plan adds two rows to
  - phase: 113 (plan 09)
    provides: `seal_input_required`, the single mint site, and `mrtr_egress`'s `Err(&'static str)` -> `fail_mrtr_egress` routing the mint-site backstop reuses
  - phase: 113 (plans 05/07)
    provides: `ClientBuilder::mrtr_round_limit` / `DEFAULT_MRTR_ROUND_LIMIT` — the client half whose value the server ceiling is defined as 2x of
provides:
  - a SERVER-side `MAX_MRTR_ROUNDS` ceiling, so the D-09 round counter is no longer enforced solely by the party it exists to constrain (D-113-L closed)
  - two independent enforcement points whose relationship is MEASURED (A necessary, B a backstop) rather than claimed symmetric
  - a raw-frame end-to-end proof that a client ignoring its own limit is refused, with the handler-invocation count as the falsifiable evidence
  - a checked 8-round legitimate-flow guard, which turns the 2x headroom into a property rather than a comment
affects: [HTTP-02, HTTP-03, 113-26, 113-27, 113 phase re-verification, 114 (Tasks reuses the MRTR identity/round pattern)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "a security bound gets a NAMED constant whose rustdoc records the sibling it is defined relative to, so the relationship survives an edit to either file"
    - "an ingress refusal placed BEFORE dispatch owns 'the handler never runs'; a same-invariant check at the write site is documented as an unreachable-by-construction backstop, with its INTERNAL_ERROR class justified by that unreachability"
    - "a `pub(crate)` constant an integration test cannot name is mirrored with a compile-time relationship assertion plus a test that MEASURES the live value, so the mirror is checked rather than commented"

key-files:
  created: []
  modified:
    - src/server/core.rs
    - tests/v2_mrtr.rs

key-decisions:
  - "MAX_MRTR_ROUNDS = 16, a CONSTANT and not a builder knob: a configurable ceiling would have to land on ServerCoreBuilder/ServerBuilder — files plan 113-25 is editing in this same wave — and a knob is not what closes D-113-L; an enforced bound is. The rustdoc records the deferral and its reason."
  - "16 is exactly 2x the shipped DEFAULT_MRTR_ROUND_LIMIT (8), so a default-configured pmcp client can never trip it; the relationship is asserted at compile time in tests/v2_mrtr.rs and exercised by an 8-round flow test."
  - "The refusal code is INVALID_PARAMS (-32602), matching the sibling MRTR reject, so v2_status_for_code -> HTTP 400 is unchanged and no new code enters the pre-final -3202x range the publication gate covers."
  - "The refusal MESSAGE is distinct from the generic MRTR_REJECT_MESSAGE and names the ceiling: it fires only AFTER the AEAD tag check passed, so it is not an authentication oracle and specificity costs nothing."
  - "Verdict::Expired at or past the ceiling is REFUSED, not re-elicited. Expiry is entirely within a server's own gift, so re-eliciting there would turn T-113-49's round-preservation property into the bypass it was written to prevent."
  - "Verdict::UnknownKey still resets to round 0 and is NOT treated as a bypass (T-113-113, ACCEPT): it is indistinguishable from a client starting a fresh operation, which any client may always do; the reason is written into the verdict-table rustdoc so it is not re-litigated."
  - "The mint-site check is placed AHEAD of every other mint precondition (principal, codec, binding), which is what lets a test prove ordering structurally: it refuses with NO codec configured, which the mint itself would need."
  - "HTTP-02 and HTTP-03 stay [~]; .planning/REQUIREMENTS.md was not edited and no checkbox was flipped."

patterns-established:
  - "Pattern: state the threat in the constant's own rustdoc — what the bound is, who used to enforce it, and why that was not enough — so the next reader meets the reasoning at the definition rather than in a planning file."
  - "Pattern: when two enforcement points exist, MEASURE which are necessary. NC-2 here is green, and saying so is what makes 'A necessary, B a backstop' a finding rather than a claim."
  - "Pattern: a loop test against a hostile peer carries a hard iteration cap sized as a multiple of the bound, so a regression fails FAST instead of hanging the suite."

requirements-completed: []  # HTTP-02 / HTTP-03 remain [~] — the STATE.md publication gate forbids flipping them this round

# Metrics
duration: 21min
completed: 2026-07-27
---

# Phase 113 Plan 24: Server-Side MRTR Round Ceiling Summary

**The MRTR round counter is no longer a security bound enforced only by the attacker: `MAX_MRTR_ROUNDS = 16` refuses a verified continuation at the ingress verdict before dispatch, a mint-site backstop refuses to seal past it, and a raw-frame client that ignores its own limit now terminates at exactly 16 handler invocations with a typed `-32602` instead of climbing toward 255.**

## Performance

- **Duration:** 21 min
- **Started:** 2026-07-27T09:07:32Z
- **Completed:** 2026-07-27T09:29:28Z
- **Tasks:** 2 (2 commits + this metadata commit)
- **Files modified:** 2

## Accomplishments

- **D-113-L closed.** `src/server/core.rs:2230` minted `inputs.round.saturating_add(1)` on every resend and compared it to nothing. The only bound in the tree was `DEFAULT_MRTR_ROUND_LIMIT` (8) in `src/client/mod.rs` — the D-09 security counter enforced exclusively by the party it exists to constrain. There is now a server-side ceiling, and `grep -rn "MAX_MRTR_ROUNDS" src/` no longer returns nothing.
- **The handler is the thing that stops running.** Enforcement point A lives in `route_mrtr_verdict`, whose `Reject` flows through `MrtrIngest::apply` to `Err((code, message))`, which both dispatch sites turn into a JSON-RPC error *without invoking the handler*. The end-to-end test asserts this with a counter, not with an error code — and NC-1 proves the assertion is load-bearing (see below).
- **Expiry cannot launder a round past the ceiling.** `Verdict::Expired` preserves the round precisely so a hostile server cannot reset a client's bound by letting tokens expire (T-113-49). Since letting one's own tokens expire is entirely within a server's gift, re-eliciting *at* the ceiling would have converted that property into the bypass. It is refused instead.
- **The 2x headroom is checked, not asserted.** `a_flow_within_the_client_default_limit_is_unaffected` drives a legitimate flow to MRTR round 8 — the full depth the shipped client default reaches — and completes with no refusal. A future "tighten the ceiling" change that broke deep flows fails here rather than in a deployment.
- **Saturation is now unreachable and proven so.** A proptest over the entire `0u8..=u8::MAX` range asserts that every round threaded into egress is strictly below the ceiling and that `saturating_add(1)` agrees exactly with widened arithmetic — i.e. it can no longer be observed saturating. That is what makes `RequestHandlerExtra::mrtr_round` a number a handler can reason about rather than a ceiling hiding three thousand rounds.
- **No public API change.** Everything added is `pub(crate)`, a private const, or test code. `semver-checks` is unchanged at 223/223, no update required.

## The ceiling and its documented relationship to the client half

| | Value | Where | Enforced by |
|---|---|---|---|
| Client bound (pre-existing) | `DEFAULT_MRTR_ROUND_LIMIT = 8` | `src/client/mod.rs:146` | the client — i.e. by the party it constrains |
| **Server ceiling (new)** | **`MAX_MRTR_ROUNDS = 16`** | `src/server/core.rs` | the server, at two points |

16 is exactly **2x** the client default, and the rustdoc says so rather than leaving it a coincidence two files apart. The consequences are both directions of the boundary:

- a default-configured `pmcp::Client` gives up at 8 requests and can therefore **never** trip the server ceiling;
- the headroom still admits a deliberately raised client limit while bounding an absent one.

`tests/v2_mrtr.rs` mirrors both constants (the server one is `pub(crate)`, so an integration test cannot name it) and pins the relationship at compile time:

```rust
const _: () = assert!(SERVER_MAX_MRTR_ROUNDS == CLIENT_DEFAULT_ROUND_LIMIT * 2);
```

The mirror is not a comment-pinned duplicate: the loop test **measures** the ceiling by resending until the server refuses and asserts the measurement equals the mirrored value, and `core.rs`'s own unit tests pin the boundary by name. Both halves cannot drift silently.

## Where the boundary sits, exactly

| Presented round | Ingress verdict | Handler runs? | Minted round |
|---|---|---|---|
| none (first call) | `Inert` | yes | 1 |
| 1 .. 14 | `Proceed` | yes | 2 .. 15 |
| **15** (`MAX_MRTR_ROUNDS - 1`) | `Proceed` | yes | **16** — the last admissible mint, landing exactly ON the ceiling |
| **16** (`MAX_MRTR_ROUNDS`) | **`Reject`** `-32602` | **no** | none |

So the observed handler-invocation count at termination is **exactly 16** = one first call + fifteen admitted resends. The refusing seventeenth request never reaches the handler.

## Enforcement points

**A — ingress (`route_mrtr_verdict`), the one that matters.**
`refuse_past_round_ceiling(round, method)` is extracted rather than inlined in two match arms, both because `route_mrtr_verdict` exists to hold `mrtr_ingest` under cognitive-complexity 25 and because a bound duplicated inline is a bound that can be half-removed. It fires for `Verdict::Ok` and `Verdict::Expired`, `tracing::warn!`s on `target: "mcp.mrtr"` with the method, observed round and ceiling, and returns `INVALID_PARAMS` with a message that names the limit.

**B — mint (`seal_input_required`), an unreachable-by-construction backstop.**
`inputs.round.saturating_add(1) > MAX_MRTR_ROUNDS` returns the existing `Err(&'static str)` shape, which `mrtr_egress` routes through `fail_mrtr_egress` as `INTERNAL_ERROR`. That classification is correct *precisely because* reaching it means the ingress invariant broke — the client did nothing new, the server did. The check is placed ahead of every other mint precondition, which is what lets `mint_backstop_precedes_every_other_mint_precondition` prove the ordering structurally: it refuses with **no codec configured**, which the mint itself would need.

## Negative controls — recorded verbatim

All three were run against the landed code by patching `src/server/core.rs`, running the two new end-to-end tests, and reverting with `git checkout -- src/server/core.rs`. A fix whose controls were never run is indistinguishable from a comment.

### NC-1 — enforcement point A disabled, B intact

Predicted: the loop still terminates (B stops the mint) but the handler-count assertion fails and the error code becomes `INTERNAL_ERROR` rather than `INVALID_PARAMS`.

```
running 2 tests
test a_flow_within_the_client_default_limit_is_unaffected ... ok
test a_client_that_ignores_its_own_round_limit_is_stopped_by_the_server ... FAILED

---- a_client_that_ignores_its_own_round_limit_is_stopped_by_the_server stdout ----
assertion `left == right` failed: body was {"jsonrpc":"2.0","id":17,"error":{"code":-32603,"message":"a requestState continuation cannot be minted past the server's round limit"}}
  left: 200
 right: 400

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 27 filtered out; finished in 0.03s
```

**Confirmed, with one honest correction to the plan's prediction.** The loop *did* still terminate — request id 17 is answered — and the code *did* degrade to `-32603` at HTTP 200. But the **status** assertion fires before the handler-count assertion, so the run above does not by itself exhibit the handler divergence the plan named as A's distinguishing property. A supplementary probe was therefore run with A still disabled and the status assertion temporarily replaced by the handler-count assertion (both files were committed first, so the probe was reverted with `git checkout --`):

```
---- a_client_that_ignores_its_own_round_limit_is_stopped_by_the_server stdout ----
assertion `left == right` failed: NC-1 PROBE handler invocations
  left: 17
 right: 16
```

**17 versus 16.** With A disabled the handler runs on the refusing round too — the mint is stopped, but only *after* the handler has already done the work. That is the property A owns and B structurally cannot provide, and it is why the end-to-end test asserts an invocation counter rather than only an error code.

### NC-2 — enforcement point B disabled, A intact

Predicted: everything still passes.

```
running 2 tests
test a_flow_within_the_client_default_limit_is_unaffected ... ok
test a_client_that_ignores_its_own_round_limit_is_stopped_by_the_server ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 27 filtered out; finished in 0.03s
```

**Still green, recorded as such.** A is *sufficient* for the client-driven path, so B is a **backstop**, not a co-equal check. This plan does not claim two independently *necessary* enforcement points, because the measurement says otherwise. B earns its place against a different failure mode — a future refactor of the verdict table silently deleting the bound, which is exactly what D-113-L already demonstrated once — and that failure mode is what NC-1 exhibits, not NC-2.

### NC-3 — both disabled

Predicted: the loop does not terminate at the ceiling.

```
running 2 tests
test a_flow_within_the_client_default_limit_is_unaffected ... ok
test a_client_that_ignores_its_own_round_limit_is_stopped_by_the_server ... FAILED

---- a_client_that_ignores_its_own_round_limit_is_stopped_by_the_server stdout ----
the server must terminate an unbounded resend loop, not follow it

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 27 filtered out; finished in 0.05s
```

**Confirmed.** With both points removed the test exhausted its hard cap of `16 * 3 = 48` resends without ever receiving a refusal — the pre-fix behaviour D-113-L describes, reproduced. The cap is why this control fails in 0.05 s instead of hanging the suite; it is deliberately a multiple of the ceiling so a regression producing a saturating counter would still be visible before the cap is reached.

Note that `a_flow_within_the_client_default_limit_is_unaffected` passes under **all three** controls. That is correct and worth stating: it is a guard against the ceiling being *too tight*, so removing the ceiling cannot make it fail. It is falsifiable in the other direction — lowering `MAX_MRTR_ROUNDS` below 9 breaks it.

## Verification

| Check | Result |
|---|---|
| `cargo test --features full --lib server::core::tests` | **85 passed**, 0 failed |
| `cargo test --features full --lib server::core::tests::mrtr` | **43 passed**, 0 failed (7 new) |
| `cargo test --features full --test v2_mrtr` | **29 passed**, 0 failed (2 new) |
| `cargo test --features full --test v2_mrtr_ingress` | **10 passed**, 0 failed (unchanged) |
| `cargo test --features full --test v2_bounded_reads_tripwire` | **13 passed**, 0 failed (113-21's tripwire, unweakened) |
| `make lint` (pedantic + nursery, the real gate) | **exit 0** |
| `make check-todos` | no technical-debt comments |
| `make quality-gate` (background job, polled) | **exit 0** — 246 suites, **4353 passed, 0 failed** |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks: 223 pass, 30 skip — no semver update required** |
| `git diff --stat` scope | only `src/server/core.rs` and `tests/v2_mrtr.rs` |

Two pedantic lints bit during execution and are worth recording, since the plan's own `-D clippy::all` verification command does **not** catch either:

- `clippy::doc_link_code` on the new verdict-table rows — a code span adjacent to an intra-doc link (`` `c.round >= ` ``[`MAX_MRTR_ROUNDS`]). Reworded to "whose round is at or past".
- `clippy::cast_possible_wrap` on `u64 as i64` for the JSON-RPC ids in the loop test. Removed by letting `json!` take the `u64` directly.

This is the third consecutive plan in this wave to be caught by the pedantic/nursery gap between the plan's stated clippy invocation and `make lint`. Running `make lint` remains mandatory.

## Deviations from Plan

**None that changed scope.** Two small honesty corrections inside the plan's own frame:

1. **NC-1 needed a supplementary probe.** The plan predicted that with A disabled "the handler-invocation-count assertion FAILS". It would — but the status assertion fires first, so the primary NC-1 run does not display it. Rather than report the prediction as confirmed on evidence that did not show it, the probe above was run to obtain the 17-vs-16 measurement directly. The plan's claim is correct; the primary control simply could not demonstrate it.

2. **The 8-round guard uses nine requests, and the summary says so.** `TOOL_CLIENT_DEFAULT_DEPTH` completes once its sealed counter reaches 8, so the exchange is one initial call plus eight resends, the deepest sealed MRTR round is 8, and the handler runs nine times. A default-limited `pmcp::Client` would send only eight requests, so the guard drives one request *further* than the client-side bound it protects — strictly stronger, and the arithmetic is written into the test's rustdoc so it cannot be misread as an off-by-one.

## Threat Register Disposition

| Threat ID | Disposition | Evidence |
|---|---|---|
| T-113-110 (DoS, unbounded resend loop) | **mitigated** | `a_client_that_ignores_its_own_round_limit_is_stopped_by_the_server`; NC-3 shows the loop unbounded without the fix |
| T-113-111 (EoP, counter enforced by the constrained party) | **mitigated** | the bound is server-side; the client limit is now a courtesy |
| T-113-112 (Tampering, reset the bound via expiry) | **mitigated** | `Verdict::Expired` at/past the ceiling is refused, pinned by `round_ceiling_refuses_every_authentic_verdict_at_the_ceiling` |
| T-113-113 (Tampering, reset via unknown key id) | **accepted** | documented in the verdict-table rustdoc with its reason; `unknown_key_still_resets_to_round_zero_under_the_ceiling` |
| T-113-114 (Repudiation, `mrtr_round` saturating at 255) | **mitigated** | `no_authentic_round_can_reach_saturation` proptest over the full `u8` range |
| T-113-115 (DoS, a future refactor dropping the ingress bound) | **mitigated** | the mint-site backstop; its non-redundancy measured by NC-1, its insufficiency-alone by NC-2 |

## Threat Flags

No new security-relevant surface was introduced. Both changes are refusals on an existing code path; no new endpoint, auth path, file access or schema at a trust boundary.

## Known Stubs

None.

## What This Does NOT Close

- **HTTP-02 / HTTP-03 remain `[~]`.** `.planning/REQUIREMENTS.md` was not edited and no checkbox was flipped — the STATE.md publication gate binds independently of this plan, and both requirements are still "pending final schema" for the 2026-07-28 recheck.
- **Per-server configurability of the ceiling is deliberately deferred**, with the reason recorded in the constant's rustdoc. It is additive and cannot reintroduce the defect, because the enforcement point now exists.
- **Nothing here bears on D-113-M** (`write_canonical`'s depth cap collapsing distinct params to one AAD), which remains open and unowned.

## Self-Check: PASSED

- `src/server/core.rs` — FOUND, contains `MAX_MRTR_ROUNDS`
- `tests/v2_mrtr.rs` — FOUND, contains both new test names
- commit `6f1a44b6` — FOUND (`feat(113-24): give the MRTR round counter a server-side ceiling`)
- commit `4f045462` — FOUND (`test(113-24): prove a client ignoring its own round limit is refused`)
- working tree clean for `src/` and `tests/` after all negative controls were reverted
