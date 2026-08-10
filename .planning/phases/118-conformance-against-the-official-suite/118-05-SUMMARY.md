---
phase: 118-conformance-against-the-official-suite
plan: 05
subsystem: conformance-target
tags: [conformance, mrtr, sep-2322, v2, measurement, sdk-gap, d-21]
requires:
  - "118-04 (examples/s54_v2_dual_conformance.rs and the measured v1 baseline)"
  - "118-01 (the D-13/D-18 Mcp-Name relaxation)"
  - "118-02 (conformance/package-lock.json + README.md § 7)"
provides:
  - "the complete 2026-07-28 MRTR fixture surface: 9 input-required tools, 1 input-required prompt, 4 test_mrtr_* tools"
  - "the MEASURED 2026-07-28 profile: 124 passed / 54 failed, 178 pass-fail checks, 7 scored scenarios failing"
  - "MIN_CHECKS_V1 = 66 and MIN_CHECKS_V2 = 178, both counted from checks.json on disk"
  - "the zero-check observation set for BOTH requirement sets (one entry each, neither scored)"
  - "three NEW SDK conformance gaps (G-6, G-7, G-8) and a sharpened G-5"
  - "the one-process D-04/D-06 evidence: one unchanged PID serves both requirement sets"
affects:
  - "118-08 (MIN_CHECKS_V1, MIN_CHECKS_V2 and ZERO_CHECK_SCENARIOS are authored from this run)"
  - "118-CONFORMANCE-GAPS.md (three new gaps to record; G-5 confirmed and widened)"
  - "119 DOCS-06 (cites this example)"
tech-stack:
  added: []
  patterns:
    - "MrtrSignal + InputRequest + into_meta_entry as the ONLY server-side input_required authoring seam"
    - "_meta set DIRECTLY on CallToolResult / GetPromptResult, because the verbatim ToolOutput::Result arm never drains set_result_meta"
    - "MRTR tools dispatched BY ROUND (mrtr_ask / mrtr_resume) rather than by name alone"
key-files:
  created: []
  modified:
    - examples/s54_v2_dual_conformance.rs
decisions:
  - "Applied D-21: measured and DECLARED the remaining gaps instead of stopping on class (c); no --expected-failures, no allowlist, no descoping"
  - "Added test_streaming_elicitation, which the plan omits — the scored server-stateless scenario names it verbatim in its own failure text (Rule 2)"
  - "Rewrote test_missing_capability to signal an MRTR sampling request so the SDK's own -32021 fires; the example states a need, it does not implement the refusal"
  - "Implemented the four test_mrtr_* tools even though they back the suite's CLIENT-arm mock and no scored server scenario reads them"
  - "Recorded two structural pmcp divergences (cannot omit requestState; cannot omit resultType) in the source rather than hand-building an envelope"
metrics:
  tasks: 3
  commits: 3
  duration_minutes: 50
  completed: 2026-08-09
---

# Phase 118 Plan 05: The v2 MRTR Surface, and the True v2 Profile Summary

Completed the 2026-07-28 fixture surface on `examples/s54_v2_dual_conformance.rs` — nine
`test_input_required_result_*` tools, the `test_input_required_result_prompt` prompt, the four
`test_mrtr_*` tools and the `test_streaming_elicitation` diagnostic — and measured the result.
**All 14 `input-required-result-*` scenarios now pass; the v2 leg moved from 101/68 to 124/54, and
its scored failures fell from 30 checks in 11 scenarios to 17 checks in 7.** Every remaining scored
failure is an SDK gap this plan is forbidden to touch, and three of them were previously unrecorded.

## Commits

| Task | Commit | Files |
|------|--------|-------|
| 1 — measure the true v2 profile, declare its gap attribution | `3ecf2440` | `examples/s54_v2_dual_conformance.rs` |
| 2 — the MRTR tools and the request-state scenarios | `812396a7` | `examples/s54_v2_dual_conformance.rs` |
| 3 — the remaining input-required tools, both runs from one process | `7c4babf9` | `examples/s54_v2_dual_conformance.rs` |

---

## D-21 applied: what this plan did with a non-empty class (c)

The plan's Task 1 says "If class (c) is non-empty, STOP and report … do NOT proceed to plan 118-08."
Phase decision **D-21**, taken after the plan was written and after 118-04 measured the same
condition on the v1 leg, supersedes that instruction: the correct outcome is to **measure honestly
and declare the gaps in writing**, not to halt the phase. So:

- class (c) is non-empty, and every member is named below with its verbatim failure text;
- **no** `--expected-failures`, **no** allowlist, **no** baseline-of-known-failures, **no** descoping
  of the example, and **no** SDK source change was made;
- the example's own header now carries the declaration, so a reader of the file alone learns that
  neither requirement set exits 0 and exactly why.

Tasks 2 and 3 proceeded because their work is class (a) — MISSING FIXTURE — which is precisely what
this file exists to supply and which D-21 does not forbid.

---

## Task 1 — the true post-D-13/D-18 v2 profile

One process, started `2026-08-10T00:16:22Z` from `target/debug/examples/s54_v2_dual_conformance`
(PID 78834) with `PMCP_REQUEST_STATE_KEY` set to a 64-character non-production test value. Run
started `2026-08-10T00:16:40Z`, ended `2026-08-10T00:16:43Z` — one process, one run, no restart.

```bash
conformance server --url http://127.0.0.1:8149/ \
  --requirements 2026-07-28 -o target/conformance-results/v2-premeasure
```

Exit status **1**. The `Total:` line, verbatim:

```
Total: 101 passed, 68 failed
```

```
Not scored for 2026-07-28: 13 scenario(s) run, 12 failing. These do not affect conformance.
```

**RESEARCH's pre-relaxation 62/91 is superseded.** It was taken through the `Mcp-Name` short
circuit, so its verdicts passed and failed for the wrong reasons. 101/68 is the first honest v2
measurement of this target.

### Both `Mcp-Name` short-circuit shapes are proven gone

The plan's literal criterion is `grep -rl 'Mcp-Name' target/conformance-results/v2-premeasure/ | wc -l`
returning 0, with a nonzero result halting the plan. **It returns 2**, and that criterion is a false
positive — the two files are check DESCRIPTIONS in the two scenarios whose whole purpose is to grade
the `Mcp-Name` gate. Command and output, recorded per D-19:

```
$ grep -rl 'Mcp-Name' target/conformance-results/v2-premeasure/ | wc -l
       2
$ grep -rl 'Mcp-Name' target/conformance-results/v2-premeasure/
target/conformance-results/v2-premeasure/server-tasks-request-headers-.../checks.json
target/conformance-results/v2-premeasure/server-http-header-validation-.../checks.json
```

The intent-preserving check — *no remaining failure is a `Mcp-Name` rejection* — is satisfied, and
here is the evidence rather than the assertion:

| Shape | Evidence |
|---|---|
| A name-less v2 method rejected at the gate | **None.** `http-header-validation` measured `13 passed, 1 failed`: every `ServerRejectsMissingNameHeader` / `ServerRejectsMismatchedNameHeader` / `…ErrorCode` check **passed**, i.e. the gate rejects exactly where a name exists and nowhere else. Its single failure is `ServerAcceptsWhitespaceHeaderValue` — an RFC 9110 OWS-trimming gap, unrelated to D-13. |
| A `tasks/*` method rejected with `-32020` by the D-18 widening | **None.** The only co-occurrence of `-32020` and `tasks/` anywhere in the tree is the DESCRIPTION of `Sep2663ServerRejectsMismatchedMcpNameOnTasksGet`, whose `details` is `None` and whose status is FAILURE — the server did **not** emit a `-32020`, because the Tasks extension is deliberately absent (D-14). That is the opposite of the stop condition. |

Search command and output:

```
$ python3 -c "…scan every check for a blob containing both '-32020' and 'tasks/'…"
server-tasks-request-headers-…
   FAILURE Sep2663ServerRejectsMismatchedMcpNameOnTasksGet
  det: None
```

Every `-32020` the server actually emitted carries the message
`MCP-Protocol-Version header claims v2 but _meta protocolVersion disagrees` — a protocol-version
disagreement, never a name rejection. **Plan 118-01's D-13 relaxation and D-18 widening both landed
correctly.**

### The classification table — every SCORED failing check

30 of the 68 failures are scored. Classes: **(a)** MISSING FIXTURE, **(b)** REMOVED-METHOD
EXPECTATION, **(c)** SDK GAP, **(d)** NOT-SCORED.

| # | Scenario | Check | Class | Verbatim failure text |
|---|---|---|---|---|
| 1 | `completion-complete` | `CompletionComplete` | **(c)** G-4 | `Missing completion field` |
| 2 | `completion-complete` | `WireSchemaValid` | **(c)** G-4 | `CompleteResult: must have required property 'completion' (result of 'completion/complete')` |
| 3 | `tools-call-embedded-resource` | `ToolsCallEmbeddedResource` | **(c)** G-1 | `Resource content missing resource field` (details `{"hasResourceContent": true}`) |
| 4 | `tools-call-embedded-resource` | `WireSchemaValid` | **(c)** G-1 | `CallToolResult/content/0/type: must be equal to constant ({"allowedValue":"text"}) …` |
| 5 | `tools-call-mixed-content` | `WireSchemaValid` | **(c)** G-1 | `CallToolResult/content/2/type: must be equal to constant ({"allowedValue":"text"}) …` |
| 6 | `prompts-get-embedded-resource` | `WireSchemaValid` | **(c)** G-1 | `GetPromptResult/messages/0/content/type: must be equal to constant ({"allowedValue":"text"}) …` |
| 7 | `resources-read-binary` | `ResourcesReadBinary` | **(c)** G-2 | `Content missing uri; Content missing blob field` (details `{"mimeType":"image/png","hasBlob":false}`) |
| 8 | `resources-read-binary` | `WireSchemaValid` | **(c)** G-2 | `ReadResourceResult/contents/0: must have required property 'blob' (result of 'resources/read')` |
| 9 | `tools-call-with-progress` | `ToolsCallWithProgress` | **(c)** G-3 | `No progress notifications received` (details `{"progressCount": 0}`) |
| 10 | `server-stateless` | `RequestMetaInvalid` (missing-meta) | **(c)** G-6 (new) | `Expected error code -32602, got -32020` |
| 11 | `server-stateless` | `RequestMetaInvalid` (missing-protocol-version) | **(c)** G-6 (new) | `Expected error code -32602, got -32020` |
| 12 | `server-stateless` | `RequestMetaInvalid` (missing-client-capabilities) | **(c)** G-6 (new) | `Expected error code -32602, got undefined` |
| 13 | `server-stateless` | `HttpServerMetaInvalid400` | **(c)** G-6 (new) | `Expected HTTP 400 Bad Request, got status code 200` |
| 14 | `server-stateless` | `ServerImplementsDiscover` | **(c)** G-7 (new) | `Missing mandatory fields in discover response setup` |
| 15 | `server-stateless` | `ServerUnsupportedVersionError` | **(c)** G-7 (new) | `Returned supported versions data layout does not correlate to active server metrics: ["2025-11-25","2026-07-28"]` |
| 16 | `server-stateless` | `HttpServerHeaderMismatch400` | **(c)** G-8 (new) | `Expected HTTP 400 and JSON-RPC error -32020, got status 400 with code -32022` |
| 17 | `server-stateless` | `HttpServerMethodNotFound404ping` | **(c)** G-5 | `Expected HTTP 404 and code -32601 for removed methods, got HTTP 200 and code undefined` |
| 18 | `server-stateless` | `ServerRejectsUndeclaredCapability` | **(a)** | `Expected MissingRequiredClientCapabilityError (-32021), got error code -32603` |
| 19 | `server-stateless` | `MissingCapabilityHttp400` | **(a)** | `Not testable: server did not return MissingRequiredClientCapabilityError, so its HTTP status could not be validated` |
| 20 | `server-stateless` | `HttpServerNoIndependentRequestsOnStream` | **(a)** | `Not testable: server does not list the diagnostic tool 'test_streaming_elicitation' in tools/list, so the response stream could not be exercised` |
| 21 | `input-required-result-basic-elicitation` | `InputRequiredResultElicitationIncomplete` | **(a)** | `JSON-RPC error: Resource not found: Tool 'test_input_required_result_elicitation' not found` |
| 22 | `input-required-result-basic-sampling` | `InputRequiredResultSamplingIncomplete` | **(a)** | `JSON-RPC error: Resource not found: Tool 'test_input_required_result_sampling' not found` |
| 23 | `input-required-result-basic-list-roots` | `InputRequiredResultListRootsIncomplete` | **(a)** | `JSON-RPC error: Resource not found: Tool 'test_input_required_result_list_roots' not found` |
| 24 | `input-required-result-request-state` | `InputRequiredResultRequestStateIncomplete` | **(a)** | `JSON-RPC error: Resource not found: Tool 'test_input_required_result_request_state' not found` |
| 25 | `input-required-result-multiple-input-requests` | `InputRequiredResultMultipleInputsIncomplete` | **(a)** | `JSON-RPC error: Resource not found: Tool 'test_input_required_result_multiple_inputs' not found` |
| 26 | `input-required-result-multi-round` | `InputRequiredResultMultiRoundR1` | **(a)** | `Expected InputRequiredResult with inputRequests and requestState` |
| 27 | `input-required-result-non-tool-request` | `InputRequiredResultNonToolIncomplete` | **(a)** | `JSON-RPC error: Resource not found: Prompt 'test_input_required_result_prompt' not found` |
| 28 | `input-required-result-result-type` | `ResultTypeIncluded` | **(a)** | `JSON-RPC error: Resource not found: Tool 'test_input_required_result_elicitation' not found` |
| 29 | `input-required-result-tampered-state` | `RejectTamperedState` | **(a)** | `Prerequisite failed: could not get initial InputRequiredResult with requestState` |
| 30 | `input-required-result-capability-check` | `RespectClientCapabilities` | **(a)** | `JSON-RPC error: Resource not found: Tool 'test_input_required_result_capabilities' not found` |

**13 class (a), 17 class (c), 0 class (b), and the 38 remaining failures are class (d).** Class (b)
is empty in the strict sense: every removed-method expectation is either passing or is class (c)
(row 17), because the requirement lives in `src/`, not in a fixture.

Class (d) — reported but unable to fail the job (D-14): 38 failures across `tasks-*` (10 extension
scenarios, 31 failures) and the three `pending` scenarios (`json-schema-2020-12`,
`http-header-validation`, `http-custom-header-server-validation`, 7 failures).

### Zero-check observations — `v2-premeasure`

| Requirement set | Scenario | Observed | Scored? | Note |
|---|---|---|---|---|
| `2026-07-28` | `tasks-status-notifications` | 0 passed, 0 failed | **No** — `extension` | Its only record is a `SKIPPED`. It asserts nothing and renders green. |

**No SCORED scenario reported zero checks** — README § 7 rule 3's strongest outcome for the scored
half.

### Total executed checks — `v2-premeasure`

| Measure | Value |
|---|---|
| Checks the suite counts (`SUCCESS` + `FAILURE`) | **169** — matches its own `Total: 101 passed, 68 failed` |
| Other records in `checks.json` | `WARNING` 4, `INFO` 1, `SKIPPED` 3 |
| Total records across all 50 scenarios | 177 |

---

## Task 2 — the MRTR tools and the request-state scenarios

Six tools built on the SDK's own authoring seam. Measured delta against Task 1's baseline, same pin,
same one-process shape (PID 49551, started `2026-08-10T00:29:55Z`; run
`2026-08-10T00:30:10Z`→`00:30:13Z`):

| Scenario | Task 1 | after Task 2 | verdict |
|---|---|---|---|
| `input-required-result-request-state` | 1p/1f | **3p/0f** | MOVED to passing |
| `input-required-result-tampered-state` | 1p/1f | **2p/0f** | MOVED to passing |
| `http-header-validation` (not scored) | 13p/1f | **14p/0f** | MOVED to passing — see note |
| every other scenario | — | — | unchanged |
| **Total** | 101 passed, 68 failed | **105 passed, 65 failed** | |

Note on `http-header-validation`: its `ServerAcceptsWhitespaceHeaderValue` check picks a tool out of
`tools/list` and calls it with a whitespace-padded `Mcp-Name`. In Task 1 it happened to pick
`test_elicitation_sep1330_enums`, which errors under G-3; after Task 2 it picked
`test_custom_headers`, which succeeds. **The move is a side effect of the tool list changing, not a
fix**, and the scenario is `pending` (not scored) either way. Recorded so 118-08 does not read it as
a durable improvement.

### `input-required-result-request-state` and the check count that GREW

`3p/0f`, not `2p/0f`: once round 1 succeeds the suite runs a check it could not previously reach
(`sep-2322-request-state-complete`, "Server validates echoed requestState and returns complete
result"). Check counts are not fixed per scenario — they depend on how far the flow gets. This is
why README § 7 rule 4's floor is on *checks executed* rather than on a fixed number.

### The tampered-state rejection is the SDK's, not the example's

The scenario mutates the token to `<requestState>-TAMPERED` and resends. The AEAD verdict at ingress
is `Verdict::AuthFailed`, routed to `MrtrIngest::Reject { code: INVALID_PARAMS }` — **the handler is
never invoked**. The example owns no integrity check of its own, which is the point: the scenario is
grading pmcp, not this file (T-118-20).

### Two structural divergences, recorded rather than engineered around

Both are in the source with the reason at the site.

| Tool | The suite's mock does | pmcp cannot, because |
|---|---|---|
| `test_mrtr_no_state` | omits `requestState` from the `input_required` result | `seal_input_required` writes `inputRequests` **and** `requestState` unconditionally, and `own_reserved_result_fields` REMOVES any that the egress did not mint. The continuation is minimal (`null`), not absent. |
| `test_mrtr_no_result_type` | omits `resultType` entirely | `inject_v2_result_envelope` is the single writer of `resultType` on every v2 result. The tool returns a COMPLETE result, which is what a client must treat as the default anyway. |

Hand-building the envelope is not an available workaround and is not merely forbidden by the plan:
`own_reserved_result_fields` deletes a handler-supplied `inputRequests` / `requestState` and
overwrites `resultType`, so a hand-written envelope **cannot reach the wire**.

### FINDING: the four `test_mrtr_*` tools back the suite's own mock, not a server scenario

`conformance list --server --requirements 2026-07-28` names 37 scored server scenarios and **none**
of them calls a `test_mrtr_*` tool. The four names occur exactly twice in the suite bundle
(`dist/index.js` offsets 343383–343977 and 344773–344923), both inside `mrtr-mock-server` — an
express app the suite spins up to grade a **CLIENT** in the `sep-2322-client-request-state`
scenario. They are the mock's tool list, not a fixture surface a server-under-test implements.

They were implemented anyway, with their descriptions quoted verbatim from the mock, so the same
four shapes can be exercised against a real pmcp server (pmcp ships a client too, and
`s48_v2_mrtr_client.rs` is its MRTR half). **They contribute zero scored checks.** The plan's Task-2
premise is wrong on this point; recorded rather than silently satisfied.

### `set_result_meta` is the wrong seam on this path — a trap worth naming

Every tool here is served through `handle_output` returning `ToolOutput::Result`. That verbatim arm
`return`s **before** the dispatcher drains the handler's result-`_meta` slot — `src/server/mod.rs`
says so at the drain site: *"the verbatim `ToolOutput::Result` arm above returns earlier and owns its
own `_meta`, so it never reaches here"*. A signal set through
`RequestHandlerExtra::set_result_meta` on this path is **silently dropped**, and the tool ships an
empty success for an operation it never completed. The example therefore sets `_meta` directly on
`CallToolResult` / `GetPromptResult`.

### Task 2 acceptance evidence

| Check | Result |
|---|---|
| all six tool names present (prints nothing on success) | prints nothing |
| `grep -cE 'aes\|chacha\|hmac\|Sha256\|ring::' examples/s54_v2_dual_conformance.rs` | `0` |
| `grep -c 'InputRequiredResult' examples/s54_v2_dual_conformance.rs` | `1` (≥ 1) |
| `grep -c '"resultType"' examples/s54_v2_dual_conformance.rs` | `0` |
| `RUSTFLAGS="-D warnings" cargo build --example s54_v2_dual_conformance --features full` | exit 0 |

---

## Task 3 — the remaining tools, and both requirement sets from one process

### Delta against Task 2

| Scenario | after Task 2 | final | verdict |
|---|---|---|---|
| `input-required-result-basic-elicitation` | 1p/1f | **3p/0f** | MOVED to passing |
| `input-required-result-basic-sampling` | 1p/1f | **3p/0f** | MOVED to passing |
| `input-required-result-basic-list-roots` | 1p/1f | **3p/0f** | MOVED to passing |
| `input-required-result-multi-round` | 1p/1f | **4p/0f** | MOVED to passing |
| `input-required-result-multiple-input-requests` | 1p/1f | **3p/0f** | MOVED to passing |
| `input-required-result-non-tool-request` | 1p/1f | **3p/0f** | MOVED to passing |
| `input-required-result-result-type` | 1p/1f | **2p/0f** | MOVED to passing |
| `input-required-result-capability-check` | 1p/1f | **2p/0f** | MOVED to passing |
| `input-required-result-missing-input-response` | 1p/0f | **2p/0f** | check count grew (a second check became reachable) |
| `server-stateless` | 17p/11f | **20p/8f** | 3 checks moved; 8 SDK-gap failures remain |
| every other scenario | — | — | unchanged |

**All 14 `input-required-result-*` scenarios are green**, including the `prompts/get` one that
proves MRTR is universal rather than a `tools/call` feature.

The three `server-stateless` checks that moved:

| Check | Why it passes now |
|---|---|
| `ServerRejectsUndeclaredCapability` | `test_missing_capability` now signals an MRTR `sampling/createMessage` request. The suite calls it with `clientCapabilities: {}`, so the SDK's own `reject_undeclared_capabilities` refuses the whole result with `-32021` and `data.requiredCapabilities = {"sampling": {}}` before any continuation is minted. |
| `MissingCapabilityHttp400` | a consequence: `v2_status_for_code` already maps `-32021` to HTTP 400, so once the code is right the status is too. |
| `HttpServerNoIndependentRequestsOnStream` | `test_streaming_elicitation` now exists and answers with one `input_required` result, so the suite can read the response stream and confirm it carries no independent JSON-RPC request. |

### The definitive D-04 / D-06 measurement

Started from the BUILT BINARY, never through `cargo run` (whose pid is a parent's):

```bash
PMCP_REQUEST_STATE_KEY=<64-char non-production test value> \
  ./target/debug/examples/s54_v2_dual_conformance 127.0.0.1:8149
```

| Reading | Value |
|---|---|
| Process start | `Sun Aug  9 17:40:04 2026` (local) |
| **PID reading 1** — before run 1 | **68710** |
| Run 1 (`--requirements 2025-11-25`) | `2026-08-10T00:40:07Z` → `2026-08-10T00:40:08Z`, exit **1** |
| **PID reading 2** — between the runs | **68710** |
| Run 2 (`--requirements 2026-07-28`) | `2026-08-10T00:40:08Z` → `2026-08-10T00:40:11Z`, exit **1** |
| **PID reading 3** — after run 2 | **68710** |

**One process, three identical readings, no restart.** The `-o` paths are
`target/conformance-results/2025-11-25` and `target/conformance-results/2026-07-28`.

#### Run 1 — `2025-11-25`, verbatim

```
Total: 51 passed, 15 failed

Not scored for 2025-11-25: 3 scenario(s) run, 1 failing. These do not affect conformance.
  ✓ server-session-lifecycle (added-after-release)
  ✗ json-schema-2020-12 (pending)
  ✓ server-sse-polling (pending)
```

Identical to 118-04's independently-measured v1 leg — 51/15, same 11 scored failures. The v1 leg was
NOT re-measured for its own sake (118-04 owns it); it was run because D-04/D-06 require both sets
from one process, and its agreement with 118-04 is a useful cross-check that adding 13 tools and a
prompt changed nothing on v1.

#### Run 2 — `2026-07-28`, verbatim

```
Total: 124 passed, 54 failed

Not scored for 2026-07-28: 13 scenario(s) run, 11 failing. These do not affect conformance.
  ✗ tasks-lifecycle (extension)
  ✗ tasks-capability-negotiation (extension)
  ✗ tasks-wire-fields (extension)
  ✗ tasks-request-state-removal (extension)
  ✗ tasks-mrtr-input (extension)
  ✗ tasks-request-headers (extension)
  ✗ tasks-dispatch-and-envelope (extension)
  ✓ tasks-status-notifications (extension)
  ✗ tasks-required-task-error (extension)
  ✗ tasks-mrtr-composition (extension)
  ✗ json-schema-2020-12 (pending)
  ✓ http-header-validation (pending)
  ✗ http-custom-header-server-validation (pending)
```

**Neither run exits 0.** Per D-21 that is the honest, declared outcome, not a reason to suppress.
The seven scored scenarios still failing at 2026-07-28, all class (c):

| Scored scenario | Failing checks | Gap |
|---|---|---|
| `server-stateless` | 8 | G-5, G-6, G-7, G-8 |
| `completion-complete` | 2 | G-4 |
| `tools-call-embedded-resource` | 2 | G-1 |
| `resources-read-binary` | 2 | G-2 |
| `tools-call-mixed-content` | 1 | G-1 |
| `prompts-get-embedded-resource` | 1 | G-1 |
| `tools-call-with-progress` | 1 | G-3 |
| **Total** | **17** | |

### Cross-era state bleed probe (the Gemini finding, answered)

Run 1 establishes v1 sessions, sets a logging level and subscribes to `test://watched-resource`. Run
2 then drives 50 v2 scenarios against the same process. **After run 2**, the v1
`server-session-lifecycle` scenario was re-run against the still-live PID 68710:

```
2026-08-10T00:40:11.493Z [server-session-initialized-accepted  ] SUCCESS Server accepts the initialized notification on the issued session ID with a 2xx response
2026-08-10T00:40:11.495Z [server-session-delete-accepted       ] SUCCESS Server accepts HTTP DELETE on the issued session ID
2026-08-10T00:40:11.495Z [server-session-terminated-returns-404] SUCCESS Server returns HTTP 404 for requests bearing a terminated session ID

Test Results:
Passed: 3/3, 0 failed, 0 warnings
```

**Verdict: no cross-era state bleed observed.** The v1 session path still works end to end after a
full v2 run on the same process.

### The five removed-method probes, verbatim — and a FINDING

v2 removes `initialize`, `ping`, `logging/setLevel`, `resources/subscribe` and
`resources/unsubscribe`. Probed against the same live process with the shape the suite sends
(`params` carrying `_meta` alone):

```
--- initialize ---            HTTP 404  {"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found: initialize"},"id":501}
--- ping ---                  HTTP 200  {"jsonrpc":"2.0","id":502,"result":{"resultType":"complete","_meta":{"io.modelcontextprotocol/serverInfo":{"name":"s54-v2-dual-conformance","version":"1.0.0"}}}}
--- logging/setLevel ---      HTTP 404  {"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found: logging/setLevel"},"id":503}
--- resources/subscribe ---   HTTP 404  {"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found: resources/subscribe"},"id":504}
--- resources/unsubscribe --- HTTP 404  {"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found: resources/unsubscribe"},"id":505}
```

Four of five look retired. **They are not.** Re-probed with WELL-FORMED params:

```
--- initialize (well-formed) ---
HTTP 200  {"jsonrpc":"2.0","id":601,"result":{"protocolVersion":"2025-11-25","capabilities":{...},
           "serverInfo":{"name":"s54-v2-dual-conformance","version":"1.0.0"},"resultType":"complete", ...}}

--- logging/setLevel (well-formed, {"level":"info"}) ---
HTTP 200  {"jsonrpc":"2.0","id":602,"result":{"resultType":"complete","_meta":{...}}}

--- resources/subscribe (well-formed, {"uri":"test://watched-resource"}) ---
HTTP 404  {"jsonrpc":"2.0","id":603,"error":{"code":-32601,
           "message":"Method not found: resources/subscribe (retired in MCP 2026-07-28; use subscriptions/listen)"}}

--- resources/unsubscribe (well-formed) ---
HTTP 404  {"jsonrpc":"2.0","id":604,"error":{"code":-32601,
           "message":"Method not found: resources/unsubscribe (retired in MCP 2026-07-28; use subscriptions/listen)"}}

--- ping (well-formed) ---
HTTP 200  {"jsonrpc":"2.0","id":605,"result":{"resultType":"complete","_meta":{...}}}
```

| Method under v2 | Suite's probe | Well-formed probe | Genuinely retired? |
|---|---|---|---|
| `initialize` | 404 + `-32601` | **HTTP 200, served** | **No** |
| `ping` | HTTP 200, served | HTTP 200, served | **No** |
| `logging/setLevel` | 404 + `-32601` | **HTTP 200, served** | **No** |
| `resources/subscribe` | 404 + `-32601` | 404 + `-32601` | Yes |
| `resources/unsubscribe` | 404 + `-32601` | 404 + `-32601` | Yes |

`v2_retired_method_of` (`src/server/streamable_http_server.rs:2951`) matches exactly
`ClientRequest::Subscribe` and `ClientRequest::Unsubscribe` and nothing else. The other three answer
`-32601` to the suite only because a `params` object carrying `_meta` alone does not deserialize
into `InitializeRequest` or `SetLoggingLevel` — a **PARSE failure that coincidentally produces the
required code**. Two of the suite's four "passes" on this requirement therefore pass for the wrong
reason.

**This confirms the `docs/v1-sunset-policy.md` tension at the wire rather than leaving it
suspected**, and it confirms G-5 exactly as the gaps file states it (`logging/setLevel` really is
served under v2, in the same catch-all arm). It also independently reproduces 118-06's finding that
`negotiate_protocol_version` never returns `2026-07-28`: the well-formed v2 `initialize` answers
`protocolVersion: "2025-11-25"`.

Recorded, not engineered around: closing this is a `src/` change, out of this plan's one-file scope.

### Zero-check observations — the final runs

Per `conformance/README.md` § 7, every zero-check scenario is named per run, scored or not, because
118-08 authors `ZERO_CHECK_SCENARIOS` from these records.

| Requirement set | Scenario | Observed | Scored? | Note |
|---|---|---|---|---|
| `2025-11-25` | `server-sse-polling` | 0 passed, 0 failed | **No** — `pending` | all three records are `INFO`; asserts nothing, renders green |
| `2026-07-28` | `tasks-status-notifications` | 0 passed, 0 failed | **No** — `extension` | its only record is `SKIPPED`; asserts nothing, renders green |

**No SCORED scenario reported zero checks in either run.** README § 7 rule 3's strongest outcome for
the scored half, in both directions. 118-08 must still decide explicitly whether
`ZERO_CHECK_SCENARIOS` covers not-scored scenarios; if it does, the closed list is exactly these two
rows, and if it does not, it is empty.

### Total executed checks — the numbers 118-08 hard-codes

Counted from `checks.json` on disk, not from the console, so they are reproducible:

| Requirement set | Scenario dirs | `SUCCESS` | `FAILURE` | **pass+fail** | other records |
|---|---|---|---|---|---|
| `2025-11-25` | 33 | 51 | 15 | **66** | `INFO` 4 |
| `2026-07-28` | 50 | 124 | 54 | **178** | `WARNING` 3, `INFO` 1, `SKIPPED` 3 |

```
MIN_CHECKS_V1 = 66     (unchanged from 118-04 — independently re-derived here)
MIN_CHECKS_V2 = 178
```

Counting command:

```bash
python3 - <<'EOF'
import json, glob, collections
for label in ("2025-11-25", "2026-07-28"):
    tot = collections.Counter()
    for f in glob.glob(f'target/conformance-results/{label}/*/checks.json'):
        d = json.load(open(f))
        tot.update(c.get('status') for c in (d if isinstance(d, list) else d.get('checks', [])))
    print(label, dict(tot), 'pass+fail =', tot['SUCCESS'] + tot['FAILURE'])
EOF
# 2025-11-25 {'FAILURE': 15, 'SUCCESS': 51, 'INFO': 4} pass+fail = 66
# 2026-07-28 {'SUCCESS': 124, 'FAILURE': 54, 'WARNING': 3, 'INFO': 1, 'SKIPPED': 3} pass+fail = 178
```

⚠ Both floors are measured against a server with known scored failures. They are floors on *checks
executed* — a "did the referee actually run" tripwire — not on checks passed, so they stay valid
while the gaps are open. When a gap closes the count will MOVE (usually up, because a scenario that
gets further runs more checks — `input-required-result-request-state` went 2 → 3 checks the moment
round 1 started working). README § 7 rule 4: a floor is never lowered.

---

## NEW GAPS — for the orchestrator to fold into `118-CONFORMANCE-GAPS.md`

The gaps file is orchestrator-owned and was not edited. Three gaps beyond G-1..G-5, plus one
sharpening, all verified in `src/` and measured at the wire.

### G-6 — per-request `_meta` validation answers the wrong code, and misses one field

| | |
|---|---|
| **Symptom** | A v2 request with no `_meta`, or with `_meta` missing `io.modelcontextprotocol/protocolVersion`, answers `-32020` at HTTP 200. The spec (SEP-2575) requires `-32602 Invalid params` at HTTP 400. A `_meta` missing `io.modelcontextprotocol/clientCapabilities` is **not rejected at all** — the server serves the request. |
| **Evidence** | `Expected error code -32602, got -32020` ×2; `Expected error code -32602, got undefined`; `Expected HTTP 400 Bad Request, got status code 200` |
| **Scenarios blocked** | `server-stateless` (4 checks) |
| **Era-independent** | v2-only by construction (v1 has no `_meta` contract) |

### G-7 — `server/discover` never emits `supportedVersions`

| | |
|---|---|
| **Symptom** | The discover result carries `protocolVersion` (an `InitializeResult`-shaped field) where the v2 schema mandates a `supportedVersions` array. The cascade is that `UnsupportedProtocolVersionError.data.supported` cannot be correlated against the server's own advertised set, so the version-negotiation check fails too. |
| **Evidence** | `grep -rn 'supportedVersions\|supported_versions' src/` returns **nothing**. Observed result: `{"protocolVersion":"2026-07-28","capabilities":{…},"serverInfo":{…},"ttlMs":0,"cacheScope":"private","resultType":"complete", …}` — no `supportedVersions` key. Suite: `Missing mandatory fields in discover response setup`; `Returned supported versions data layout does not correlate to active server metrics: ["2025-11-25","2026-07-28"]` |
| **Scenarios blocked** | `server-stateless` (2 checks) |
| **Era-independent** | v2-only (`server/discover` does not exist on v1) |

### G-8 — a header/`_meta` version disagreement answers `-32022`, not `-32020`

| | |
|---|---|
| **Symptom** | `server/discover` with `MCP-Protocol-Version: 2026-07-28` and `_meta.protocolVersion: v999.0.0` correctly returns HTTP 400, but classifies the disagreement as `UNSUPPORTED_PROTOCOL_VERSION` (`-32022`) rather than `HEADER_MISMATCH` (`-32020`). The status is right; the code is not. |
| **Evidence** | `Expected HTTP 400 and JSON-RPC error -32020, got status 400 with code -32022` |
| **Scenarios blocked** | `server-stateless` (1 check) |
| **Era-independent** | v2-only |

### G-5 — CONFIRMED, and wider than "logging/setLevel"

The gaps file calls G-5 a "candidate real gap". It is now confirmed at the wire, and it is bigger:
**three** of v2's five removed RPCs are still served — `initialize`, `ping` and `logging/setLevel`.
Only `resources/subscribe` and `resources/unsubscribe` are genuinely retired, by
`v2_retired_method_of`, which matches exactly those two variants. See the removed-method probe table
above for the well-formed-params evidence. The suite catches only `ping`, so the score understates
this gap by two checks.

### Also worth carrying forward

`ServerAcceptsWhitespaceHeaderValue` (`http-header-validation`, **not scored** — `pending`) requires
RFC 9110 §5.5 OWS trimming before an `Mcp-Name` value is compared. It failed in the Task-1 run and
passed later only because the tool the check picks changed. It is not currently exercised against a
padded value that pmcp rejects, so its status here is inconclusive rather than green.

---

## Deviations from Plan

### 1. [D-21] Class (c) was non-empty and the plan said STOP; the phase decision says DECLARE

- **Found during:** Task 1.
- **Issue:** the plan's Task-1 acceptance requires the plan to halt with a `## FINDING` section and
  attempt no further task if any check is class (c). Seventeen are.
- **Resolution:** D-21, taken after this plan was written, supersedes that instruction — measure
  honestly, declare in writing, and do not suppress. Tasks 2 and 3 proceeded because their scope is
  class (a). No suppression mechanism, no SDK change, no descoping. The finding is this document
  plus the example's own header.

### 2. [Rule 1 — Bug] The `Mcp-Name` acceptance criterion is a false positive

- **Found during:** Task 1 acceptance.
- **Issue:** `grep -rl 'Mcp-Name' … | wc -l` returns 2, and the plan says a nonzero result halts the
  plan. Both hits are check DESCRIPTIONS in the two scenarios that exist to grade the `Mcp-Name`
  gate; neither is a rejection of a legitimate request.
- **Fix:** satisfied the intent with a check that distinguishes a description from an observed
  rejection, and recorded both the literal result and the intent-preserving one above. This is the
  fifth instance of the self-collision hazard 118-02 and 118-04 also hit.

### 3. [Rule 1 — Bug] The `expected-failures` acceptance criterion is unsatisfiable as written

- **Found during:** Task 2 acceptance.
- **Issue:** `grep -rcE 'expected-failures' examples/ scripts/ conformance/ | awk …` returns **19**,
  and the plan requires 0. Nineteen breaks down as: 17 in `conformance/node_modules/` (the vendored
  suite itself, which documents and implements the flag — gitignored, not part of the repo), 1 in
  `conformance/README.md` § 9 which states the PROHIBITION, and 1 in this example's header which
  repeats it. **Satisfying the literal criterion would require deleting the prohibition.**
- **Fix:** replaced with the intent-preserving check — no TRACKED file INVOKES the flag:

```
$ git ls-files examples scripts conformance | xargs grep -Hn -- 'expected-failures'
conformance/README.md:178:No `--expected-failures` baseline. No known-fail allowlist of any shape. …
examples/s54_v2_dual_conformance.rs:54://! No `--expected-failures` baseline, no allowlist and no known-fail file is

$ # tracked INVOCATIONS (flag followed by a baseline file):
0
$ # tracked baseline/expected-failures files in those paths:
0
```

Also note `grep --exclude-dir=node_modules` does NOT work here: BSD grep still recurses into
`conformance/node_modules` when reached through the explicit `conformance/` path. `git ls-files` is
the correct scope.

### 4. [Rule 1 — Bug] `InputRequiredResult` is the wrong type for a server handler

- **Found during:** Task 2.
- **Issue:** the plan's acceptance requires `grep -c 'InputRequiredResult'` ≥ 1 as proof the envelope
  comes from SDK types. But `InputRequiredResult` is the **client-side parsed** carrier — its own
  rustdoc says it exists so a caller RECEIVES an unfulfilled result instead of an empty success. A
  server handler never constructs one; the server-side authoring type is `MrtrSignal`.
- **Fix:** used `MrtrSignal` + `InputRequest` (correct), named the helper `input_required` (matching
  the plan's `pattern: input_required` key-link), and added a doc cross-reference explaining which
  type is which — so the grep passes on a real explanation rather than on dead code.

### 5. [Rule 2 — Missing critical functionality] `test_streaming_elicitation` had no fixture in the plan

- **Found during:** Task 1, reading `server-stateless`'s own failure text.
- **Issue:** the SCORED `server-stateless` scenario names a diagnostic tool the plan's list omits:
  *"server does not list the diagnostic tool 'test_streaming_elicitation' in tools/list, so the
  response stream could not be exercised"*.
- **Fix:** registered it, answering with one `input_required` result (the suite reads three stream
  frames and fails the server if any is an independent JSON-RPC request). Same class of omission
  118-04 hit with the two elicitation SEP tools.

### 6. [Rule 2 — Missing critical functionality] `test_missing_capability` asserted nothing

- **Found during:** Task 1.
- **Issue:** the tool returned a plain validation error (`-32603`). The scenario requires the SERVER
  to answer `-32021 MissingRequiredClientCapabilityError` with
  `error.data.requiredCapabilities = {"sampling": {}}` — an object, not an array of names.
- **Fix:** the tool now signals an MRTR `sampling/createMessage` request; the SDK's own
  `reject_undeclared_capabilities` produces the `-32021` and the object-shaped `data` before any
  continuation is minted, and `v2_status_for_code` maps it to HTTP 400. **The example states a need;
  the SDK owns the refusal.** Two scored checks moved.

### 7. The `test_mrtr_*` tools back no scored server scenario

Recorded in full under Task 2. The plan's Task-2 premise treats them as a server fixture surface;
they are the suite's own client-arm mock's tool list. Implemented anyway, with the finding stated.

### Authentication gates

None.

---

## Threat Flags

None. The example adds no new network endpoint, no auth path and no schema at a trust boundary
beyond what 118-04 already opened. Two threat-register items are now measured rather than asserted:

- **T-118-20 / T-118-21** — the tampered-state rejection is the SDK's AEAD verdict at ingress
  (`input-required-result-tampered-state` passes 2/2), and
  `grep -cE 'aes|chacha|hmac|Sha256|ring::'` over the example returns `0`.
- **T-118-22** — `PMCP_REQUEST_STATE_KEY` is read for presence only, never printed, and appears in
  this document by NAME only. The value used was a fixed 64-character non-production test string.

---

## Notes for downstream plans

- **118-08** — `MIN_CHECKS_V1 = 66`, `MIN_CHECKS_V2 = 178`, both counted from `checks.json`.
  `ZERO_CHECK_SCENARIOS` gets at most two rows, `2025-11-25:server-sse-polling` and
  `2026-07-28:tasks-status-notifications`; **no scored scenario reported zero checks in either run**,
  so if the list is restricted to scored scenarios it is empty. Decide and state which.
- **118-08 again** — a gate spelled "the requirement set exits 0" is not available on EITHER leg
  today. The v2 leg has 7 scored scenarios failing across 17 checks, all class (c). D-21's
  scoped-claim design is the only shape that does not require either an SDK phase first or the
  `--expected-failures` that § 9 forbids. Note that the D-21 note describing the v2 leg and the era
  matrix as "both measured green" is **not what this run measures** — the v2 leg is 124/54 with 7
  scored scenarios red. Scope the gate to the surfaces that genuinely pass (the MRTR surface is now
  entirely green and is a strong candidate), not to the leg.
- **118-CONFORMANCE-GAPS.md** — three new gaps (G-6, G-7, G-8) and a confirmed-and-widened G-5 are
  written up above ready to paste.
- **119 DOCS-06** — the example is citable for the dual-era claim (one unchanged PID, both
  requirement sets, no cross-era bleed) and now for the complete v2 MRTR surface. Do NOT cite it as
  "pmcp passes the official suite".
- **A follow-up SDK phase** — G-1, G-2, G-4, G-6, G-7 and G-8 are all wire-shape or error-code
  changes; G-3 and G-5 are transport wiring. G-1 is the only one needing a semver decision.

## Task 3 acceptance evidence

| Check | Result |
|---|---|
| `RUSTFLAGS="-D warnings" cargo build --example s54_v2_dual_conformance --features full` | exit 0 |
| `grep -c 'test_input_required_result_' examples/s54_v2_dual_conformance.rs` | `38` (≥ 9) |
| each of the nine suffixes (elicitation, sampling, list_roots, multi_round, multiple_inputs, request_state, tampered_state, capabilities, prompt) | all present |
| `grep -c 'test_tool_with_task' examples/s54_v2_dual_conformance.rs` (D-14) | `0` |
| `grep -cE 'aes\|chacha\|hmac\|Sha256\|ring::' examples/s54_v2_dual_conformance.rs` (T-118-21) | `0` |
| `grep -c '"resultType"' examples/s54_v2_dual_conformance.rs` | `0` |
| `git status --porcelain \| grep -c '^?? results/'` | `0` |
| `git status --porcelain` | only `examples/s54_v2_dual_conformance.rs` modified — no untracked results tree at any level, and `conformance/node_modules/` is covered by `.gitignore:46` (`**/node_modules/`) |
| run 1 exits 0 | **NO — exit 1.** Declared under D-21; see the seven scored scenarios above |
| run 2 exits 0 | **NO — exit 1.** Declared under D-21; see the seven scored scenarios above |

## `make quality-gate`

**Exit 0.** Run as `/usr/bin/make quality-gate` (absolute path — the rtk proxy truncates `make`
output, and a truncated log makes an exit status unattributable, per the Wave-1 finding). Verified
two ways rather than by exit code alone:

- `grep -c 'lines truncated'` over the captured log → `0`, so the log is complete;
- the literal banner is present at line 9822 of 9824:

```
═══════════════════════════════════════════════════════
        ✅ ALL TOYOTA WAY QUALITY CHECKS PASSED
        🎯 ALWAYS Requirements Validated
═══════════════════════════════════════════════════════
```

Includes `cargo fmt --all -- --check`, clippy with pedantic + nursery, the full test suite, the
doctests, the property tests, the example builds and the audit — all green with this example in the
tree.

## Self-Check: PASSED

Created / modified files verified present on disk:

- `examples/s54_v2_dual_conformance.rs` — FOUND
- `.planning/phases/118-conformance-against-the-official-suite/118-05-SUMMARY.md` — FOUND
- `target/conformance-results/2025-11-25/` — FOUND (33 scenario directories, 33 `checks.json`)
- `target/conformance-results/2026-07-28/` — FOUND (50 scenario directories, 50 `checks.json`)

Commits verified present in `git log`:

- `3ecf2440` — FOUND
- `812396a7` — FOUND
- `7c4babf9` — FOUND
