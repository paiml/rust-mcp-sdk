---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 11
subsystem: testing
tags: [mcp-2026-07-28, mrtr, conformance, streamable-http, elicitation, examples, interoperability]

# Dependency graph
requires:
  - phase: 113-09
    provides: "the hardened server-side MRTR egress — MrtrSignal::into_meta_entry authoring, the unconditional signal strip, the submode-aware -32021 capability precheck, and the loud INTERNAL_ERROR on a forbidden path"
  - phase: 113-07
    provides: "MrtrOutcome, Error::input_required_unfulfilled / is_mrtr_round_limit_exceeded, call_tool_mrtr and the bounded gather->resend loop"
  - phase: 113-06
    provides: "the requestState ingress verdict table and the tests/common/v2.rs live-HTTP harness"
  - phase: 113-05
    provides: "the pmcp Client v2 mode (with_protocol_version, raw v2 frames, registry-authoritative clientCapabilities)"
  - phase: 113-03
    provides: "the server-instance-owned AEAD requestState codec plus the pmcp::testing open_request_state seam"
  - phase: 113-01
    provides: "113-SPEC-RECHECK.md section B — the PINNED conformance commit and its 23 sep-2322 check ids, which this plan's manifest is generated from"
provides:
  - "113-CONFORMANCE-MANIFEST.md — the scenario-id -> local-test inventory generated from the pinned conformance commit, with a must-be-empty Unmapped section"
  - "tests/v2_mrtr.rs — 27 tests: 15 sep-2322 scenario mirrors, 3 pmcp wire-shape rows, 1 manifest-enforcement test, 8 real-client interoperability tests"
  - "manifest_maps_every_pinned_scenario — the build-visible failure for an unmapped upstream scenario"
  - "examples/s47_v2_stateless_mrtr.rs — a runnable dual-version stateless MRTR SERVER"
  - "examples/s48_v2_mrtr_client.rs — a runnable, scriptable v2 CLIENT that fulfils it automatically"
  - "the finding that a handler-less pmcp client can never receive an input_required from a pmcp server (capability honesty composes with the -32021 precheck)"
affects: [113-12, 118, 119]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A conformance manifest that is ENFORCED by a test which re-reads the planning records at runtime, so re-pinning the suite without regenerating the manifest fails the build instead of drifting silently"
    - "Guard the enforcement's early-exit on the DIRECTORY, not the file: `.planning/` is excluded from the published crate, so a missing manifest is only tolerated when the whole phase directory is absent"
    - "One scripted fixture server with a per-tool Script enum, so 15 scenarios and 8 interoperability tests share one dispatch surface instead of 23 bespoke servers"
    - "Drive conformance mirrors with raw `post` (bytes) and interoperability with a real Client (agreement) — a Client in the middle of a conformance assertion passes whenever both ends share a bug"
    - "Verify a printed curl command by actually running it, so example documentation cannot rot into a wrong incantation"

key-files:
  created:
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/113-CONFORMANCE-MANIFEST.md
    - tests/v2_mrtr.rs
    - examples/s47_v2_stateless_mrtr.rs
    - examples/s48_v2_mrtr_client.rs
  modified:
    - Cargo.toml
    - src/types/mrtr.rs

key-decisions:
  - "The manifest keys on CHECK ID (23 rows) while a local test may mirror several ids, because the suite emits one check per ROUND of one exchange and Rust cannot split a single HTTP round trip across three #[tokio::test] functions. The two deviations from the mechanical dash->underscore rule are enumerated in the manifest and both are still pure string comparisons."
  - "manifest_maps_every_pinned_scenario reads 113-SPEC-RECHECK.md at RUNTIME rather than include_str!-ing it, so the .planning exclusion from the published crate cannot break compilation of a packaged test (the precedent, tests/team_contracts_conformance.rs, had to be excluded from the package entirely for reading contracts/ at runtime)."
  - "The plan's 'register NO handlers' premise for the two D-06 tests is UNREACHABLE between two conformant pmcp peers and the tests now use a DECLINING handler. See the Deviations section — this was a real finding, not a test convenience."
  - "sep_2322_not_on_unsupported_requests asserts plan 09's concrete shipped behavior over the WIRE by invoking the signalling tool on a v1 request (where MRTR is impossible) and requiring -32603 with no result. `resources/list` was rejected as the vehicle because ListResourcesResult has no `_meta` to smuggle a signal through."
  - "sep_2322_reject_tampered_state tampers with a token the SERVER actually minted during a live round trip, rather than one minted through the pmcp::testing seam, so it is the conformance shape and not a duplicate of tests/v2_mrtr_ingress.rs::tampered_state_errors."
  - "The examples keep the plan-pinned s47_/s48_ names despite colliding with the existing s47_task_augmented_result / s48_durable_poll_decision numbering. Renaming would break the plan's artifact contract and the phase acceptance record; the collision is recorded in Cargo.toml and here for a follow-up decision."

patterns-established:
  - "Planning records can be load-bearing test inputs: a phase record (the pin) -> a derived inventory (the manifest) -> a runtime cross-check is a cheap way to make an external requirement measurable before its real harness exists"
  - "When two independently-correct rules compose into an unreachable scenario, LOCK the composition with its own test rather than deleting the test that discovered it"

requirements-completed: []

# Metrics
duration: 40min
completed: 2026-07-26
---

# Phase 113 Plan 11: sep-2322 Conformance Mirror, Real-Client Interoperability and Runnable Examples Summary

**Every `sep-2322` scenario at the pinned conformance commit now has a named, passing Rust test whose absence would fail the build; a real pmcp `Client` completes one-round, three-round and mixed-kind MRTR exchanges against a real pmcp server over HTTP with no handshake and no session; and a developer can run one command to watch a stateless v2 server elicit and a second to watch a client fulfil it automatically.**

## Performance

- **Duration:** ~40 min
- **Tasks:** 3
- **Files:** 4 created, 2 modified
- **Tests added:** 27 in `tests/v2_mrtr.rs` (plan floor: 21)

## What Shipped

### Task 1 — the manifest and the scenario mirrors (`4fc0aeac`)

`113-CONFORMANCE-MANIFEST.md` is generated from `113-SPEC-RECHECK.md` § B — the pinned commit
`a865118206d4d8cc8dbc5f5201607839281d0c3b` and its 23 `sep-2322` check ids across 14 scenario
classes — and explicitly NOT from the `113-RESEARCH.md` table. It carries the pinned sha, a
documented name-derivation rule, the 23-row mapping, the three pmcp-added wire-shape rows, an
EMPTY `## Unmapped` section, a `## Research-Table Delta`, and the DRIFT-1 known-failing record.

`tests/v2_mrtr.rs` mirrors them with one scripted fixture server: eight tools driven by a
`Script` enum, plus an MRTR-signalling prompt and resource so `prompts/get` and `resources/read`
are exercised as first-class MRTR legs. All 15 scenario mirrors drive the wire with raw `post`.

The three pmcp-added rows are the ones a purely in-house test would have missed:

| Test | What it catches |
|------|-----------------|
| `mrtr_fields_are_params_siblings` | an `_meta`-placed retry must NOT resume — it re-elicits. An in-house round trip that put the fields in `_meta` at both ends would pass while every conformance check failed (T-113-28) |
| `mrtr_retry_uses_different_id` | a retry with a different id — including a different JSON *type* — is accepted and its OWN id echoed (T-113-07) |
| `mrtr_signal_key_never_on_wire` | seven exchanges' raw bodies contain zero occurrences of `dev.pmcp/mrtr` (T-113-31) |

`manifest_maps_every_pinned_scenario` is the enforcement. At test time it re-reads
`113-SPEC-RECHECK.md` § B.1/B.2 and the manifest and asserts: the two pinned shas match; the
mapping's check-id set EQUALS § B.2's in both directions; every mapped test name exists as a `fn`
in this file; every auxiliary table names a real test; and `## Unmapped` contains no `sep-2322`.
**Verified negatively** — renaming one mapping cell to `sep_2322_TYPO_HERE` failed the test with
`sep-2322-elicitation-incomplete maps to \`sep_2322_TYPO_HERE\`, which does not exist`, and the
manifest was then restored.

### Task 2 — real Client ↔ real server (`6142ebeb`)

Eight tests drive the SAME fixture server with a real `pmcp::Client` opted into 2026-07-28,
behind a recording HTTP middleware so "exactly N requests" is an observation rather than an
inference. One-round, three-round and mixed-kind exchanges complete with exact handler
invocation counts (1 / 3 / one-of-each); the whole multi-request loop runs with no `initialize`
and no `Mcp-Session-Id`; `mrtr_round_limit(2)` surfaces the typed error after exactly 2
server-observed requests; and an unfulfillable result reaches the caller as
`MrtrOutcome::InputRequired` or as `Error::input_required_unfulfilled` after exactly 1 request,
never as an empty `CallToolResult`.

The strongest assertion here: the `requestState` recovered from the typed error is OPENED with
the fixture server's own key via `pmcp::testing::open_request_state`, recovering the exact
continuation (`{"step": 1}`) the scripted handler sealed at round 1. That proves the token that
reached the caller is the server's real minted continuation, not a shell.

### Task 3 — the runnable pair (`3647cfa4`)

`examples/s47_v2_stateless_mrtr.rs` (296 lines) is a dual-version server — 2025-11-25 AND
2026-07-28 in one accept-list — whose `weather` tool asks for the missing city through
`MrtrSignal::into_meta_entry()` and resumes from `extra.mrtr_continuation()` +
`extra.input_responses()` on the retry. It runs on the STATEFUL default HTTP config on purpose,
so the session-freedom on display is the per-request era gate rather than a build-time switch. It
prints its bound `127.0.0.1:<port>`, a copy-pasteable round-1 `curl`, and the round-2 procedure,
and it documents the `PMCP_REQUEST_STATE_KEY` contract while **showing** the unset-key startup
warning instead of suppressing it (T-113-17).

`examples/s48_v2_mrtr_client.rs` (236 lines) is a scriptable one-shot client with a programmatic
(non-stdin) elicitation handler. It demonstrates automatic fulfilment through plain `call_tool`,
the D-06 typed-error path when the handler declines, and the `-32021` capability-honesty refusal,
and exits 0 only when all three behave as documented.

## Deviations from Plan

### 1. [Rule 1 — Bug in the plan's premise] "Register NO handlers" is unreachable between two conformant pmcp peers

- **Found during:** Task 2, on the first run of `client_server_mrtr_outcome_input_required` and
  `client_server_mrtr_existing_method_typed_error`.
- **Issue:** both tests failed with `-32021 the server needs a client capability this client did
  not declare`. The plan assumed a handler-less client would receive an `input_required` it could
  not fulfil. It cannot, because two independently-correct rules compose:
  - the CLIENT's v2 `clientCapabilities` are registry-authoritative (`Client::v2_client_capabilities`
    derives them from registered handlers — capability honesty, HOST-05), so an empty registry
    declares no `elicitation`; and
  - the SERVER refuses, all-or-nothing, to emit `inputRequests` for an undeclared capability
    BEFORE minting anything (T-113-32, plan 09).
- **Fix:** both D-06 tests now register a DECLINING elicitation handler — the reachable path with
  the identical shape (capability declared, server mints, client still cannot fulfil because the
  user said no). Every exact assertion the plan specified survives unchanged: 1 server-observed
  request, `is_input_required_unfulfilled()`, no empty `CallToolResult`.
- **Added:** `client_server_mrtr_undeclared_capability_is_refused` locks the discovered
  composition so neither rule can regress unnoticed, and a manifest subsection explains it.
- **Files:** `tests/v2_mrtr.rs`, `113-CONFORMANCE-MANIFEST.md`
- **Commit:** `6142ebeb`

This is the kind of behavior only a real-client-against-real-server test can observe, which is
precisely why Task 2 exists.

### 2. [Rule 2 — Missing critical demonstration] The client example gained a third demonstration

- **Found during:** Task 3, as a direct consequence of deviation 1.
- **Issue:** the plan specified two demonstrations for `s48`, the second being "no handler
  registered → `Error::input_required_unfulfilled`". As above, that combination yields `-32021`.
- **Fix:** demonstration 2 uses a declining handler (the D-06 path the plan wanted), and a third
  demonstration shows the handler-less `-32021` refusal explicitly. The example now teaches both,
  and its header explains why they differ. `is_input_required_unfulfilled` is still present and
  exercised, per the acceptance criteria.
- **Files:** `examples/s48_v2_mrtr_client.rs`
- **Commit:** `3647cfa4`

### 3. [Rule 3 — Blocking issue] `sep-2322-not-on-unsupported-requests` needed a reachable vehicle

- **Issue:** the plan asked to assert plan 09's "a handler signalling on `tools/list` fails
  loudly". No `tools/list` handler exists — pmcp builds that result itself — and
  `ListResourcesResult` carries no `_meta`, so `resources/list` cannot smuggle a signal either.
- **Fix:** the test asserts both reachable halves at the wire level: MRTR fields on `tools/list`
  are inert (`resultType: "complete"`, no MRTR fields on the result), AND the signalling tool
  invoked on a **v1** request — where MRTR is equally impossible — answers `-32603` with no
  result and no leaked signal key. The manifest's "Row 18" subsection records this and points at
  plan 09's `mod mrtr_egress` unit suite for the compile-time half.
- **Files:** `tests/v2_mrtr.rs`, `113-CONFORMANCE-MANIFEST.md`
- **Commit:** `4fc0aeac`

### 4. [Recorded, not fixed] The `s47_`/`s48_` example numbering now collides

`examples/s47_task_augmented_result.rs` and `examples/s48_durable_poll_decision.rs` already
occupied those slots. Cargo example NAMES are unique, so all four build and run and nothing is
broken — but the `sNN_` sequence is no longer a bijection. The plan's `must_haves.artifacts` and
its acceptance greps pin `examples/s47_v2_stateless_mrtr.rs` and `examples/s48_v2_mrtr_client.rs`
by exact path, so renaming here would break the phase's artifact contract. Recorded in a
`Cargo.toml` comment next to the new entries and deferred as a naming decision.

## Verification

| Check | Result |
|-------|--------|
| `cargo test --test v2_mrtr --features full` | **27 passed**, 0 failed (plan floor: 21) |
| Manifest exists, carries the pinned sha, `## Unmapped` empty, `## Research-Table Delta` present | yes |
| Every manifest test name exists in `tests/v2_mrtr.rs` | enforced by `manifest_maps_every_pinned_scenario`, **negatively verified** |
| `cargo test --test v2_mrtr_ingress / v2_stateless_http / v2_client / v2_subscriptions` | 10 / 23 / 21 / 9 passed |
| `cargo test --test '*' --features full` (all integration suites) | all green |
| `cargo test --lib --features full` | 1513 passed |
| `make test-doc` (`cargo test --doc --features full`) | 390 passed, 0 failed |
| `make lint` (clippy pedantic+nursery, `-D warnings`, incl. `cargo check --examples`) | clean |
| `cargo fmt --all -- --check` | clean |
| `cargo build --example s47_v2_stateless_mrtr / s48_v2_mrtr_client --features full` | both exit 0 |
| s47 liveness | `timeout` **PRESENT** on this machine, so the `timeout 12 cargo run` branch ran and returned **124** (still alive); `target/s47_run.log` matched `127\.0\.0\.1:[0-9]+` |
| s48 against a backgrounded s47 | exit **0**, all three demonstrations passed |
| The printed round-1 `curl` | executed verbatim against a live s47: returned `resultType: "input_required"` with `inputRequests.city` and a `requestState` |
| The printed round-2 procedure | executed verbatim with the round-1 token: returned `resultType: "complete"` with the resumed forecast |

**Which liveness branch ran:** the `timeout`-returns-124 branch. Both `timeout` and `gtimeout`
are present on this machine; the background-spawn fallback was not exercised here and remains the
documented path for environments without GNU/BSD `timeout`.

## Threat Model Coverage

| Threat | Where it is now asserted |
|--------|--------------------------|
| T-113-17 (multi-instance deploy without a shared key) | `s47`'s header documents the contract and the example EMITS the startup warning — captured in `target/s47_run.log` on every run |
| T-113-28 (MRTR fields accepted from `_meta`) | `mrtr_fields_are_params_siblings` — the `_meta` variant re-elicits, the top-level variant resumes |
| T-113-32 (array-shaped capability payload) | `input_required_result_capability_check` asserts `data.requiredCapabilities.is_object()` and names `sampling`, driven by a deliberate `v2_body_with_caps` under-declaration |
| T-113-11 (round-limit regression) | `client_server_mrtr_round_limit_typed_error` asserts an exact server-observed count of 2 |
| T-113-31 (internal signal key on the wire) | `mrtr_signal_key_never_on_wire` greps 7 raw bodies; `sep_2322_not_on_unsupported_requests` greps the v1 failure body too |
| T-113-37 (an example leaking a key) | neither example hardcodes a key; both rely on the env contract and the documented per-process fallback |
| T-113-56 (unfulfilled normalized into empty success) | `client_server_mrtr_existing_method_typed_error` rules out the `Ok` branch explicitly and OPENS the recovered token with the server's key |
| T-113-65 (an upstream scenario going unmeasured) | `manifest_maps_every_pinned_scenario`, negatively verified |

## Known Stubs

None. No hardcoded empty values, placeholder text or unwired components were introduced.

## Notes for Plan 12

- `requirements-completed` is deliberately **empty**. `113-SPEC-RECHECK.md` records the schema
  verdict as `PENDING` and makes plan 12 Task 3's re-verification against the published
  `schema/2026-07-28` binding before HTTP-02 / HTTP-03 / CLNT-02 may be flipped. This plan
  supplies the evidence, not the flip.
- The `MrtrSignal::into_meta_entry` doctest already existed (plan 09); it gained a
  cross-reference to the new runnable pair rather than a duplicate snippet.
- `tests/v2_mrtr.rs` reads `.planning/` at RUNTIME. Unlike
  `tests/team_contracts_conformance.rs` this does **not** require a `Cargo.toml` `exclude`
  entry: the manifest check exits early when the phase directory is absent (as it is in the
  published crate) and asserts loudly when the directory exists but the manifest does not.

## Self-Check: PASSED
