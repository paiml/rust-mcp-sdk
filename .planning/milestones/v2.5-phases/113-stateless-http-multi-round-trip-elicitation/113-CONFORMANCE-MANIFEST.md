# Phase 113 — `sep-2322` Conformance Manifest

**Produced by:** Plan 113-11, Task 1
**Generated from:** `113-SPEC-RECHECK.md` § `Conformance Suite Pin (Section B)` — **NOT** from the
`113-RESEARCH.md` table (see [Research-Table Delta](#research-table-delta) for why that matters).
**Mirror file:** `tests/v2_mrtr.rs`

Phase 118 owns the official Node conformance suite in CI. Phase 113 must be self-verifying
without a Node toolchain, so every `sep-2322` check id emitted by the PINNED suite commit is
mirrored here by a named, passing Rust integration test. This file is the inventory; the test
`manifest_maps_every_pinned_scenario` in `tests/v2_mrtr.rs` is the ENFORCEMENT — it re-reads
`113-SPEC-RECHECK.md` § B.2 at test time, re-reads this file, and fails the build when the three
disagree.

---

## Pinned Commit

| Field | Value |
|-------|-------|
| Repository | `github.com/modelcontextprotocol/conformance` |
| **Pinned sha** | `a865118206d4d8cc8dbc5f5201607839281d0c3b` |
| Commit date | 2026-07-23T06:04:40Z |
| Source file enumerated | `src/scenarios/server/input-required-result.ts` (1644 lines) |
| Check ids at the pin | **23**, across **14** scenario classes |

The sha is copied verbatim from `113-SPEC-RECHECK.md` § B.1. `manifest_maps_every_pinned_scenario`
asserts the two files still carry the same 40-character sha, so re-pinning the suite without
regenerating this manifest is a test failure rather than silent drift.

---

## Name-Derivation Rule

The mapping must be a NAME COMPARISON, not a judgement call. The rule is:

1. A check id maps to a local test named `<check id with '-' replaced by '_'>`.
2. **Exception — paired halves of ONE exchange.** The suite grades a single scenario class with
   two or three check ids (`…-incomplete` + `…-complete`, or `…-r1`/`-r2`/`-r3`), because it
   asserts once per ROUND of the same exchange. Rust cannot split one HTTP round trip across
   three `#[tokio::test]` functions without re-driving the server, so those check ids share the
   ONE test that performs the whole exchange. The shared name is the class's distinguishing
   suffix with `-` → `_`, plus `_incomplete_then_complete` where the class is a two-round
   incomplete→complete pair.
3. **Exception — row 20.** `input-required-result-capability-check` is a scenario CLASS name,
   not a check id (`113-SPEC-RECHECK.md` § B.3); its emitted check id is
   `sep-2322-respect-client-capabilities`. The local test is named after the CLASS
   (`input_required_result_capability_check`) because that is the name the phase plan, the
   threat register (T-113-32) and the suite's own class declaration all use.

Rules 2 and 3 are the only deviations from rule 1, and both are enumerated in the table below —
so the mapping is still checkable by string comparison, which is what
`manifest_maps_every_pinned_scenario` does.

---

## Scenario → Test Mapping

Every row: the PINNED check id, its scenario class, the one-line obligation it grades, and the
EXACT local test fn name in `tests/v2_mrtr.rs` that mirrors it.

| # | Check id | Scenario class | Assertion it grades | Local test |
|---|----------|----------------|---------------------|------------|
| 1 | `sep-2322-elicitation-incomplete` | `input-required-result-basic-elicitation` | `tools/call` answers `input_required` with an `elicitation/create` entry and a `requestState` | `sep_2322_elicitation_incomplete_then_complete` |
| 2 | `sep-2322-elicitation-complete` | `input-required-result-basic-elicitation` | the retry carrying `inputResponses.<key>` completes | `sep_2322_elicitation_incomplete_then_complete` |
| 3 | `sep-2322-sampling-incomplete` | `input-required-result-basic-sampling` | the same first leg for a `sampling/createMessage` entry | `sep_2322_sampling_incomplete_then_complete` |
| 4 | `sep-2322-sampling-complete` | `input-required-result-basic-sampling` | the sampling retry completes | `sep_2322_sampling_incomplete_then_complete` |
| 5 | `sep-2322-list-roots-incomplete` | `input-required-result-basic-list-roots` | the same first leg for a `roots/list` entry | `sep_2322_list_roots_incomplete_then_complete` |
| 6 | `sep-2322-list-roots-complete` | `input-required-result-basic-list-roots` | the roots retry completes | `sep_2322_list_roots_incomplete_then_complete` |
| 7 | `sep-2322-request-state-incomplete` | `input-required-result-request-state` | the incomplete result carries BOTH `inputRequests` and `requestState` | `sep_2322_request_state_incomplete_then_complete` |
| 8 | `sep-2322-request-state-complete` | `input-required-result-request-state` | a retry echoing BOTH fields resumes the handler's sealed continuation | `sep_2322_request_state_incomplete_then_complete` |
| 9 | `sep-2322-multiple-inputs-incomplete` | `input-required-result-multiple-input-requests` | one result may carry several `inputRequests` of DIFFERENT kinds | `sep_2322_multiple_inputs` |
| 10 | `sep-2322-multiple-inputs-complete` | `input-required-result-multiple-input-requests` | all of them answered in ONE retry completes the call | `sep_2322_multiple_inputs` |
| 11 | `sep-2322-multi-round-r1` | `input-required-result-multi-round` | round 1 of three mints a `requestState` | `sep_2322_multi_round` |
| 12 | `sep-2322-multi-round-r2` | `input-required-result-multi-round` | round 2 re-elicits with an EVOLVED `requestState` | `sep_2322_multi_round` |
| 13 | `sep-2322-multi-round-r3` | `input-required-result-multi-round` | round 3 completes; all three tokens are distinct | `sep_2322_multi_round` |
| 14 | `sep-2322-missing-response-rerequests` | `input-required-result-missing-input-response` | under-supplied `inputResponses` produce a NEW `input_required` re-requesting only the missing entry — never an error | `sep_2322_missing_response_rerequests` |
| 15 | `sep-2322-non-tool-incomplete` | `input-required-result-non-tool-request` | `prompts/get` AND `resources/read` can answer `input_required` | `sep_2322_non_tool_incomplete_then_complete` |
| 16 | `sep-2322-non-tool-complete` | `input-required-result-non-tool-request` | both non-tool retries resume and complete | `sep_2322_non_tool_incomplete_then_complete` |
| 17 | `sep-2322-result-type-included` | `input-required-result-result-type` | `resultType` is EXPLICITLY present (`"input_required"` / `"complete"`), never inferred from the presence of `inputRequests` | `sep_2322_result_type_included` |
| 18 | `sep-2322-not-on-unsupported-requests` | `input-required-result-unsupported-methods` | `input_required` never appears on a method outside `tools/call` / `prompts/get` / `resources/read` | `sep_2322_not_on_unsupported_requests` |
| 19 | `sep-2322-reject-tampered-state` | `input-required-result-tampered-state` | a tampered `requestState` is a JSON-RPC error — never a complete result and never a re-prompt | `sep_2322_reject_tampered_state` |
| 20 | `sep-2322-respect-client-capabilities` | `input-required-result-capability-check` | a server needing an UNDECLARED client capability answers `-32021` at HTTP 400 with an OBJECT-shaped `data.requiredCapabilities` | `input_required_result_capability_check` |
| 21 | `sep-2322-ignore-unexpected-params` | `input-required-result-ignore-extra-params` | unexpected/extra `params` are TOLERATED, on the first call and on the retry | `sep_2322_ignore_unexpected_params` |
| 22 | `sep-2322-validate-input-responses` | `input-required-result-validate-input` | a structurally invalid `inputResponses` map is REJECTED before the handler runs | `sep_2322_validate_input_responses` |
| 23 | `sep-2322-error-on-protocol-error` | `input-required-result-validate-input` | a genuine protocol error surfaces as a JSON-RPC error, not as a re-prompt | `sep_2322_error_on_protocol_error` |

### Row 18 — the concrete behavior pmcp ships

Plan 113-09 made a handler signalling `input_required` where the spec forbids it a LOUD
`-32603 INTERNAL_ERROR` rather than a silently mangled complete result. `tests/v2_mrtr.rs`
asserts BOTH halves of that at the wire level:

- a `tools/list` carrying `requestState` + `inputResponses` is INERT — HTTP 200,
  `resultType: "complete"`, and no `inputRequests` / `requestState` on the result; and
- the same signalling tool invoked on a **v1** request (where MRTR is impossible) answers
  `-32603` with NO `result` at all, and still never leaks `dev.pmcp/mrtr`.

The compile-time half — the exhaustive no-wildcard `client_request_mrtr_eligible` match over
`ClientRequest` — is covered by plan 09's `mod mrtr_egress` unit suite in `src/server/core.rs`
and is not re-asserted here.

---

## pmcp-Added Wire-Shape Rows

Not check ids at the pin. These are the wire facts the suite DEPENDS on but does not name, and
they are the single most likely silent interop failures in this phase, so they are graded here.

| Local test | What it proves | Threat |
|------------|----------------|--------|
| `mrtr_fields_are_params_siblings` | the server accepts `inputResponses` / `requestState` at `params` TOP LEVEL, and a request placing them inside `params._meta` does NOT resume — it re-elicits | T-113-28 |
| `mrtr_retry_uses_different_id` | a retry whose JSON-RPC id differs from the original is accepted and its OWN id is echoed back (including a string id) | T-113-07 |
| `mrtr_signal_key_never_on_wire` | the RAW response body of every exchange in the file contains ZERO occurrences of the pmcp-internal `dev.pmcp/mrtr` signal key | T-113-31 |

## Enforcement

| Local test | What it enforces |
|------------|------------------|
| `manifest_maps_every_pinned_scenario` | this manifest's sha equals `113-SPEC-RECHECK.md` § B.1's; this manifest's check-id set equals § B.2's EXACTLY (both directions); every `Local test` cell above names a fn that exists in `tests/v2_mrtr.rs`; and `## Unmapped` is empty |

## Real-Client Interoperability (Plan 113-11 Task 2)

Not conformance rows — the official suite drives a Node client, so it cannot grade pmcp's OWN
client against pmcp's OWN server. These prove the two halves built in plans 05/07 (client) and
06/09 (server) agree on the wire, each having been tested only against a hand-built counterpart
until now. They drive the SAME fixture server as the scenario mirrors above.

| Local test | What it proves |
|------------|----------------|
| `client_server_mrtr_elicitation_roundtrip` | a real `Client` completes a one-round MRTR exchange; the host handler ran exactly once |
| `client_server_mrtr_three_rounds` | three rounds complete for the caller; the handler ran exactly three times |
| `client_server_mrtr_mixed_kinds` | elicitation + sampling + roots in one result, one invocation of each, one retry |
| `client_server_mrtr_no_session_no_handshake` | the whole MULTI-request loop ran with NO `initialize` observed and NO `Mcp-Session-Id` on any request |
| `client_server_mrtr_round_limit_typed_error` | `mrtr_round_limit(2)` surfaces as a typed error after EXACTLY 2 server-observed requests |
| `client_server_mrtr_outcome_input_required` | an unfulfillable result reaches `call_tool_mrtr` as `MrtrOutcome::InputRequired` after EXACTLY 1 server-observed request (no resend) |
| `client_server_mrtr_existing_method_typed_error` | the same scenario through plain `call_tool` is `Error::input_required_unfulfilled` carrying the full result — explicitly NOT an empty `CallToolResult`; the recovered token OPENS with the server's key |
| `client_server_mrtr_undeclared_capability_is_refused` | **discovered by this plan** — a client with an EMPTY host registry is refused `-32021` rather than handed an `input_required` it could never answer |

### The "no handlers at all" scenario is unreachable between two conformant pmcp peers

Plan 113-11 Task 2 specified the two D-06 tests as "register NO handlers". Wiring the real client
to the real server showed that shape cannot occur, because two independently-correct rules
compose:

* the CLIENT's v2 `clientCapabilities` are REGISTRY-AUTHORITATIVE — it cannot advertise
  `elicitation` without an elicitation handler (capability honesty, HOST-05); and
* the SERVER refuses, all-or-nothing, to emit `inputRequests` for a capability the client did not
  declare, BEFORE minting any continuation (T-113-32, row 20).

So an empty registry gets `-32021`, never an `input_required`. That is the BETTER outcome — no
cryptographic work is spent and the client is told exactly what to declare. The two D-06 tests
therefore use a registered-but-DECLINING elicitation handler, which is the reachable path with
the identical shape (capability declared, server mints, client still cannot fulfil because the
user said no), and the newly-added row above locks the composed behavior so neither rule can
regress unnoticed.

---

## Unmapped

*(This section MUST be empty. A `sep-2322` check id listed here is an UNMEASURED requirement and
the plan is incomplete —* `manifest_maps_every_pinned_scenario` *fails the build if any id from*
`113-SPEC-RECHECK.md` *§ B.2 is missing from the mapping table above.)*

None. All 23 pinned check ids are mapped.

---

## Research-Table Delta

`113-RESEARCH.md`'s table "Official conformance checks this phase is graded on" was cross-checked
against the pinned enumeration. It is **incomplete and partly mis-keyed**, which is exactly why
plan 113-01 forbade deriving this inventory from it.

**Present at the pin, ABSENT from the research table (4):**

| Check id | Obligation the research table did not capture |
|----------|-----------------------------------------------|
| `sep-2322-respect-client-capabilities` | `inputRequests` only for capabilities the client declared |
| `sep-2322-ignore-unexpected-params` | the server must tolerate unexpected/extra params |
| `sep-2322-validate-input-responses` | the server must validate the `inputResponses` map it receives |
| `sep-2322-error-on-protocol-error` | a genuine protocol error must surface as a JSON-RPC error, not a re-prompt |

**Mis-keyed in the research table (1):** it lists `input-required-result-capability-check` as
though it were a check id. It is a scenario CLASS name; the emitted check id is
`sep-2322-respect-client-capabilities` (§ B.3). Row 20 above keys on the check id and names the
test after the class, so both spellings are searchable.

**Present in the research table, ABSENT from the pin: none.** Every id the research table lists
exists at the pin (the table collapses `…-incomplete` / `…-complete` pairs onto one line, which
is a formatting difference, not a delta).

**Consequence:** had this manifest been generated from the research table, four server
obligations — including the `-32021` capability check that carries a wire-visible payload shape
(T-113-32) — would have shipped unmeasured.

---

## Known-Failing Against DRIFT-1

`113-SPEC-RECHECK.md` § D.2 records DRIFT-1 and ADJUDICATES it: pmcp requires `Mcp-Name` on
EVERY v2 request, which is STRICTER than the draft transport spec (which requires it only for
`tools/call`, `resources/read`, `prompts/get`). Phase-112 D-05 stays LOCKED.

The affected conformance scenarios live in `src/scenarios/server/http-standard-headers.ts` at the
pin — every `tools/list` probe there is sent with `Mcp-Method` alone and no `Mcp-Name`, so pmcp
answers `-32020` at HTTP 400 and those header scenarios **cannot pass**.

| Scope | Status |
|-------|--------|
| `src/scenarios/server/http-standard-headers.ts` `tools/list` probes | **KNOWN-FAILING**, cause = DRIFT-1, deliberate |
| The 23 `sep-2322` check ids in the mapping table above | unaffected — none of them omits `Mcp-Name` |

This is NOT a plan-11 defect and plan 11 must NOT "fix" it by loosening the fail-closed header
gate. It is flagged for re-verification against the PUBLISHED 2026-07-28 transport spec (plan 12
Task 3, whose re-verification obligation is binding).

---

*Manifest generated 2026-07-25 by Phase 113 Plan 11 Task 1. Regenerate whenever
`113-SPEC-RECHECK.md` § B is re-pinned — `manifest_maps_every_pinned_scenario` fails until you do.*
