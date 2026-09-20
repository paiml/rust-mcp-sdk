---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 02
subsystem: protocol-types
tags: [mrtr, wire-adapter, elicitation, test-harness, v2, dos-bounds, aad-digest]
requires:
  - "113-01: the three v2 transport error codes + the Mcp-Name header rule record"
provides:
  - "src/types/mrtr.rs — the ONE place the 2026-07-28 MRTR wire spelling is assembled and parsed"
  - "MRTR_ELIGIBLE_METHODS + logical_name_key — one table each, read by client and server"
  - "encode/decode_header_value — the Mcp-Name base64 sentinel codec, empty-string safe"
  - "extract/splice_mrtr_params — fail-loud parse with five DoS bounds; stale-clearing splice"
  - "salient_param_digest — the spec's third replay binding for the requestState AAD"
  - "InputRequiredResult + MrtrOutcome — public carriers for an unfulfilled input_required result"
  - "MrtrSignal + MRTR_SIGNAL_META_KEY — the handler authoring surface"
  - "ElicitRequestParams accepts a mode-less form elicitation (serialization unchanged)"
  - "tests/common/v2.rs — the shared v2 HTTP harness for plans 04, 06, 08, 10, 11, 13"
affects:
  - "plan 03 (salient_param_digest is the AAD input; MAX_REQUEST_STATE_LEN bounds the token)"
  - "plan 04 (encode/decode_header_value + logical_name_key; two forward tripwires to flip)"
  - "plan 05 (client emits Mcp-Name via the same codec, empty for name-less methods)"
  - "plan 06 (extract_mrtr_params -> MrtrParseError -> JSON-RPC error mapping)"
  - "plan 07 (MrtrOutcome / InputRequiredResult are its return types)"
  - "plan 09 (MRTR_ELIGIBLE_METHODS tripwire + MRTR_SIGNAL_META_KEY stripping)"
  - "plan 12 (remove the module-level #![allow(dead_code)] once all consumers are wired)"
tech-stack:
  added: []
  patterns:
    - "Fail-loud parse: Result<T, E> where only a genuinely ABSENT key yields the default"
    - "Unconditional key removal before insert, so a later round cannot inherit an earlier one"
    - "Kind-directed decoding from the originating request, not untagged guessing"
    - "Whitelist-not-blacklist canonicalization for a security digest"
    - "Forward tripwire tests that PIN current wrong behaviour so a later plan must flip them"
key-files:
  created:
    - "src/types/mrtr.rs"
    - "tests/common/mod.rs"
    - "tests/common/v2.rs"
    - "tests/common_harness_smoke.rs"
    - ".planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md"
  modified:
    - "src/types/mod.rs"
    - "src/types/elicitation.rs"
decisions:
  - "InputResponse gets an eighth MrtrParseError variant (InputResponseUndecodable) rather than silently dropping an unrecognised entry"
  - "The empty-Mcp-Name tripwire runs on server/discover, not tools/list — tools/list cannot carry a v2 _meta signal today"
  - "MrtrRequestParams is deliberately NOT PartialEq (its payloads wrap non-PartialEq public v1 result types)"
metrics:
  duration: 42min
  tasks: 3
  files: 7
  completed: 2026-07-25
---

# Phase 113 Plan 02: MRTR Protocol Types, Elicitation Fix & Shared v2 Harness Summary

Built `src/types/mrtr.rs` — the single adapter every other Phase-113 plan trusts for the
2026-07-28 wire spelling — fixed the `mode`-optional elicitation deserialization that broke the
client half of CLNT-02, and lifted the Phase-112 HTTP harness into `tests/common/v2.rs` for six
downstream plans. All four cross-AI review findings on this plan are closed, and three
previously invisible pre-existing defects were surfaced and pinned by tests.

## What Was Built

### Task 1 — `src/types/mrtr.rs` (1536 lines; commits `078452a0` RED, `5b74ac73` GREEN)

The module owns exactly seven concerns, each with a single home:

| Concern | Items |
|---------|-------|
| Wire types | `InputRequest` (adjacently method-tagged, mirroring `ServerRequest`), `InputResponse`, `InputRequests`/`InputResponses` as `BTreeMap` aliases |
| Kind direction | `InputRequestKind` + `InputRequest::kind()` + `InputResponse::decode_for(kind, value)` |
| Method tables | `MRTR_ELIGIBLE_METHODS` / `mrtr_eligible`, `logical_name_key` / `logical_name_of` |
| Header codec | `encode_header_value` / `decode_header_value`, `=?base64?…?=` sentinel |
| Params adapter | `MrtrRequestParams`, `MrtrParseError`, `extract_mrtr_params`, `splice_mrtr_params`, five `MAX_*` bounds |
| AAD binding | `salient_param_digest` |
| Client / handler surface | `InputRequiredResult`, `MrtrOutcome<T>`, `MrtrSignal`, `MRTR_SIGNAL_META_KEY` |

**All four review findings closed.**

1. *Absent-vs-invalid conflation (HIGH).* `extract_mrtr_params` returns
   `Result<MrtrRequestParams, MrtrParseError>`. Only an ABSENT key — or a non-object `params` —
   yields the default. A present-but-wrong-shaped, oversized, over-deep or over-count value is an
   `Err`. A `null` `requestState` is INVALID, not absent.
2. *Stale-key leakage (HIGH).* `splice_mrtr_params` removes BOTH keys unconditionally before
   inserting anything, so `splice(params, &Default::default())` provably leaves neither key — a
   proptest asserts it over arbitrary starting params objects.
3. *Untagged response guessing (MEDIUM).* `decode_for` dispatches on the kind of the ORIGINATING
   `InputRequest`; the untagged path is a documented server-ingress-only fallback.
4. *No public carrier for an unfulfilled result (architectural).* `InputRequiredResult` +
   `MrtrOutcome<T>` exist, with `raw` holding the verbatim result object.

Visibility discipline holds: only `InputRequest`, `InputRequestKind`, `InputRequests`,
`InputResponse`, `InputResponses`, `InputRequiredResult`, `MrtrOutcome`, `MrtrSignal` (and
`MRTR_SIGNAL_META_KEY`) are `pub`. `grep -c 'pub fn extract_mrtr_params|pub fn splice_mrtr_params|pub fn salient_param_digest'` is **0**.

51 tests, including 6 proptests (no-panic over arbitrary JSON, splice/extract identity,
default-splice leaves no key, header codec round trip, decode never panics, digest never panics).

### Task 2 — `src/types/elicitation.rs` (commits `7213bf28` RED, `4f370558` GREEN)

Replaced the derived internally-tagged serde impls with hand-written ones. Deserialization reads
the OPTIONAL `mode`, treats absent/null as `"form"`, rejects a non-string `mode`, and dispatches to
serde-derived `FormShape`/`UrlShape` helpers so camelCase renaming and missing-required-field
errors stay serde-produced. Serialization is byte-identical to the old derived output — asserted
against the literal string `{"mode":"form","message":"hi","requestedSchema":{}}`.

`cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp`: **223 checks, 223 pass, no
semver update required.**

### Task 3 — `tests/common/v2.rs` (511 lines) + `tests/common_harness_smoke.rs` (commit `330466c3`)

The shared harness plans 04, 06, 08, 10, 11 and 13 consume: parameterized JSON-RPC id, mandatory
`clientCapabilities` with a `v2_body_with_caps` under-declaring escape hatch, always-emitted
`Mcp-Name` (empty for name-less methods), `post` / `post_raw` / `post_with_accept` / `get` /
`delete`, a `Resp` that captures `mcp_session_id` and `content_type` and transparently unwraps an
SSE `data:` frame, and both spawn helpers. `tests/v2_required_headers.rs` is byte-for-byte
untouched and still 25/25 green.

## Key Decisions

**An eighth `MrtrParseError` variant, not a silent drop.** The plan enumerated seven variants, but
`InputResponses` is a typed `BTreeMap<String, InputResponse>` — so an entry matching none of the
three spec-permitted result shapes has to go somewhere. Dropping it would reintroduce exactly the
absent-vs-invalid conflation this plan exists to kill, and the conformance suite has a check
(`sep-2322-validate-input-responses`) that demands the map be validated. Added
`InputResponseUndecodable { key }`.

**The empty-`Mcp-Name` tripwire runs on `server/discover`, not `tools/list`.** The plan specified
`tools/list` as the name-less method. It cannot be a v2 request today (finding D-113-B below), so
asserting 200 there would have been a test that could not pass. `server/discover` is the only
name-less method that IS v2-capable today, so the header rule is proved end-to-end there, and
`tools/list` gets a forward tripwire asserting the current 400 instead.

**Forward tripwires instead of comments.** Three defects this plan surfaced belong to plan 04.
Rather than writing them down and hoping, each is pinned by a test that asserts the CURRENT wrong
behaviour with a message naming what plan 04 must flip it to. A silent fix is impossible; a silent
non-fix is visible.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 — Missing critical functionality] `MrtrParseError::InputResponseUndecodable` added**

- **Found during:** Task 1, wiring `extract_input_responses` into the typed `InputResponses` map
- **Issue:** the plan's seven error variants had no home for an entry matching none of
  `ElicitResult` / `CreateMessageResult` / `ListRootsResult`; the alternative was dropping it
- **Fix:** added an eighth variant carrying the offending key (not echoed by `Display`)
- **Files modified:** `src/types/mrtr.rs`
- **Commit:** `5b74ac73`

**2. [Rule 3 — Blocking] `Resp` must unwrap SSE framing**

- **Found during:** Task 3, first run against `spawn_default_config`
- **Issue:** `StreamableHttpServerConfig::default()` sets `enable_json_response: false`, so a POST
  reply arrives as `event: message\ndata: {…}` and `serde_json::from_str` on the raw text yields
  `Null` — every `body["result"]` assertion in six downstream plans would silently fail
- **Fix:** `parse_body` falls back to the first `data:` frame, so both spawn helpers assert alike
- **Files modified:** `tests/common/v2.rs`
- **Commit:** `330466c3`

**3. [Rule 3 — Blocking] the harness must emit `_meta` under two spellings**

- **Found during:** Task 3 (see finding D-113-A)
- **Fix:** `v2_body_with_caps` inserts the reserved object under both `_meta` and the probed
  `request_meta_key()`; a tripwire test pins the probe result so the workaround cannot outlive the
  defect
- **Commit:** `330466c3`

**4. [Rule 3 — Blocking] four clippy pedantic/nursery findings on the new module**

- 6× `doc_markdown` on `DoS`, 1× `implicit_clone`, 1× `needless_pass_by_value` on
  `try_from_value_untagged` (restructured so the last decode attempt CONSUMES the value), and
  1× `large_enum_variant` on `MrtrOutcome<T>` — the last resolved with a `// Why:`-annotated
  `#[allow]` because clippy sizes a generic `T` at 0 bytes and reports a difference that does not
  exist for any real instantiation. Two more `needless_pass_by_value` in the harness were fixed
  properly by building the JSON-RPC envelope through a `serde_json::Map` (`jsonrpc_envelope`)
  instead of the borrowing `json!` macro.

### Plan Assumptions That Did Not Hold

**5. `spawn_default_config` / `spawn_stateless_config` are `pub async fn`, not `pub fn`**

The acceptance criterion greps for the literal `pub fn spawn_default_config`.
`StreamableHttpServer::start()` is `async` — and awaiting it before returning is precisely what
gives callers the readiness guarantee the harness doctrine requires — so a synchronous signature is
not expressible. Both are `pub async fn`. The two literal greps therefore return 0; every other
Task-3 grep criterion passes.

**6. `MrtrRequestParams` is not `PartialEq`**

The plan's tests were written as `assert_eq!(extract(..), MrtrRequestParams::default())`. Deriving
`PartialEq` would require `PartialEq` on `ElicitResult`, `CreateMessageResult` and
`ListRootsResult` — widening three public v1 types for a test comparison. Instead the two tests
assert structurally via a local `assert_is_default` helper, and the type documents why.

**7. Module-level `#![allow(dead_code)]` on `src/types/mrtr.rs`**

`RUSTFLAGS = -D warnings` is the real gate, and this module lands in Wave 1 ahead of all its
production consumers (plans 03/04/06/07/09), so every `pub(crate)` item is dead code today. The
allow carries a `// Why:` naming each consuming plan and instructing plan 12 to remove it.

## Findings Surfaced (not defects in this plan)

All three are recorded in
`.planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md`, each pinned
by a test.

**D-113-A (HIGH) — typed request structs rename `_meta` to `meta` on the wire.**
`CallToolRequest` / `GetPromptRequest` / `ReadResourceRequest` all carry
`#[serde(rename_all = "camelCase")]`, which renames the `_meta` FIELD. Probed directly:
serialization emits `"meta"`, and deserializing a spec-spelled `"_meta"` yields `_meta == None`
(silently ignored) while `"meta"` yields `Some`. Because Phase 112 routes the entire per-request
era signal through `extract_request_meta_value(request)` — which reads the typed `req._meta` — a
**conformant v2 client sending `_meta` gets no era detection at all**, and its
`MCP-Protocol-Version: 2026-07-28` header is then rejected as a header/`_meta` disagreement. The
`server/discover` ingress reads a RAW `params._meta` and uses the CORRECT spelling, so the two
ingress paths currently disagree with each other. Not auto-fixed: changing the wire spelling of a
field on three public v1 request types changes v1 bytes for every existing client — a Rule-4
architectural decision. Pinned by
`forward_tripwire_typed_requests_rename_meta_away_from_the_spec_spelling`.

**D-113-B (HIGH for HTTP-01) — `tools/list` cannot be a v2 request.**
`extract_request_meta_value` enumerates exactly three `_meta`-bearing client requests;
`ListToolsRequest` has no `_meta` field at all. A stateless v2 server has no handshake, so the
per-request `_meta` signal is the ONLY era channel — which means `tools/list`, `prompts/list`,
`resources/list`, `completion/complete` and `subscriptions/listen` are all currently
un-v2-able. Pinned by `forward_tripwire_tools_list_cannot_be_a_v2_request`.

**D-113-C (expected) — the stateful `::default()` config still demands a session on v2.**
This IS requirement HTTP-01 and is plan 04's deliverable. Pinned by
`forward_tripwire_stateful_config_still_demands_a_session_on_v2`.

## Verification

| Check | Result |
|-------|--------|
| `cargo test --lib --features full -- types::mrtr` | **51 passed** (6 proptests) |
| `cargo test --lib --features full -- types::elicitation` | **12 passed** |
| `cargo test --lib --features full` (whole lib) | **1292 passed, 0 failed** |
| `cargo test --test common_harness_smoke --features full` | **7 passed** |
| `cargo test --test v2_required_headers --features full` | **25 passed** (Phase-112 baseline) |
| `git diff --stat tests/v2_required_headers.rs` | **empty — untouched** |
| `cargo build --lib --target wasm32-unknown-unknown` | exit 0; **0 warnings from `mrtr.rs`** |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223/223 pass, no semver update required** |
| `make quality-gate` | **ALL TOYOTA WAY QUALITY CHECKS PASSED** (exit 0, 0 test failures) |

### Task-1 acceptance greps

| Criterion | Result |
|-----------|--------|
| `MRTR_ELIGIBLE_METHODS`, `salient_param_digest`, `splice_mrtr_params`, `extract_mrtr_params`, `logical_name_key` present | all present |
| `MRTR_SIGNAL_META_KEY` + all five `MAX_*` bounds present | all present |
| `pub enum MrtrOutcome`, `pub struct InputRequiredResult`, `pub enum InputRequestKind`, `fn decode_for` | all present |
| `enum MrtrParseError`, `extract_mrtr_params` returns `Result<` | present |
| `encode_header_value`, `decode_header_value`, literal `=?base64?` | present |
| `grep -c 'pub fn extract_mrtr_params\|pub fn splice_mrtr_params\|pub fn salient_param_digest'` | **0** |
| `src/types/mod.rs` contains `pub mod mrtr;` | 1 |
| `grep -c 'input_responses\|request_state'` in `prompts.rs` / `resources.rs` | **0 / 0** |

### Task-2 acceptance greps

| Criterion | Result |
|-----------|--------|
| `impl<'de> Deserialize<'de> for ElicitRequestParams` | present |
| `grep -c '#\[serde(tag = "mode")\]'` | **0** |
| mode-less form test / `to_value(Form)["mode"] == "form"` test | both present and passing |

### Task-3 acceptance greps

| Criterion | Result |
|-----------|--------|
| all three files exist | yes |
| `pub async fn post_raw`, `pub fn v2_body_with_caps`, `pub fn v2_headers_raw` | present |
| `pub fn spawn_default_config` / `pub fn spawn_stateless_config` | **`pub async fn`** — see deviation 5 |
| literal `io.modelcontextprotocol/clientCapabilities` | present |
| `Resp` contains `mcp_session_id` + `content_type` | present |
| empty-`Mcp-Name` 200 test | present (on `server/discover`) |

## TDD Gate Compliance

Both `tdd="true"` tasks completed a real RED→GREEN cycle with the failure verified, not assumed.

| Task | RED commit | RED evidence | GREEN commit |
|------|-----------|--------------|--------------|
| 1 | `078452a0` `test(113-02)` | six behaviour functions stubbed `unimplemented!()`; **37 failed / 14 passed** — proving the suite is non-vacuous, and that the 14 table/wire-shape tests genuinely exercise data declarations rather than behaviour | `5b74ac73` `feat(113-02)` |
| 2 | `7213bf28` `test(113-02)` | **1 failed / 11 passed**, failing with the exact defect: `Error("missing field `mode`")` | `4f370558` `feat(113-02)` |

No REFACTOR commit was needed for either task.

## Threat Model Coverage

| Threat ID | Disposition | How this plan discharged it |
|-----------|-------------|------------------------------|
| T-113-03 | mitigate | `salient_param_digest` uses a per-method WHITELIST (`name`+`arguments`, or `uri`) with recursive key-sorted canonicalization (depth-capped) before SHA-256, and binds the method name into the hash. Tests prove key-insertion-order stability, sensitivity to `name`/`uri`/`arguments`, method binding, and INSENSITIVITY to `_meta`/`inputResponses`/`requestState` |
| T-113-14 | mitigate | Five bounds, five distinct `MrtrParseError` variants, each with a doc naming the DoS surface it closes; `json_depth` is ITERATIVE so an adversarially nested value cannot exhaust the stack before the bound reports; a proptest asserts no panic over arbitrary JSON |
| T-113-15 | mitigate | `splice_mrtr_params` / `extract_mrtr_params` are the only two places the key spelling exists; a test asserts the fields land as top-level siblings and NOT inside `_meta` or `arguments` |
| T-113-16 | mitigate | `MRTR_ELIGIBLE_METHODS` is one const with a derived predicate; a test pins the exact three-in / five-out set. `logical_name_key` moved here so client and server read one table |
| T-113-44 | mitigate | `extract_mrtr_params` returns `Result`; only a genuinely absent key yields the default; `null` is INVALID, not absent |
| T-113-45 | mitigate | Unconditional removal before insert, with a proptest asserting a default splice leaves neither key over arbitrary starting params |
| T-113-46 | mitigate | `decode_for(kind, ..)` is kind-directed; tests prove a `CreateMessageResult`-shaped value is REJECTED for an elicitation entry and vice versa; the untagged path is documented server-ingress-only |
| T-113-47 | mitigate | `encode_header_value` excludes `"`, `,`, `;`, `\` from the pass-through set and sentinel-encodes anything else — including a value that itself starts with the sentinel marker. Round-trip tests cover ASCII, non-ASCII, delimiters and the EMPTY string; malformed sentinels decode to `None` without panicking |

## Known Stubs

None. No `TODO`/`FIXME`/`unimplemented!()` remains in either committed implementation (the
`unimplemented!()` stubs existed only inside the transient RED commit `078452a0` and were replaced
wholesale by `5b74ac73`). The `make quality-gate` zero-SATD check passes.

## Threat Flags

None. This plan added protocol TYPES and a test harness; it introduced no network endpoint, no
auth path, no file access pattern, and no schema change at a trust boundary. The three defects it
surfaced are pre-existing and recorded in `deferred-items.md`.

## Follow-ups for Later Plans

1. **Plan 03** — `salient_param_digest` is the AAD input; `MAX_REQUEST_STATE_LEN = 8192` is the
   accepted-token bound the minted token must fit inside.
2. **Plan 04** — consume `encode_header_value` / `decode_header_value` / `logical_name_key` from
   `src/types/mrtr.rs` rather than the local copies in `streamable_http_server.rs`, and flip the
   three forward tripwires in `tests/common_harness_smoke.rs`. **D-113-A and D-113-B are blocking
   for a genuinely stateless v2** — a conformant client cannot currently be detected as v2 at all.
3. **Plan 05** — the client's `Mcp-Name` emission must use the same codec and emit `""` for
   name-less methods.
4. **Plan 06** — map every `MrtrParseError` (all EIGHT variants) to a JSON-RPC error before
   dispatch.
5. **Plan 07** — `MrtrOutcome<T>` / `InputRequiredResult` are the return types; `raw` preserves
   anything the typed fields do not model.
6. **Plan 09** — `MRTR_ELIGIBLE_METHODS` is the tripwire source; `MRTR_SIGNAL_META_KEY` must be
   stripped on EVERY path including v1.
7. **Plan 11** — D-113-A means the conformance suite (which sends spec-spelled `_meta`) will fail
   every v2 scenario until it is resolved; sequence that decision before the conformance run.
8. **Plan 12** — remove the module-level `#![allow(dead_code)]` from `src/types/mrtr.rs` once every
   consumer is wired, and audit the eight `pub` MRTR items in the `cargo public-api` delta.

## Self-Check: PASSED

- `src/types/mrtr.rs` — FOUND (1536 lines)
- `src/types/mod.rs` — FOUND (contains `pub mod mrtr;` + the narrow re-export)
- `src/types/elicitation.rs` — FOUND (contains the hand-written `Deserialize`)
- `tests/common/mod.rs` — FOUND
- `tests/common/v2.rs` — FOUND (511 lines)
- `tests/common_harness_smoke.rs` — FOUND (236 lines)
- `.planning/phases/113-.../deferred-items.md` — FOUND
- Commit `078452a0` — FOUND
- Commit `5b74ac73` — FOUND
- Commit `7213bf28` — FOUND
- Commit `4f370558` — FOUND
- Commit `330466c3` — FOUND
