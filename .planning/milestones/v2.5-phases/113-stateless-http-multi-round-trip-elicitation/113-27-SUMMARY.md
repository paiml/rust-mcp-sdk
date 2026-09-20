---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 27
subsystem: protocol
tags: [mrtr, inputResponses, kind-directed, requestState, aead, mcp-2026-07-28]

# Dependency graph
requires:
  - phase: 113 (plan 02)
    provides: "`InputRequestKind`, `InputRequest::kind()`, `InputResponse::decode_for` and `try_from_value_untagged` — T-113-46's kind-directed claim, which this plan finally makes true on the server"
  - phase: 113 (plan 03)
    provides: "`Continuation`, `RequestStateCodec::mint`/`verify` — the sealed payload the kinds now ride inside"
  - phase: 113 (plan 06)
    provides: "`mrtr_ingest` / `MrtrIngest` / `apply` — the verified path the re-decode is inserted on"
  - phase: 113 (plan 24)
    provides: "`MAX_MRTR_ROUNDS` (16), which turned D-113-O's infinite loop into a bounded-but-wrong one; this plan removes the cause"
  - phase: 113 (plan 26)
    provides: "the fallible `RequestBinding::from_request` / `salient_param_digest`, left untouched; and the 'the only constructor forces every caller to decide' pattern this plan reuses for `mint`"
provides:
  - "the server's own record of which kind it requested under each `inputRequests` key, carried inside the AEAD-sealed continuation and unforgeable"
  - "a kind-directed re-decode at ingress on the VERIFIED path, replacing the untagged guess before the handler is invoked"
  - "a typed `INVALID_PARAMS` refusal for an answer that cannot be the requested kind, and for a key the continuation never requested"
  - "a measured correction to the plan's premise: the literal D-113-O answer is TYPED correctly, not rejected, and that alone closes the loop"
affects: [HTTP-03, 113-28, 114 (Tasks reuses the MRTR identity pattern)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "an `Option<Map>` rather than a bare `Map` when ABSENT and EMPTY are different statements and both are reachable — the sentinel that conflates them is the bug"
    - "what a refusal may NAME is decided by the provenance of the datum, not by convenience: a value read out of the sealed continuation may be echoed, a client-chosen one may not"
    - "a new parameter on a `pub(crate)` constructor, rather than a defaulted field, so every call site has to state a decision it could otherwise omit"
    - "when the plan's predicted failure branch is wrong, MEASURE which branch actually fires and pin both — the correction is the finding"

key-files:
  created: []
  modified:
    - src/types/mrtr.rs
    - src/server/request_state.rs
    - src/server/core.rs
    - src/types/protocol/context.rs
    - src/client/mod.rs
    - src/server/mod.rs
    - src/server/builder.rs
    - src/testing/mod.rs
    - tests/v2_mrtr.rs

key-decisions:
  - "`Continuation.kinds` is `Option<InputRequestKinds>`, NOT a bare map. ABSENT means pre-kinds (degrade to untagged); EMPTY means this build asked for nothing (reject every answer). The plan's 'empty means degrade' rule conflated them, and the empty case is reachable — a handler may signal `input_required` with an empty `inputRequests`."
  - "`mint` takes the kinds as an explicit parameter rather than defaulting them, so every mint site must decide — the same discipline 113-26 established by making `from_request` the only binding constructor."
  - "A `KindMismatch` NAMES its key (read out of the sealed map: server-assigned, bounded by the token); an `Unsolicited` key is never rendered (client-chosen, bounded only by the 256 KiB `inputResponses` total). The plan asked for both to be named, justifying it with 'the key is server-assigned' — a premise that is false for the unsolicited branch."
  - "`MrtrIngest::Reject.message` widened from `&'static str` to `String`, because the mismatch refusal names a runtime key. Both dispatch sites already called `.to_string()`, so the change removed code rather than adding it."
  - "The re-decode sits in `MrtrIngest::apply`, on the `Proceed` arm only — i.e. strictly after the AEAD tag check, so the kinds it enforces against cannot be chosen or altered by the client."
  - "`testing::mint_request_state` mints `None` kinds, keeping its PUBLIC signature byte-unchanged and every existing caller's behaviour byte-identical."
  - "HTTP-03 stays `[~]`; `.planning/REQUIREMENTS.md` was not edited and no checkbox was flipped."

patterns-established:
  - "Pattern: reproduce the defect against the UNFIXED tree first and record it verbatim, so the negative control later has a measurement to be compared against rather than a prediction to confirm (carried from 113-26)."
  - "Pattern: when a fix's *expected* failure branch turns out not to fire, do not bend the implementation to make the plan right — measure why, pin the branch that actually fires AND the branch the plan meant, and record the correction."

requirements-completed: []  # HTTP-03 remains [~] — the STATE.md publication gate forbids flipping it this round

# Metrics
duration: 96min
completed: 2026-07-27
---

# Phase 113 Plan 27: Type inputResponses by Kind Summary

**`src/types/mrtr.rs` typed every client answer with the untagged decoder — Roots, then Sampling, then Elicitation, first shape that fits — so an elicitation answered with an object carrying `action` AND `content`+`model` arrived at the handler as `InputResponse::Sampling`; measured against `9a7024cd`, the handler's `Elicitation` arm fell through and the operation ran 16 resends and 16 handler invocations before dying on a misleading round-limit error. The server's own record of what it requested now rides inside the AEAD-sealed continuation, ingress re-decodes kind-directed after the tag check, and the same exchange completes on the first resend with the handler run exactly twice.**

## Performance

- **Duration:** ~96 min
- **Started:** 2026-07-27T13:05:00Z
- **Completed:** 2026-07-27T14:41:00Z
- **Tasks:** 3 (3 commits + this metadata commit)
- **Files modified:** 9

## Task Commits

1. **Task 1: Carry the requested kinds inside the sealed continuation** — `64de5b15` (feat)
2. **Task 2: Kind-directed re-decode at ingress with a typed mismatch rejection** — `7b47694e` (feat)
3. **Task 3: End-to-end — the loop terminates — plus negative control** — `9b7fedb0` (test)

## The defect, reproduced against the UNFIXED tree

Recorded verbatim BEFORE any source change (both reproductions reverted with
`git checkout --` before the fix was written; neither is in the tree).

### Unit level — the reclassification itself

```
running 1 test
D-113-O server ingress typed the answer under "k" as = Sampling
D-113-O the handler asked for            = Elicitation
D-113-O silently reclassified            = true

thread 'types::mrtr::tests::do_reproduce_the_silent_reclassification' panicked at src/types/mrtr.rs:1253:9:
assertion `left == right` failed: the server must type the answer as the kind it ASKED for
  left: "Sampling"
 right: "Elicitation"
test types::mrtr::tests::do_reproduce_the_silent_reclassification ... FAILED
```

### End to end — the loop, and how 113-24 already bounded it

A handler requesting an elicitation under `"k"`, answered with the overlapping
object, driven over a real `StreamableHttpServer`:

```
D-113-O round 1 asked for keys = ["k"]
D-113-O resend 1 re-elicited again (round climbing)
D-113-O resend 2 re-elicited again (round climbing)
… (resends 3–15 identical) …
D-113-O resend 16 TERMINATED with = {"jsonrpc":"2.0","id":17,"error":{"code":-32602,"message":"this request exceeded the server's multi-round-trip round limit"}}
D-113-O total resends            = 16
D-113-O handler invocations      = 16
```

**The two fixes compose, and neither subsumes the other.** D-113-O's report says
the loop runs "forever with no error anywhere". As of plan 113-24 that is no
longer literally true: `MAX_MRTR_ROUNDS` (16) terminates it. But it terminates it
with a **misleading** `-32602 "this request exceeded the server's
multi-round-trip round limit"` — an error blaming the client's round count for an
answer that was mistyped on round one, after 16 wasted round trips and 16 handler
invocations. The ceiling bounds a loop it cannot diagnose; this plan removes the
cause. That composition is asserted in the negative control below, where 113-24's
round-limit test stays green while all three D-113-O tests fail.

## What changed

### `src/types/mrtr.rs` — the kinds, the raw retention, the re-decode

`InputRequestKind` gains `Serialize`/`Deserialize` with **per-variant `rename`**
(`"elicitation"` / `"sampling"` / `"roots"`), not a container-level `rename_all`.
The spelling travels in exactly one place — inside the sealed continuation — and
never on the public wire, where a kind is still its `wire_method()`. The reason
for the explicit form is in the rustdoc and is not stylistic: a token minted by
one build is presented to another during a rolling deploy, and a re-spelled
variant would make in-flight kinds maps undecodable, which under the
absent-means-pre-kinds rule is a hard failure rather than a degradation.
`the_sealed_kind_spelling_is_pinned` asserts all six directions.

`MrtrRequestParams` gains `input_responses_raw`. A value already forced into the
wrong variant cannot be un-forced, so the re-decode needs the original. The copy
is taken in `extract_input_responses` **after** all four ingress bounds pass, per
entry, with the loop returning on the first violation — so it inherits every one
of them and the worst case it adds is 256 KiB of already-accepted JSON, a bounded
duplication rather than a new retention. `the_ingress_bounds_still_fire_before_the_raw_retention`
feeds an over-count, an over-size entry, an over-deep entry and a
many-medium-values map and asserts the four unchanged `MrtrParseError` variants.

`retype_input_responses_for_kinds` is the entry point, with three outcomes:

| `kinds` | Meaning | Result |
|---|---|---|
| `None` | continuation predates the field | `Ok(None)` — keep the untagged values |
| `Some(map)`, key present and decodable | requested and answered correctly | `Ok(Some(typed))` |
| `Some(map)`, key present, value undecodable | wrong shape for the requested kind | `Err(KindMismatch)` |
| `Some(map)`, key absent | never requested | `Err(Unsolicited)` |

`Ok(None)` is a deliberate third state rather than "return the input unchanged":
it makes the caller's degradation branch explicit instead of implicit.

### `src/server/request_state.rs` — the sealed payload

`Continuation.kinds: Option<InputRequestKinds>`, `#[serde(default)]`.

`mint` takes the kinds as an explicit fourth parameter. That is the 113-26
pattern applied again — `from_request` being the only binding constructor is what
forced every caller to decide what an unidentifiable request meant to it, and an
explicit parameter is what forces every mint site to state whether it can say
which kinds were requested. Passing `None` opts a token into the degradation, so
it has to be a choice, never an omission.

### `src/server/core.rs` — populate at mint, enforce at ingress

`seal_input_required` builds the map from `signal.input_requests` via
`InputRequest::kind()`. This is the one place in the SDK where the requested kinds
are known and therefore the one place they can be sealed. It passes `Some(...)`
**unconditionally, including for an empty `inputRequests`** — see the decision
below.

`MrtrIngest::Proceed` carries the kinds off the verified continuation;
`retype_verified_input_responses` runs inside `apply`, on the `Proceed` arm only.
That placement is the security argument: the kinds it enforces against have
already passed the AEAD tag check, so the client can neither choose them nor alter
them, and the refusal happens before the handler is invoked.

### `src/types/protocol/context.rs` — the swap seam

`with_kind_directed_input_responses` replaces the typed map;
`input_responses_raw()` feeds the re-decode. **`RequestHandlerExtra::input_responses()`
is byte-unchanged** — handlers still receive `Option<&InputResponses>`. Only the
correctness of the typing moves, which is exactly why the fix re-decodes into the
same typed map instead of handing handlers raw JSON. The redacting `Debug` is
unaffected: it reports presence only, and the new field lives inside
`MrtrRequestParams`, which that `Debug` never renders.

## The measured correction: the literal D-113-O answer is TYPED, not rejected

The plan predicted that the overlapping answer would be **rejected** with
`INVALID_PARAMS`, and asked for a test asserting exactly that. It is not, and the
plan's premise is wrong against the tree:

- `ElicitResult` carries no `#[serde(deny_unknown_fields)]`, so the surplus
  `model` is ignored;
- its `content` is `Option<HashMap<String, Value>>`, which
  `{"type":"text","text":"hello"}` satisfies.

So `{"action":"accept","content":{…},"model":"…"}` **is a valid `ElicitResult`.**
The client's answer was well formed all along; it was the SERVER's guess that was
the defect. The correct fix for that value is to type it right, not to reject it —
and the loop closes because the handler's `Elicitation` arm now matches and the
operation *completes*.

Rejection is the outcome for a value that genuinely cannot be the requested kind.
Dropping `action` makes the object a `CreateMessageResult` and nothing else
(`ElicitResult::action` has no default), and that is refused with `-32602` naming
the key. **Both branches are pinned**, at both the unit and the socket level:

| Answer to an `elicitation/create` under `"k"` | Outcome |
|---|---|
| `{"action","content","model"}` — the literal D-113-O value | typed `Elicitation`, round **completes** |
| `{"content","model"}` — cannot be an `ElicitResult` | **refused** `-32602`, names `"k"` and `elicitation/create` |
| `{"action":"accept","content":{…}}` — unambiguous | typed `Elicitation`, round completes |
| answered under an unrequested key | **refused** `-32602`, key NOT echoed |

Recorded rather than papered over, because bending the implementation to make the
plan's assertion true would have meant rejecting well-formed client answers — a
strictly worse outcome than the defect.

## What the refusal may say, decided by provenance

The plan asked for a message "naming the offending KEY but NOT echoing its value
(the key is server-assigned…)". That justification is sound for one branch and
false for the other, so the rule was applied rather than the instruction:

| Variant | Key provenance | Bound | `Display` |
|---|---|---|---|
| `KindMismatch` | read out of the SEALED kinds map — server-assigned | the whole continuation fits in `MAX_REQUEST_STATE_LEN` (8192) | **names the key** and the requested kind |
| `Unsolicited` | CLIENT-chosen by definition — it is a key the continuation never contained | only the 256 KiB `inputResponses` total | **names nothing** |

The mismatch key is taken with `kinds.get_key_value(key)` rather than from the
client's map — identical by construction at that point, but taking it from the
trusted side makes the provenance structural instead of argued. Echoing an
unsolicited key would both amplify (up to 256 KiB into an error message) and
poison logs, which is precisely the discipline `MrtrParseError`'s `Display`
already applies to its own `key` fields. Neither variant ever renders a VALUE.
`an_unsolicited_key_is_refused_without_being_echoed` asserts the key does not
appear anywhere in the response bytes, using a distinctive key so the negative
assertion is decidable.

## Blast radius of the wire-visible rejection — stated plainly

This change **rejects requests that previously succeeded**, and also **completes
requests that previously looped**. Precisely:

**What a client that previously "worked" now sees.** Only a client that answers
`inputResponses` in a way that does not match what the server asked for is
affected, and only on a **v2** connection to a server **with a `requestState`
codec configured**, presenting a `requestState` that **verifies**. Two cases:

1. **An answer that cannot decode as the requested kind** → `-32602` at HTTP 400,
   handler never invoked. Previously: the answer was silently reclassified into
   some other variant and handed to the handler, which — if it matched on the
   variant — re-elicited, up to `MAX_MRTR_ROUNDS`.
2. **An answer under a key the continuation never requested** → `-32602` at HTTP
   400, handler never invoked. Previously: accepted and passed through.

**Did any existing test or example rely on the untagged fallback?** **No.** The
full gate is exit 0 across **249 test-result lines, 4469 passed, 0 failed**,
including every example, with no test modified to accommodate the change. The 29
pre-existing `tests/v2_mrtr.rs` tests and the 12 in `tests/v2_mrtr_ingress.rs`
pass unchanged. The reason is structural rather than lucky: `pmcp::Client` builds
its `inputResponses` from the very `inputRequests` the server sent, through
`decode_for`, so **a pmcp client is incapable of producing a mismatched answer**.
That is also why no test involving one could ever have caught D-113-O, and why
every test here drives raw frames.

**What is NOT affected — proven, not asserted.**
`well_behaved_answers_of_every_kind_are_unaffected` drives all three kinds through
the full mint / verify / re-decode path over a socket. A first call carrying
`inputResponses` with no `requestState` is untouched (no continuation, so nothing
to enforce against). A verified round carrying no answers at all is untouched — a
client may always resend without answering, and the handler simply asks again
(`a_verified_round_with_no_answers_is_not_a_mismatch`,
`sep_2322_missing_response_rerequests`). v1 and non-opted-in requests run zero
MRTR code, unchanged.

**The rolling-deploy window.** A continuation minted by a build without the field
deserializes to `kinds: None` and DEGRADES to the untagged decode rather than
being rejected — so an in-flight token does not fail mid-deploy. This is not a
bypass: reaching that path requires presenting a continuation whose AEAD tag
verifies, and only a holder of the server's key can mint one. The only party who
can produce a kinds-less continuation is a previous build of this same server.

## `Option`, not an empty-map sentinel

The plan specified "an empty kinds map means 'this continuation predates kind
carriage' and must DEGRADE". That conflates two different statements, and the
second is **reachable in production**: a handler may signal `input_required` with
an empty `inputRequests` map, and an empty-means-degrade rule would then accept
arbitrary untagged answers on a round that requested nothing at all — a hole in
the middle of the fix.

`Option<InputRequestKinds>` splits them:

| Value | Meaning | Ingress |
|---|---|---|
| `None` (the `#[serde(default)]`) | minted by a pre-D-113-O build | degrade to untagged |
| `Some(map)`, non-empty | asked for these kinds | enforce |
| `Some(map)`, EMPTY | asked for **nothing** | enforce — every answer is unsolicited |

Pinned by `a_continuation_serialized_without_kinds_still_deserializes_as_none`
(against the serialized BYTES an older build sealed, not a hand-built struct),
`an_empty_kinds_map_survives_as_some_not_none`, and
`an_empty_kinds_map_rejects_every_answer_rather_than_degrading`.

## Token size — MEASURED, not assumed

`a_full_width_kinds_map_stays_within_the_accepted_token_bound` mints at
`MAX_INPUT_RESPONSES` (64) entries — the widest map that can ever be *answered*,
so the widest worth minting — at a realistic 12-character key length:

```
D-113-O minted token bytes: bare = 142, with 64 kinds entries = 2360 (bound 8192)
```

**2360 of 8192, i.e. 29% of the bound.** No new mint-time guard was needed: `mint`
already refuses when the encoded token would exceed `MAX_REQUEST_STATE_LEN`, so
the behaviour at the bound is a loud refusal rather than a token the server would
reject at its own front door. That path is now exercised through the kinds map
too (`an_absurd_kinds_map_is_refused_at_the_mint_rather_than_minted`, 512 entries
at 64 characters).

## Negative control — recorded verbatim

Run against the LANDED code by disabling the ingress re-decode in
`retype_verified_input_responses` (early `return Ok(context)`), leaving task 1's
kinds carriage fully in place — exactly the pre-fix ingress. `src/server/core.rs`
was restored from a byte-for-byte backup afterwards (md5
`abbf403adc2cf902aa9f77a260ee479d` before and after) and
`git diff HEAD -- src/ tests/` is empty.

### NC part 1 — the socket tests, D-113-O reproduced

```
running 34 tests
test an_answer_that_cannot_be_the_requested_kind_is_refused ... FAILED
test an_unsolicited_key_is_refused_without_being_echoed ... FAILED
test the_literal_d113o_answer_completes_instead_of_looping ... FAILED

---- the_literal_d113o_answer_completes_instead_of_looping stdout ----
thread '…' panicked at tests/v2_mrtr.rs:636:5:
assertion `left == right` failed: expected a complete result, got: {"content":[{"type":"text","text":"{\"answer\":\"need-input\"}"}],"isError":false,"inputRequests":{"k":{"method":"elicitation/create","params":{"mode":"form","message":"What is your name?","requestedSchema":{"type":"object","properties":{"value":{"type":"string"}}}}}},"requestState":"CO69auftegiF5OByS83l2Y-4cDonCGOy8WpFMUPgGKutNsW_wnRb_jKx3JKlhYzGy2UW2tmqcNDOkfRH4Zj1ALZxYpxkkfcI2K0tcd9zwIXezV1FokEJl9W2FIm3S4YTjhA9bkvfRyRlJ_59LpSt4g","resultType":"input_required","_meta":{…}}
  left: Some("input_required")
 right: Some("complete")

---- an_answer_that_cannot_be_the_requested_kind_is_refused stdout ----
thread '…' panicked at tests/v2_mrtr.rs:1516:5:
assertion `left == right` failed: body was {"jsonrpc":"2.0","id":2,"result":{…"resultType":"input_required"…}}
  left: 200
 right: 400

---- an_unsolicited_key_is_refused_without_being_echoed stdout ----
thread '…' panicked at tests/v2_mrtr.rs:1596:5:
assertion `left == right` failed: body was {"jsonrpc":"2.0","id":2,"result":{…"resultType":"input_required"…}}
  left: 200
 right: 400

test result: FAILED. 31 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.59s
```

**Every one fails by NOT erroring**, which is precisely the shape D-113-O
describes. The first failure body shows the handler **re-eliciting** — a fresh
`inputRequests` for `"k"` and a fresh `requestState` — rather than completing.
That IS the silent reclassification, on a live socket.

**The orthogonality is the evidence.** Exactly three tests fail and 31 pass. The
two controls — `a_correctly_shaped_answer_still_completes_the_round` and
`well_behaved_answers_of_every_kind_are_unaffected` — stay GREEN, so they are not
merely restating the three; and `a_client_that_ignores_its_own_round_limit_is_stopped_by_the_server`
stays green too, which is the direct evidence that **113-24's ceiling is
independent of this fix** and bounds a loop it cannot diagnose.

### NC part 2 — the `apply`-layer tests

```
running 56 tests
test server::core::tests::mrtr_ingest_tests::an_unsolicited_key_is_rejected_at_the_verified_path ... FAILED
test server::core::tests::mrtr_ingest_tests::an_answer_that_cannot_be_the_requested_kind_is_rejected_at_the_verified_path ... FAILED
test server::core::tests::mrtr_ingest_tests::the_literal_d113o_answer_reaches_the_handler_as_an_elicitation ... FAILED

---- …::the_literal_d113o_answer_reaches_the_handler_as_an_elicitation stdout ----
thread '…' panicked at src/server/core.rs:5638:13:
assertion failed: matches!(context.expect("context survives").input_responses().expect("answers")["k"],
    crate::types::mrtr::InputResponse::Elicitation(_))

test result: FAILED. 53 passed; 3 failed; 0 ignored; 0 measured; 1598 filtered out; finished in 0.02s
```

The last one is the reclassification stated at its narrowest: with the re-decode
off, the handler's answer under `"k"` is not an `Elicitation`.

The `types::mrtr::kind_directed_tests` suite stays GREEN under this control, and
correctly so — it tests `retype_input_responses_for_kinds` directly, which the
control did not disable. That is the right split: the mrtr tests pin the FUNCTION,
the core tests pin that it is CALLED.

## Verification

| Check | Result |
|---|---|
| `cargo test --features full --lib -- types::mrtr` | **72 passed**, 0 failed (**12 new**; 113-26's 60 intact) |
| `cargo test --features full --lib -- server::core::tests` | **98 passed**, 0 failed (**7 new**; 113-26's 91 intact) |
| `cargo test --features full --lib -- server::request_state` | **50 passed**, 0 failed (**5 new**; 113-25/26's 45 intact) |
| `cargo test --features full --test v2_mrtr` | **34 passed**, 0 failed (**5 new**; 113-24's 29 intact) |
| `cargo test --features full --test v2_mrtr_ingress` | **12 passed**, 0 failed (113-26's, unchanged) |
| `cargo test --features full --test v2_bounded_reads_tripwire` | **13 passed**, 0 failed (113-21's tripwire, unweakened) |
| `cargo build --features "full,fuzzing"` | clean — the fuzz seam still compiles |
| `cargo clippy --features "full" --lib --tests -- -D clippy::all` | clean |
| **`make lint`** (pedantic + nursery, the real gate) | **exit 0** |
| `cargo fmt --all -- --check` | clean |
| `make check-todos` | no technical-debt comments |
| `make quality-gate` (background job, polled) | **exit 0** — **249 test-result lines, 4469 passed, 0 failed** |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks: 223 pass, 30 skip — no semver update required** |
| `make wasm-build` | `✓ WASM build complete`; **83 → 86** warnings (see below) |
| `Cargo.toml` / `Cargo.lock` | **untouched** — zero new dependencies |

Totals were read from the raw gate log with `awk` and via `$HOME/.cargo/bin/cargo`,
since the rtk shell proxy compresses `test result:` lines.

**`semver-checks` re-confirmed WITH the new public derives in place**, which was
the one item that could have moved the needle: adding `Serialize`/`Deserialize`
to the public `InputRequestKind` is additive, and the run above was executed after
that landed rather than before. Everything else added is `pub(crate)` or test code.

**The three new wasm warnings are the known dead-code class, measured not
assumed.** The baseline was taken the safe way — `tar` backup, `git checkout
HEAD~3 -- <files>`, measure, restore, `git diff HEAD` to confirm — and is **83**,
matching 113-26's recorded post-plan value exactly. The three additions are
`InputRequestKinds`, `InputResponseTypingError` and
`retype_input_responses_for_kinds`, all "never used" on wasm because the wasm
server has no MRTR (D-14) — the same class as `CanonicalDepthExceeded`,
`MAX_CANONICAL_DEPTH`, `write_canonical` and `salient_param_digest`, which already
warned there before this plan.

**`make lint` found nothing this plan's own `-D clippy::all` command missed** —
the first plan in this wave where the two agreed. `make lint` was still run as
mandated, and remains mandatory.

## Threat Register Disposition

| Threat ID | Disposition | Evidence |
|---|---|---|
| T-113-127 (Tampering, `try_from_value_untagged` reclassifying an answer) | **mitigated** | `the_literal_d113o_answer_completes_instead_of_looping`; NC part 2 shows the reclassification at its narrowest with the re-decode off |
| T-113-128 (DoS, the silent re-elicitation loop) | **mitigated** | the loop now ends on the FIRST resend, handler run exactly twice; composes with 113-24's ceiling, which the NC shows is independent |
| T-113-129 (Spoofing, answering under an unrequested key) | **mitigated** | `an_unsolicited_key_is_refused_without_being_echoed` (socket) + `an_unsolicited_key_is_rejected_at_the_verified_path` (unit) |
| T-113-130 (Tampering, a client choosing the kinds) | **mitigated** | the kinds ride inside the AEAD-sealed continuation and are read only on the `Verdict::Ok` arm, i.e. after the tag check; `mint_then_verify_round_trips_the_requested_kinds` |
| T-113-131 (DoS, in-flight tokens failing during a rolling deploy) | **accept, guarded** | `#[serde(default)]` + `Option` degradation, pinned against the serialized BYTES an older build sealed; not a bypass, since an attacker cannot mint a continuation |
| T-113-132 (DoS, the kinds map bursting `MAX_REQUEST_STATE_LEN`) | **mitigated** | measured at 2360/8192 for 64 entries; the pre-existing mint refusal covers the bound and is exercised through the kinds map |
| T-113-133 (Information Disclosure, a rejection echoing attacker content) | **mitigated** | provenance table above; neither variant renders a value, and `Unsolicited` renders nothing at all |

## Threat Flags

None. No new network endpoint, auth path, file access pattern or schema at a
trust boundary. The change is a re-typing and two refusals on existing code paths.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The plan's predicted rejection branch does not fire for the literal D-113-O value**

- **Found during:** Task 2
- **Issue:** The plan's task 2 step 5 and its success criteria assert that the
  overlapping answer (`action` + `content` + `model`) answered to an elicitation
  is REJECTED with `INVALID_PARAMS`. It is not. `ElicitResult` has no
  `deny_unknown_fields` and its `content` is `Option<HashMap<String, Value>>`, so
  that object is a **valid `ElicitResult`** — the surplus `model` is ignored. The
  first version of the test failed against a correct implementation.
- **Fix:** Measured which branch actually fires, and pinned BOTH: the literal
  D-113-O value is typed as `Elicitation` and the round COMPLETES
  (`the_literal_d113o_answer_...`), while a value that genuinely cannot be the
  requested kind — drop `action` — is refused naming the key
  (`an_answer_that_cannot_be_the_requested_kind_is_...`). Bending the
  implementation to make the plan's assertion true would have meant rejecting
  well-formed client answers, which is strictly worse than the defect.
- **Files modified:** `src/types/mrtr.rs`, `src/server/core.rs`, `tests/v2_mrtr.rs`
- **Committed in:** `7b47694e`, `9b7fedb0`

**2. [Rule 2 - Missing critical functionality] `Option<InputRequestKinds>`, not an empty-map sentinel**

- **Found during:** Task 1
- **Issue:** The plan's compatibility note specifies "an empty kinds map means
  'this continuation predates kind carriage' and must DEGRADE to the untagged
  decode". That conflates two distinct states, and the second is reachable in
  production: a handler may signal `input_required` with an empty
  `inputRequests`, and empty-means-degrade would then accept arbitrary untagged
  answers on a round that requested nothing — a hole in the middle of the fix.
- **Fix:** `Option<InputRequestKinds>` with `#[serde(default)]`. ABSENT is the
  pre-kinds marker; `Some(empty)` means "asked for nothing" and rejects every
  answer. `seal_input_required` passes `Some(...)` unconditionally.
- **Committed in:** `64de5b15`

**3. [Rule 2 - Missing critical functionality] An unsolicited key is NOT echoed**

- **Found during:** Task 2
- **Issue:** The plan asks for "a message naming the offending KEY", justifying it
  with "the key is server-assigned". True for a kind mismatch; **false** for an
  unsolicited key, which by definition is a key the continuation never contained
  and is therefore client-chosen, bounded only by the 256 KiB `inputResponses`
  total. Echoing it would amplify into an error message and poison logs — exactly
  what `MrtrParseError`'s `Display` discipline exists to prevent.
- **Fix:** Two variants with different `Display` behaviour. `KindMismatch` names
  its key, taken via `kinds.get_key_value` so the rendered key provably comes
  from the SEALED map; `Unsolicited` carries its key for programmatic use and
  renders nothing.
- **Committed in:** `7b47694e`

**4. [Rule 3 - Blocking] `MrtrIngest::Reject.message` widened to `String`**

- **Found during:** Task 2
- **Issue:** The mismatch refusal names a runtime key, which a `&'static str`
  cannot carry.
- **Fix:** `String`, propagated through `MrtrIngest::apply` and `MrtrRound::begin`.
  Both dispatch sites already called `.to_string()` on it, so the change removed
  two conversions rather than adding any.
- **Committed in:** `7b47694e`

**5. [Rule 3 - Blocking] Task 1 does not add the `Proceed` field**

- **Found during:** Task 1
- **Issue:** The plan's task 1 step 4 asks for the kinds to be carried forward to
  where task 2 consumes them. Doing so produces a `field 'kinds' is never read`
  dead-code warning, which `make lint` treats as an error — so task 1's commit
  would not have passed the gate.
- **Fix:** Task 1 ends at the sealed carriage (readable from `Continuation` on the
  `Verdict::Ok` arm) and task 2 adds the `Proceed` field together with its
  consumer. No transitional block was needed, unlike 113-25/113-26.
- **Verification:** both commits build and lint clean independently.

**6. [Rule 3 - Blocking] `MrtrRequestParams`' new field at the client's egress sites**

- **Found during:** Task 2
- **Issue:** `src/client/mod.rs` constructs `MrtrRequestParams` to SPLICE onto an
  outgoing request, where a raw-retention field has no meaning.
- **Fix:** Explicit `input_responses_raw: None` at both sites with a comment
  naming them as EGRESS, rather than `..Default::default()`, which would have
  hidden a future field addition from those sites.
- **Committed in:** `7b47694e`

---

**Total deviations:** 6 auto-fixed (1× Rule 1, 2× Rule 2, 3× Rule 3)
**Impact on plan:** No scope creep. Deviations 2 and 3 are places where the plan's
stated RULE and its literal INSTRUCTION diverged, and the rule was applied.
Deviation 1 is a factual correction to a premise, measured rather than argued.

## Issues Encountered

None beyond the deviations above. No `git stash` was run at any point.

## Known Stubs

None.

## Requirements

**HTTP-03 stays `[~]`.** `.planning/REQUIREMENTS.md` was NOT edited and no
checkbox was flipped — the STATE.md publication gate forbids flipping
HTTP-01..09 / CLNT-01/02/05 this round. `requirements-completed` in the
frontmatter is deliberately empty for the same reason.

## What This Does NOT Close

- **The untagged decoder still exists, and still reclassifies.** It has to: a
  first call carrying `inputResponses` with no `requestState` has no continuation
  to read a kind from. That residual is now DOCUMENTED on
  `try_from_value_untagged` rather than implicit, naming both surviving cases and
  D-113-O, and `the_untagged_decoder_still_reclassifies_the_overlapping_answer`
  pins the premise so the kind-directed tests cannot pass vacuously if the overlap
  ever disappears.
- **A first call carrying `inputResponses` is unvalidated.** Nothing was
  requested, so there is nothing to validate against. A handler receiving answers
  it never asked for on a first call must still treat them as untrusted — which
  the `ProtocolContext` rustdoc has always said of `inputResponses`.
- **The kinds map is not a schema check.** It pins WHICH result type an entry
  must be, not that the entry satisfies the elicitation's `requestedSchema`.
  Schema validation remains the handler's job, as documented.
- **Still open and unowned in this phase:** D-113-Q (`sse_optimized.rs:266`
  unbounded `reqwest` body — enumerated in the tripwire allowlist with a written
  NOT-BOUNDED justification), D-113-R (`drain_complete_lines` quadratic; blocks
  HTTP-09 substantively), D-113-S (no stdio listen), D-113-T (intermittent
  `LEAK` in four pre-existing `v2_subscriptions.rs` tests).
- No blockers introduced.

## Next Phase Readiness

- **113-28 (the decision checkpoint) is unblocked.** This was the last
  code-touching plan of the gap-closure round; D-113-L, D-113-M, D-113-N, D-113-O
  and D-113-P are all now closed.
- A re-verifier should reproduce the defect through a REAL socket, not through
  `pmcp::Client` — a Client on both ends is structurally incapable of producing
  the mismatched answer and would report success against the unfixed tree.

## Self-Check: PASSED

- `src/types/mrtr.rs` — FOUND, contains `InputRequestKinds`, `InputResponseTypingError`, `retype_input_responses_for_kinds`, `input_responses_raw`
- `src/server/request_state.rs` — FOUND, contains `Continuation.kinds` and the four-parameter `mint`
- `src/server/core.rs` — FOUND, contains `InputRequestKind` (via `seal_input_required`) and `retype_verified_input_responses`
- `src/types/protocol/context.rs` — FOUND, contains `with_kind_directed_input_responses` and `input_responses_raw`
- `tests/v2_mrtr.rs` — FOUND, contains all five new test names
- commit `64de5b15` — FOUND (`feat(113-27): seal the server's own record of which kinds it requested`)
- commit `7b47694e` — FOUND (`feat(113-27): type inputResponses by kind at ingress, and reject a mismatch`)
- commit `9b7fedb0` — FOUND (`test(113-27): prove D-113-O's loop terminates, over a real socket`)
- `git diff HEAD -- src/ tests/` empty after the negative control was reverted; `src/server/core.rs` md5 matches its pre-control backup
- `grep -rn "NEGATIVE CONTROL" src/ tests/` — no matches
- `.planning/REQUIREMENTS.md` — NOT in the diff

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-27*
</content>
</invoke>
