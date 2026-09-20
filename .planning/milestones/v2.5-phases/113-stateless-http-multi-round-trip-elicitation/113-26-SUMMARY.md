---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 26
subsystem: security
tags: [mrtr, requestState, aead, replay-prevention, canonicalization, depth-cap, mcp-2026-07-28]

# Dependency graph
requires:
  - phase: 113 (plan 03)
    provides: "`RequestBinding`, its AAD layout and the clause-5a/5b/5c table this plan makes a two-state row of"
  - phase: 113 (plan 06)
    provides: "`mrtr_ingest` / `MrtrIngest` verdict table — refusal point 1 adds a row to it"
  - phase: 113 (plan 09)
    provides: "`mrtr_egress` steps (1)-(4), `fail_mrtr_egress` and `seal_input_required`'s `Err(&'static str)` channel that (3b) and its backstop reuse"
  - phase: 113 (plan 24)
    provides: "the two-point enforcement structure (necessary point + unreachable-by-construction backstop) this plan mirrors, and `MAX_MRTR_ROUNDS` in the same functions"
  - phase: 113 (plan 25)
    provides: "`SecretKey` and the by-reference `resolve_codec_at_build` in `request_state.rs`, left untouched"
provides:
  - "a FALLIBLE canonicalizer — the `\"__mrtr_depth_capped__\"` marker is deleted from the tree, so two distinct requests can no longer share one AAD (D-113-M closed)"
  - "a fallible `RequestBinding::from_request`, the single constructor, which forces every caller to decide what an unidentifiable request means to it"
  - "two typed `INVALID_PARAMS` refusals — at the verify path and at the mint path — plus a documented unreachable-by-construction mint backstop"
  - "an end-to-end proof in two halves, and a MEASURED integration-side mirror of the `pub(crate)` depth cap"
affects: [HTTP-02, HTTP-03, 113-27, 113 phase re-verification, 114 (Tasks reuses the MRTR identity pattern)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "a bound whose behaviour AT the bound is a security decision states both halves in the constant's own rustdoc: why the bound exists (stack safety) and why it refuses rather than substitutes (aliasing)"
    - "a failure message rendered for the NEGATIVE CONTROL: the aliasing regression prints hex digests, so the control's failure output IS the evidence rather than requiring a separate probe"
    - "a `pub(crate)` bound an integration test cannot name is MEASURED by walking the public seam until it refuses, and the measurement is asserted against the mirror"

key-files:
  created: []
  modified:
    - src/types/mrtr.rs
    - src/server/request_state.rs
    - src/server/core.rs
    - src/server/mod.rs
    - src/server/builder.rs
    - src/testing/mod.rs
    - tests/v2_mrtr_ingress.rs

key-decisions:
  - "REFUSE at the cap rather than descend past it: removing the cap would reintroduce the unbounded recursion T-113-14 capped, trading a confidentiality/integrity hole for a denial-of-service one. Both stay closed."
  - "`CanonicalDepthExceeded` is a `pub(crate)` STRUCT, not a variant on any `pub` enum — a new public type or a widened public enum would be a semver event under this milestone's additive-2.x constraint. semver-checks stays 223/223."
  - "The ingress refusal fires BEFORE `codec.verify`, so a request whose identity cannot be computed is not granted a verification attempt at all."
  - "The egress refusal is `INVALID_PARAMS`, not `INTERNAL_ERROR`: the client's params caused it, and the existing `INTERNAL_ERROR` mint-failure channel is for server bugs."
  - "The refusal MESSAGE is specific, not the generic `MRTR_REJECT_MESSAGE`. Genericity buys secrecy only where there is something to keep secret; a nesting depth is client-chosen and client-measurable, so it is not an authentication oracle."
  - "`MAX_CANONICAL_DEPTH` was widened from private to `pub(crate)` so `server::core`'s tests pin the boundary BY NAME rather than re-spelling `64` — a `pub(crate)` const is invisible to semver."
  - "`fuzz_support` folds the new error into the EXISTING `VERDICT_UNAVAILABLE` (4) rather than adding a discriminant, which would invalidate every archived crash artifact's expected output."
  - "HTTP-02 and HTTP-03 stay [~]; .planning/REQUIREMENTS.md was not edited and no checkbox was flipped."

patterns-established:
  - "Pattern: reproduce the vulnerability against the UNFIXED tree first and record the artifact (here, the literal colliding digest and the accepting `Verdict::Ok`), so the negative control later has something to be compared against rather than merely predicted."
  - "Pattern: when a fix is wire-visible, ship a BLAST-RADIUS test that proves the unaffected path is unaffected, and state the boundary as a number an operator can check their payloads against."

requirements-completed: []  # HTTP-02 / HTTP-03 remain [~] — the STATE.md publication gate forbids flipping them this round

# Metrics
duration: 108min
completed: 2026-07-27
---

# Phase 113 Plan 26: Delete the AAD Collision Summary

**`write_canonical` substituted a fixed marker for everything below depth 64, so two `tools/call`s that agreed to depth 64 and differed below it produced the same 32-byte digest — measured, both `1bfce28e6995b41583047d92ab099f4b86329e5e2566ce1dc149655b555698f5` — and a `requestState` minted for one was accepted on the other over live HTTP, handing the second request the first's continuation. The marker is deleted, the canonicalizer refuses instead, and both the verify and the mint path fail closed with a typed `-32602`.**

## Performance

- **Duration:** ~108 min
- **Started:** 2026-07-27T10:30:45Z
- **Completed:** 2026-07-27T12:19:00Z
- **Tasks:** 3 (3 commits + this metadata commit)
- **Files modified:** 7

## Task Commits

1. **Task 1: Make the canonicaliser fallible and delete the aliasing marker** — `323b2e1a` (feat)
2. **Task 2: Fail closed at both the verify path and the mint path** — `aadfc697` (feat)
3. **Task 3: End-to-end cross-request proof and the negative control** — `18f7b4ad` (test)

## The vulnerability, reproduced against the UNFIXED tree

Recorded verbatim BEFORE any source change, so the negative control later had a
measurement to be compared against rather than a prediction to be confirmed. Two
`tools/call` params identical to depth 64 and differing below it — `"SECRET-A"`
vs `"SECRET-B"` at 64 levels of `{"n": …}` nesting inside `arguments`:

```
running 1 test
D-113-M request A digest = 1bfce28e6995b41583047d92ab099f4b86329e5e2566ce1dc149655b555698f5
D-113-M request B digest = 1bfce28e6995b41583047d92ab099f4b86329e5e2566ce1dc149655b555698f5
D-113-M digests are equal = true
test types::mrtr::tests::dm_reproduce_the_aliasing_collision ... ok
```

**The two digests are byte-identical.** And because the digest IS the AAD, the
consequence is not theoretical — the production codec accepts the cross-request
token:

```
running 1 test
D-113-M token minted for A, verified against B -> Ok(Continuation { state: Object {"step": Number(1)}, exp: 1300, round: 0 })
test server::request_state::tests::dm_token_minted_for_a_verifies_against_b ... ok
```

`Verdict::Ok`, not `AuthFailed`. Both reproductions were reverted with
`git checkout --` before the fix was written; neither is in the tree.

## What changed

### `src/types/mrtr.rs` — the canonicalizer

`write_canonical` and `salient_param_digest` are now fallible; the
`"__mrtr_depth_capped__"` literal is deleted from the source rather than
worked around, because as long as it exists the branch can be reintroduced.
`grep -rn "__mrtr_depth_capped__"` over the whole repo (excluding `.planning/`,
which records it) returns **nothing**.

`CanonicalDepthExceeded { depth, max }` carries the bound so `core.rs` can log
it without the constant becoming public, and its `Display` names the BOUND only,
never attacker-controlled content — the same discipline `MrtrParseError` follows.

`MAX_CANONICAL_DEPTH`'s rustdoc now states both halves: the cap exists for
STACK SAFETY (T-113-14, and `serde_json`'s own default recursion limit of 128
makes the 64–128 band reachable over the wire), and what changed is the
behaviour AT it, from aliasing to refusing.

### `src/server/request_state.rs` — the binding

`RequestBinding::from_request` returns `Result`. It is the ONLY constructor,
which is what forces every caller to decide. The clause table gained a third
column:

| Clause | Binding | Enforced or refused |
|--------|---------|---------------------|
| 5a — authenticated principal | `principal` | enforced |
| 5b — short expiry | the ttl in `Continuation::exp` | enforced |
| 5c — identifier for the originating request | `method` + `param_digest` | **enforced, or the request is REFUSED** |

Two-state on purpose: there is no third state in which a binding exists carrying
a digest that does not identify its request.

### `src/server/core.rs` — the two refusals

| | Where | Code | Message | Handler runs? |
|---|---|---|---|---|
| **Refusal 1 — verify path** | `mrtr_ingest`, before `codec.verify` | `INVALID_PARAMS` | `MRTR_UNCANONICALIZABLE_MESSAGE` | **no** |
| **Refusal 2 — mint path** | `mrtr_egress` step (3b), after the capability precheck, before the mint | `INVALID_PARAMS` | `MRTR_UNCANONICALIZABLE_MESSAGE` | yes (it already ran; nothing reaches the wire) |
| Backstop | `seal_input_required` | `INTERNAL_ERROR` | `MRTR_UNCANONICALIZABLE_INVARIANT_MESSAGE` | — unreachable by construction |

Refusal 1 fails **closed**: a token was presented and the request's identity
cannot be computed, so it is not granted a verification attempt.

Refusal 2 is `INVALID_PARAMS` and not `INTERNAL_ERROR` deliberately — the
condition is caused by the client's params, and the `INTERNAL_ERROR`
mint-failure channel next to it is for server bugs. That distinction is asserted,
not merely commented (`egress_refuses_to_mint_for_an_uncanonicalizable_request`).

The backstop's own `INTERNAL_ERROR` classification is correct **precisely
because** reaching it means step (3b) was moved or deleted — the same two-point
structure, and the same reasoning, as 113-24's round-ceiling backstop.
`the_depth_refusal_precedes_every_mint_precondition` proves the ordering
structurally: step (3b) refuses with **no codec configured**, which the mint
itself would need.

### Where each caller decides

| Caller | Decision |
|---|---|
| `mrtr_ingest` (production, verify) | `MrtrIngest::Reject { INVALID_PARAMS, MRTR_UNCANONICALIZABLE_MESSAGE }` + `tracing::warn!` on `mcp.mrtr` |
| `mrtr_egress` step (3b) (production, mint) | `fail_mrtr_egress(INVALID_PARAMS, …)` + `tracing::warn!` |
| `seal_input_required` (production, backstop) | existing `Err(&'static str)` → `INTERNAL_ERROR` |
| `testing::mint_request_state` / `open_request_state` | `.ok()?` — folded into the existing `Option` return, so **both public signatures are byte-unchanged** |
| `fuzz_support::verify_bytes` | `VERDICT_UNAVAILABLE` (see below) |
| unit/property tests | `.expect("…inside the canonical depth cap")` on shallow fixtures |

**`fuzz_support` uses the EXISTING `VERDICT_UNAVAILABLE` (4), not a new
discriminant.** Its params are fixed and two levels deep, so the branch is
unreachable there; it is handled with a `let … else` rather than unwrapped
because a panic in a fuzz target is a false crash artifact. Folding it into
`VERDICT_UNAVAILABLE` cannot mask an `Ok` — the target's invariant is "never
`VERDICT_OK` for input `mint` did not produce", and both conditions mean no
verification happened at all — while a new discriminant would have invalidated
every archived crash artifact's expected output. `cargo build --features
"full,fuzzing"` is green.

## The boundary, measured on both sides

| Level | Accepted | Refused |
|---|---|---|
| `write_canonical` raw depth | a value whose leaf sits at exactly `MAX_CANONICAL_DEPTH` (64) | depth 65 → `Err(CanonicalDepthExceeded { depth: 65, max: 64 })` |
| `tools/call` `arguments` nesting, through `salient_param_digest` | **63** levels | **64** levels |

The one-level offset is the salient whitelist wrapper: `salient_params` builds
`{"name": …, "arguments": …}` at canonical depth 0, so the `arguments` VALUE is
depth 1. Both sides of both boundaries are asserted
(`canonical_depth_boundary_admits_the_cap_and_refuses_one_past_it`,
`the_digest_boundary_accounts_for_the_salient_wrapper_level`) — an off-by-one
here silently narrows or widens what the server accepts on the wire.

Arrays count toward the cap exactly as objects do
(`arrays_count_toward_the_canonical_depth_cap`); a nesting bound that saw only
one container kind would be trivially bypassed.

`tests/v2_mrtr_ingress.rs` cannot name the `pub(crate)` constant, so it mirrors
it — and **measures** the mirror rather than commenting it.
`the_mirrored_depth_cap_matches_the_live_one` walks `mint_request_state` (which
wraps the production codec and the production binding) up the depth ladder until
it refuses, and asserts the first refusing depth equals the mirror, with a
`CANONICAL_DEPTH_CAP * 3` hard scan bound so a regression that removed the cap
fails fast instead of hanging the suite. Observed first refusing depth: **64**.

## Blast radius of the wire-visible refusal — stated plainly

This change **rejects requests that previously succeeded**. Precisely:

**What is affected.** A `tools/call` / `prompts/get` / `resources/read` on a
**v2** connection to a server **with a `requestState` codec configured**, whose
salient params nest 64 or more levels, AND which either (a) presents a
`requestState`, or (b) has a handler that returns `input_required`. Case (a)
gets `-32602` at HTTP 400 and the handler is never invoked. Case (b) gets
`-32602` after the handler ran, with no `requestState` and no `inputRequests` on
the wire.

**What is NOT affected — proven, not asserted.**
`a_deep_request_that_never_touches_mrtr_is_unaffected` is the blast-radius test.
An ordinary deep-`arguments` `tools/call` that presents no token computes no
digest at all: `mrtr_ingest` short-circuits to `Inert` before the binding is
built, and the same request on a **v1** connection is equally `Inert`. So the
change is confined to requests that MINT or PRESENT a continuation. Non-MRTR
methods (`tools/list`, `resources/list`, everything else) never reach the
canonicalizer on any era.

**What depth do real payloads reach?** Measured rather than guessed:

| Corpus | Max JSON nesting depth |
|---|---|
| The MCP 2026-07-28 draft schema, whole document | **12** |
| Its deepest single definition (`UnsupportedProtocolVersionError`) | **10** |
| `ElicitResult` — the MRTR payload type itself | **7** |
| Deepest of all **109** `.json` files in this repo (a CDK synth artifact, not a protocol payload) | **18** |

The bound is **63**, i.e. roughly **5x** the deepest structure the protocol's own
schema defines and **3.5x** the deepest JSON document anywhere in this
repository. A payload that trips this is not a schema-shaped tool argument; it is
either machine-generated recursion or a probe.

**Did any existing test or example exceed the cap?** **No.** `make quality-gate`
is exit 0 across **249 test-result lines, 4408 passed, 0 failed**, including
every example. The only in-repo values that reach the cap are the ones this plan
constructed deliberately. Symmetrically, nothing was made to pass by widening the
bound — `MAX_CANONICAL_DEPTH` is unchanged at 64.

**Also worth an operator's attention:** the deepest *accepted* request still
costs 63 levels of recursion in `write_canonical` on a peer-chosen input. That is
unchanged by this plan — it is the T-113-14 property the cap already bought — but
it is the reason the cap was not simply removed.

## The depth asymmetry, pinned

`input_responses_are_depth_bounded_at_ingress_but_arguments_are_not` records why
the canonical cap is load-bearing rather than redundant:

- an over-deep `inputResponses` entry is refused at INGRESS by
  `check_input_response_bounds` (`MAX_INPUT_RESPONSE_DEPTH` = 32) and never
  reaches dispatch;
- `arguments` at the SAME depth pass every ingress bound — `extract_mrtr_params`
  reports no problem — and reach the canonicalizer unfiltered.

So for `arguments` the canonical cap is not a second line of defence, it is the
ONLY one, which is what makes its behaviour at the bound a security decision. The
two bounds' ordering is asserted at compile time
(`const _: () = assert!(MAX_INPUT_RESPONSE_DEPTH < MAX_CANONICAL_DEPTH);`), so
the tighter ingress bound can never quietly become the thing that saves
`arguments`.

## Negative control — recorded verbatim

Run by restoring the marker branch in `write_canonical` (push
`"\"__mrtr_depth_capped__\""` and return `Ok(())`) against the landed code, and
reverted from a byte-for-byte backup afterwards. The aliasing regression renders
its digests as hex precisely so that **the control's failure output IS the
evidence**.

### NC part 1 — the aliasing regression fails with two EQUAL digests

```
running 3 tests
test types::mrtr::tests::params_differing_only_below_the_depth_cap_can_never_share_a_digest ... FAILED
test types::mrtr::tests::canonical_depth_boundary_admits_the_cap_and_refuses_one_past_it ... FAILED
test types::mrtr::tests::the_digest_boundary_accounts_for_the_salient_wrapper_level ... FAILED

---- types::mrtr::tests::params_differing_only_below_the_depth_cap_can_never_share_a_digest stdout ----

thread 'types::mrtr::tests::params_differing_only_below_the_depth_cap_can_never_share_a_digest' (58803611) panicked at src/types/mrtr.rs:1820:9:
over-deep params must be REFUSED, not digested.
  A      = 1bfce28e6995b41583047d92ab099f4b86329e5e2566ce1dc149655b555698f5
  B      = 1bfce28e6995b41583047d92ab099f4b86329e5e2566ce1dc149655b555698f5
  equal? = true

---- types::mrtr::tests::canonical_depth_boundary_admits_the_cap_and_refuses_one_past_it stdout ----

thread 'types::mrtr::tests::canonical_depth_boundary_admits_the_cap_and_refuses_one_past_it' (58803610) panicked at src/types/mrtr.rs:1857:9:
assertion `left == right` failed: one level past the cap must REFUSE
  left: Ok(())
 right: Err(CanonicalDepthExceeded { depth: 65, max: 64 })

---- types::mrtr::tests::the_digest_boundary_accounts_for_the_salient_wrapper_level stdout ----

thread 'types::mrtr::tests::the_digest_boundary_accounts_for_the_salient_wrapper_level' (58803612) panicked at src/types/mrtr.rs:1886:9:
one deeper must be refused

test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 1625 filtered out; finished in 0.01s
```

**`equal? = true`, and the digest is
`1bfce28e6995b41583047d92ab099f4b86329e5e2566ce1dc149655b555698f5` — byte-for-byte
the value measured against the unfixed tree before any source change.** That
match is what makes this a control rather than a coincidence: the same two
requests, the same collision, produced twice by independent routes.

### NC part 2 — the end-to-end test's first half fails because A now mints

```
running 12 tests
test a_token_minted_for_one_request_is_refused_on_a_request_differing_below_the_depth_cap ... FAILED
test the_mirrored_depth_cap_matches_the_live_one ... FAILED

---- a_token_minted_for_one_request_is_refused_on_a_request_differing_below_the_depth_cap stdout ----

thread '…' panicked at tests/v2_mrtr_ingress.rs:745:5:
assertion `left == right` failed: an over-deep request must be refused at the mint path, body was {"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\"answer\":\"need-input\"}"}],"isError":false,"inputRequests":{"user_name":{"method":"elicitation/create","params":{"mode":"form","message":"What is your name?","requestedSchema":{"type":"object"}}}},"requestState":"CFz6wJ97Fxp7LtlN67459buURgkI_lkiMONihf1dWdyLSURwiQIIvLkOFlXCiE-_yeE4IxjrFKpnIvdxPFVJNGiBVbD6ClGDay6aCBA1J9wCujnL","resultType":"input_required","_meta":{…}}}
  left: Null
 right: -32602

---- the_mirrored_depth_cap_matches_the_live_one stdout ----

thread '…' panicked at tests/v2_mrtr_ingress.rs:703:10:
the depth cap must refuse SOMEWHERE below 3x the mirrored value

test result: FAILED. 10 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.73s
```

The over-deep request **minted a `requestState`**, visible in the failure body.
The mirror measurement also fails, which is what makes it a real measurement.

### NC part 3 — supplementary probe: the harm itself, on a live socket

NC parts 1 and 2 show the collision and the mint. Neither shows the **acceptance**
— so a probe was run with the marker still restored, driving a real
`StreamableHttpServer` over loopback (reverted afterwards along with the marker):

```
NC PROBE: request A minted requestState = CFz6wJ97Fxp7ySpHAt4_oaWLT6_MtsPBqRfelk_3fKjX0_u4d0G93U25EMJp148V_y0M6YT4MKU8trg6518nMXLGLd0_dfH_F93SfVlnYL37EQtq
NC PROBE: same token presented on request B -> {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"answer\":\"resumed\"}"}],"isError":false,"resultType":"complete","_meta":{"io.modelcontextprotocol/serverInfo":{"name":"v2-mrtr-harness","version":"1.0.0"}}}}
NC PROBE: handler observations = [Observed { continuation: None, round: None, had_input_responses: false }, Observed { continuation: Some(Object {"step": Number(1)}), round: Some(1), had_input_responses: true }]
```

**Request B — a different request — received request A's continuation
`{"step": 1}` at round 1 and the server answered `"resultType":"complete"` with
`"answer":"resumed"`.** That is D-113-M's stated failure reproduced over real
HTTP, and it is what the fix closes.

All three controls were reverted. `src/types/mrtr.rs` was restored from a
byte-for-byte backup (shasum verified) and the probe test deleted;
`grep -rn "nc_probe\|NC PROBE\|__mrtr_depth_capped__" src/ tests/` returns
nothing.

## The end-to-end proof, in two halves

`a_token_minted_for_one_request_is_refused_on_a_request_differing_below_the_depth_cap`
drives a real `StreamableHttpServer` over a loopback socket:

- **Half 1 (the fix).** A and B identical to the cap, differing below. A can no
  longer mint at all — `-32602`, no `result`, no `requestState` anywhere in the
  body — so there is nothing to cross-verify. A token *presented* on the deep
  request is refused before verification and the handler count stays put.
- **Half 2 (the control).** A' and B' differing ABOVE the cap mint and
  cross-reject exactly as they always did. The recovered token is **opened with
  the server's own key** (113-11's pattern) to prove it is the real minted
  continuation rather than trusted for having round-tripped, is shown NOT to open
  against B', and the HTTP replay onto B' is `-32602` with the handler never
  reached.

Both halves are needed: without half 2, half 1 would be equally consistent with
the server having simply stopped minting.

## Verification

| Check | Result |
|---|---|
| `grep -rn "__mrtr_depth_capped__"` (whole repo, excl. `.planning/`) | **no matches** |
| `cargo test --features full --lib -- types::mrtr::tests` | **60 passed**, 0 failed (**7 new**) |
| `cargo test --features full --lib -- server::core::tests` | **91 passed**, 0 failed (**6 new**; 113-24's 85 intact) |
| `cargo test --features full --lib -- server::request_state` | **45 passed**, 0 failed (113-25's, unchanged) |
| `cargo test --features full --test v2_mrtr_ingress` | **12 passed**, 0 failed (**2 new**) |
| `cargo test --features full --test v2_mrtr` | **29 passed**, 0 failed (113-24's, unchanged) |
| `cargo test --features full --test v2_bounded_reads_tripwire` | **13 passed**, 0 failed (113-21's tripwire, unweakened) |
| `cargo build --features "full,fuzzing"` | clean — the fuzz seam still compiles |
| `cargo clippy --features "full" --lib --tests -- -D clippy::all` | clean |
| **`make lint`** (pedantic + nursery, the real gate) | **exit 0** — after fixing two lints `-D clippy::all` did not catch (below) |
| `cargo fmt --all -- --check` | clean |
| `make check-todos` | no technical-debt comments |
| `make quality-gate` (background job, polled) | **exit 0** — **249 test-result lines, all `ok`, 4408 passed, 0 failed** |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks: 223 pass, 30 skip — no semver update required** |
| `make wasm-build` | `✓ WASM build complete`; **82 → 83** warnings, the one new being `CanonicalDepthExceeded is never constructed` — the same pre-existing dead-code class as `MAX_CANONICAL_DEPTH`, `write_canonical` and `salient_param_digest`, all three of which **already warned on wasm before this plan** (measured at `HEAD~3`). The wasm server has no MRTR (D-14). |
| `Cargo.toml` / `Cargo.lock` | **untouched** — zero new dependencies |
| `git diff --stat` scope | only the 7 files listed above |

Totals were read from the raw gate log with `awk` and via `$HOME/.cargo/bin/cargo`,
since the rtk shell proxy compresses `test result:` lines.

**`make lint` caught two lints the plan's own `-D clippy::all` command does
not** — the fourth consecutive plan in this wave to hit the pedantic/nursery gap:

- `clippy::assertions_on_constants` on `assert!(MAX_INPUT_RESPONSE_DEPTH < MAX_CANONICAL_DEPTH)`
  inside a test body. Fixed by promoting it to a module-level
  `const _: () = assert!(…)`, which is **strictly stronger** — the relationship is
  now checked at compile time rather than at test time.
- `clippy::format_collect` on `.map(|b| format!("{b:02x}")).collect()` in the hex
  renderer. Rewritten as `.map(…).join("")`.
- Also `clippy::doc_lazy_continuation` on the `3b.` ordered-list item added to
  `mrtr_egress`'s rustdoc; reworded as an indented `**(3b)**` sub-item of step 3.

**Running `make lint` remains mandatory.**

## Threat Register Disposition

| Threat ID | Disposition | Evidence |
|---|---|---|
| T-113-122 (Spoofing, a token minted for A accepted on B) | **mitigated** | `params_differing_only_below_the_depth_cap_can_never_share_a_digest`; NC part 3 exhibits the acceptance itself over HTTP without the fix |
| T-113-123 (Tampering, clause 5c unenforced for a class of requests) | **mitigated** | both refusal points typed `INVALID_PARAMS`; `an_uncanonicalizable_request_presenting_a_token_is_refused`, `egress_refuses_to_mint_for_an_uncanonicalizable_request` |
| T-113-124 (DoS, removing the cap to fix the collision) | **accepted, avoided by design** | the cap is unchanged at 64; the decision and its reasoning are in the constant's rustdoc, and `canonical_depth_boundary_admits_the_cap_and_refuses_one_past_it` pins that it still bounds the recursion |
| T-113-125 (DoS, the refusal breaking ordinary deep calls) | **mitigated** | `a_deep_request_that_never_touches_mrtr_is_unaffected` (v2 and v1 legs); blast-radius numbers above |
| T-113-126 (Tampering, reintroducing the marker) | **mitigated** | the literal is deleted from the tree, and NC part 1 shows the regression fails the moment it returns |

## Threat Flags

None. No new network endpoint, auth path, file access pattern or schema at a
trust boundary. Both changes are refusals on existing code paths.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] A transitional block at core.rs's two production sites so Task 1's commit compiles**

- **Found during:** Task 1
- **Issue:** Making `RequestBinding::from_request` fallible (Task 1's file) breaks
  `core.rs`'s two production call sites (Task 2's file). A commit that does not
  build is not an atomic commit.
- **Fix:** `323b2e1a` carries two clearly-labelled `TRANSITIONAL (D-113-M, task 1
  of 2)` blocks that already fail CLOSED (so the tree is never in a state where
  the hole is open); `aadfc697` replaces them with the typed refusals, their own
  message constants and the `tracing::warn!`s, and deletes both labels.
- **Verification:** both commits build and lint clean; the final tree has zero
  occurrences of `TRANSITIONAL`. This is the same pattern 113-25 deviation 3 used.

**2. [Rule 2 - Missing critical functionality] Two message constants, not one**

- **Found during:** Task 2
- **Issue:** The plan's action step 4 asks for one constant,
  `MRTR_UNCANONICALIZABLE_MESSAGE`. But step 3 also asks the `seal_input_required`
  backstop to route through the `Err(&'static str)` channel, which surfaces as
  `INTERNAL_ERROR` — a different situation with a different meaning to an
  operator, and sharing one string would make the two indistinguishable in logs.
- **Fix:** `MRTR_UNCANONICALIZABLE_INVARIANT_MESSAGE` alongside it, exactly as
  113-24 pairs `MRTR_ROUND_CEILING_MESSAGE` with
  `MRTR_ROUND_CEILING_INVARIANT_MESSAGE`, and asserted distinct in
  `the_depth_refusal_precedes_every_mint_precondition`.
- **Committed in:** `aadfc697`

**3. [Rule 3 - Blocking] `MAX_CANONICAL_DEPTH` widened to `pub(crate)`**

- **Found during:** Task 2
- **Issue:** The plan requires `core.rs`'s tests to pin the boundary, but the
  constant was private to `types::mrtr`, so those tests could only re-spell `64`
  — which is exactly how two halves of a bound drift apart.
- **Fix:** `pub(crate)`, with the reason in its rustdoc. Invisible to semver
  (223/223 unchanged).
- **Committed in:** `aadfc697`

**4. [Rule 1 - Bug] `Result` in `request_state.rs` is the crate alias**

- **Found during:** Task 1
- **Issue:** `use crate::error::{Error, Result}` shadows `std::result::Result` in
  that module, so `-> Result<Self, CanonicalDepthExceeded>` was `E0107` (type
  alias takes 1 generic argument) plus an `E0277` on the `?`.
- **Fix:** `-> std::result::Result<Self, CanonicalDepthExceeded>`.
- **Committed in:** `323b2e1a`

### Process deviation (no code impact)

**5. An accidental `git stash push --include-untracked`, immediately reverted.**
While measuring the wasm warning baseline I reached for `git stash` — which the
executor's own destructive-git prohibition names, and correctly so. It ran on the
main working tree (not a worktree), captured this session's unrelated ambient
modifications (`.planning/config.json`, `.pmat/*`, `pmcp-course/*` and the
untracked dirs), and was popped in the very next command. `git status --short` is
byte-identical to the session's starting state and the four pre-existing stash
entries are untouched. The baseline was then measured the SAFE way — file backup,
`git checkout HEAD~3 -- <files>`, measure, `git checkout HEAD -- <files>`,
`diff -q` to confirm restoration — which is the method that should have been used
first. Recorded rather than quietly dropped because the prohibition exists for a
reason and a near-miss is worth the phase record.

---

**Total deviations:** 4 auto-fixed (1× Rule 1, 1× Rule 2, 2× Rule 3) + 1 process
**Impact on plan:** No scope creep. All four are mechanical consequences of the
plan's prescribed shape meeting the real gate and the one-commit-per-task
requirement.

## Issues Encountered

None beyond the deviations above.

## Known Stubs

None.

## Requirements

**HTTP-02 and HTTP-03 stay `[~]`.** `.planning/REQUIREMENTS.md` was NOT edited and
no checkbox was flipped — the STATE.md publication gate forbids flipping
HTTP-01..09 / CLNT-01/02/05 this round, and this plan honours it.
`requirements-completed` in the frontmatter is deliberately empty for the same
reason.

## What This Does NOT Close

- **The remaining recursion is still recursion.** A request at the cap costs 63
  levels of `write_canonical` recursion on peer-chosen input. That is the
  pre-existing T-113-14 posture, unchanged; an iterative canonicalizer would
  remove it entirely and is a candidate for a later plan, but it is not what
  D-113-M asked for and would have been a much larger diff on a security-critical
  function.
- **The cap is not configurable.** Same reasoning 113-24 recorded for
  `MAX_MRTR_ROUNDS`: a knob is not what closes the item, an enforced bound is.
  Making it tunable later is additive and cannot reintroduce the defect.
- **Still open and unowned in this phase:** D-113-O (server ingress typing
  `inputResponses` by untagged guess), D-113-Q (`sse_optimized.rs:266` unbounded
  `reqwest` body — enumerated in the tripwire allowlist with a written NOT-BOUNDED
  justification), D-113-R (`drain_complete_lines` quadratic; blocks HTTP-09
  substantively).
- No blockers introduced.

## Next Phase Readiness

- **113-27 (wave 3) edits `src/types/mrtr.rs`, `src/server/core.rs` and
  `src/server/request_state.rs` after this plan.** It inherits a fallible
  `salient_param_digest` / `RequestBinding::from_request`; any new caller it adds
  must decide explicitly what an unidentifiable request means to it, and must not
  reintroduce a fallback value at the cap.
- 113-24's round ceiling and 113-25's `SecretKey` plumbing are both intact and
  their tests pass unchanged.

## Self-Check: PASSED

- `src/types/mrtr.rs` — FOUND, contains `MAX_CANONICAL_DEPTH` and `CanonicalDepthExceeded`
- `src/server/request_state.rs` — FOUND, contains the fallible `from_request`
- `src/server/core.rs` — FOUND, contains `salient_param_digest` and both message constants
- `tests/v2_mrtr_ingress.rs` — FOUND, contains both new test names
- commit `323b2e1a` — FOUND (`feat(113-26): make the AAD canonicalizer fallible…`)
- commit `aadfc697` — FOUND (`feat(113-26): refuse an unbindable request at both…`)
- commit `18f7b4ad` — FOUND (`test(113-26): prove a deep request can no longer mint…`)
- `grep -rn "__mrtr_depth_capped__" src/ tests/` — no matches
- `grep -rn "TRANSITIONAL" src/` — no matches
- working tree clean for `src/` and `tests/` after all negative controls were reverted

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-27*
