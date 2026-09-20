---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 04
subsystem: streamable-http-transport
tags: [http-01, stateless, era-gate, session, status-mapping, mcp-name, meta-spelling, v2]
requires:
  - "113-01: the three v2 transport error codes (HEADER_MISMATCH, MISSING_REQUIRED_CLIENT_CAPABILITY, UNSUPPORTED_PROTOCOL_VERSION) + the Mcp-Name rule record"
  - "113-02: src/types/mrtr.rs (logical_name_key, decode/encode_header_value) + tests/common/v2.rs harness"
  - "112-06/112-10: the v2 header gate, ProtocolContext resolution seam and HttpIngress classification"
provides:
  - "sessions_active(state, era) — the ONE predicate every session decision routes through"
  - "active_session_generator + apply_session_header — the only readers/emitters of session state"
  - "v2_method_not_allowed — 405 on v2 GET/DELETE, before any session work"
  - "v2_status_for_code / status_for_error / v2_dispatch_response_status — the era-gated, code-driven HTTP status table"
  - "map_unparsed_body_for_v2 — raw-level 404+-32601 with the original id for a body that never typed-parses"
  - "V2GateOutcome::Reject { code, message, data } — structured rejection payloads (-32022 carries data.supported)"
  - "the spec `_meta` wire spelling on the three _meta-bearing request types (D-113-A)"
  - "raw_params_meta + Server::resolve_raw_meta_protocol_context — ONE era-detection path on the HTTP transport, covering every method with zero public API change (D-113-B)"
  - "tests/v2_stateless_http.rs — 15 live-HTTP assertions against a STATEFUL-config server"
affects:
  - "plan 05 (client): Mcp-Name emission must use the same mrtr codec; empty for name-less methods"
  - "plan 06 (MRTR params): owns the precise -32602 mapping for a KNOWN method with malformed params"
  - "plan 08 (id preservation): v2_string_id_preserved + raw_request_id are the foundations"
  - "plan 09 (dispatch): -32021 reaches HTTP 400 automatically via the code-driven mapper"
  - "plan 10 (subscriptions/listen): v2-capable for free over HTTP — the raw reader needs no typed _meta field; only the non-HTTP transports would need an extract_request_meta_value arm"
  - "plan 11 (conformance): a conformant client sending spec-spelled _meta is now detected as v2, on every method"
  - "plan 12 (semver gate): CLEAR — 223/223 pass, no semver update required"
tech-stack:
  added: []
  patterns:
    - "One era predicate over a server-wide config, not a transport fork"
    - "Code-driven status mapping (read the code about to reach the wire, not the call site)"
    - "Raw-level diagnosis for bodies that never produce a typed request"
    - "serde `rename` + `alias`: conformant on egress, backward-compatible on ingress"
    - "Forward tripwires INVERTED into permanent regression guards rather than deleted"
    - "Read the untrusted wire, not a typed field, when a typed field would cost public API surface"
key-files:
  created:
    - "tests/v2_stateless_http.rs"
  modified:
    - "src/server/streamable_http_server.rs"
    - "src/server/mod.rs"
    - "src/server/core.rs"
    - "src/types/tools.rs"
    - "src/types/prompts.rs"
    - "src/types/resources.rs"
    - "src/types/protocol/mod.rs"
    - "tests/common/v2.rs"
    - "tests/common_harness_smoke.rs"
    - "tests/v2_required_headers.rs"
decisions:
  - "The v2 header gate MOVED above session resolution in both POST entrypoints — the era must be known before the first session decision"
  - "GET/DELETE evaluate sessions at era=None, which is exactly the pre-113 config-only read, because the 405 guard already removed v2 traffic"
  - "D-113-B resolved by reading params._meta off the RAW body at HTTP ingress, NOT by adding _meta fields — the field route measured as a MAJOR semver break (D-113-D) and the raw route needs zero public API change"
  - "The two HTTP gates (typed + raw) collapsed into ONE, closing the plan-02 finding that the two ingress paths disagreed about which methods carry an era signal"
  - "v1_unknown_method_still_200 asserts pmcp's TWO distinct v1 unknown-method paths (-32601@200 for server/discover, PARSE_ERROR@400 for an unparseable method string) rather than the plan's single assumed one"
  - "The Phase-112 baseline's expected error-code VALUES move with the codes this plan changes (-32600 -> -32020, -32602 -> -32022); statuses and v1 bytes are untouched"
metrics:
  duration: 165min
  tasks: 5
  files: 10
  completed: 2026-07-25
---

# Phase 113 Plan 04: Stateless v2 HTTP (HTTP-01) Summary

Made the ERA, not the build-time config, decide whether a streamable-HTTP request has a
session — through one `sessions_active` predicate rather than a transport fork — and added the
v2 status mappings (405 / 404 / 400 + structured `data`) the spec requires. Along the way it
closed all three findings plan 02 surfaced, including the one that meant **no conformant v2
client could ever be detected as v2 at all**.

## What Was Built

### Task 1 — the `_meta` wire contract (D-113-A + D-113-B) · `73a24cf1` RED, `47eaad68` GREEN

These were scope expansions authorized by the owner after plan 02 surfaced them, and they are
prerequisites for everything else: without them a v2 `tools/list` cannot exist and a
spec-conformant client is invisible.

**D-113-A.** `CallToolRequest` / `GetPromptRequest` / `ReadResourceRequest` carry a struct-level
`#[serde(rename_all = "camelCase")]`, which renames the `_meta` **field** to `meta`. pmcp
therefore emitted a spelling the MCP spec does not define, and silently DROPPED a spec-spelled
`_meta` on ingress — after which Phase 112's fail-closed matrix rejected the request with
"header claims v2 but `_meta` protocolVersion disagrees". Each field is now pinned with
`#[serde(rename = "_meta", alias = "meta", skip_serializing_if = "Option::is_none", default)]`:
conformant on egress, backward-compatible with pre-113 pmcp peers on ingress. The repo already
used this exact idiom two lines apart in the same file (`src/types/tools.rs:219`, `:556`) — the
three request types were simply missing it.

**D-113-B — first attempt, then reverted.** A stateless v2 server runs no `initialize`
handshake, so the per-request `_meta` object is the ONLY era channel — yet `ListToolsRequest` and
its siblings had no `_meta` field. The first attempt added an optional field to all five list-
shaped types. It worked, but measurement showed it forced a MAJOR semver bump, so the owner chose
option 3 and it was reverted. See **Task 5** for what actually shipped.

An absent `_meta` still emits no key at all, so **v1 wire bytes are unchanged** — pinned by
`absent_meta_emits_no_key_on_any_request_type`.

### Task 2 — `sessions_active`, the one session predicate · `2d16e23b` RED, `2baf265f` GREEN

`stateless()` is a BUILD-TIME config. A dual-version server is built with `Default::default()`,
which keeps a live `session_id_generator` — so every session decision keyed off the CONFIG and
v2 requests got session ids minted, demanded and echoed (RESEARCH Pitfall 1).

```rust
const fn sessions_active_for(cfg_has_generator: bool, era: Option<Era>) -> bool {
    !matches!(era, Some(Era::V2)) && cfg_has_generator
}
```

Every session decision now routes through it. `sessions_active` and `active_session_generator`
are the **only** readers of `config.session_id_generator`; `apply_session_header` is the **only**
emitter of `Mcp-Session-Id` (and it replaced three `.parse().unwrap()` panics with a
non-panicking insert). The two bodyless verbs evaluate at `era = None`, which is exactly the
pre-113 config-only read — justified in code by the fact that the 405 guard already removed all
v2 traffic before they run.

The structural change that makes this possible: **the v2 header gate moved above session
resolution** in both POST entrypoints. The era has to be known before the first session decision.
For v1 and non-opted-in servers the gate is a pure passthrough, so nothing observable changes.

### Task 3 — v2 status mappings, structured rejects, the locked `Mcp-Name` rule · `7ab9b205`

| Part | What landed |
|------|-------------|
| (a) | The `Mcp-Name` rule stated verbatim on `require_three_headers`, with tests in BOTH directions: `tools/list` + `Mcp-Name: ""` is `EnforceOk`; `tools/list` with the header ABSENT is a rejection. DRIFT-1 stays locked — pmcp is deliberately stricter than the draft spec. |
| (b) | `v2_method_not_allowed` answers 405 on a v2 GET/DELETE **before** header and session validation, so a v2 GET never touches session state or the event store (T-113-18). Routes stay registered; pmcp is dual-version. |
| (c) | `map_unparsed_body_for_v2` diagnoses an unknown method at the RAW level — resolving the era from `params._meta` and recovering the id from the body bytes — because an unknown method string never produces a typed `ClientRequest` for a typed-response mapper to inspect. `v2_dispatch_response_status` then re-maps handler-produced errors from the CODE about to reach the wire, which is the only way plan 09's `-32021` can ever be mapped. |
| (d) | `V2GateOutcome::Reject { code, message, data }` — `-32022` now carries `error.data.supported` (the server accept-list) so a client can retry with a mutually supported version instead of probing (T-113-51). |
| (e) | `Mcp-Name` is decoded through the SHARED `mrtr` sentinel codec before the body cross-check, so a conformant `=?base64?…?=` non-ASCII name is accepted and a malformed sentinel is a `HEADER_MISMATCH`. The module-local `logical_name_key` is deleted — client and server read one table. |

`grep -vE '^\s*(//|///|//!)' … | grep -cE '\-32[0-9]{3}'` is **0**.

### Task 4 — `tests/v2_stateless_http.rs` (15 tests) · `860cb269`

Every server spawned with `StreamableHttpServerConfig::default()`. The file header records why
in full: a build-time-stateless spawn would make every assertion vacuous, because the session
machinery would already be gone and the per-request era gate would never be under test.
`grep -c` for the other spawn helper returns **0**, including in prose.

### Task 5 — D-113-B, redone as a raw-body read · `b2cc87fe` REVERT/RED, `f6735c03` GREEN

The owner chose **option 3** of D-113-D after seeing the measurement: revert the fields, resolve
D-113-B from the raw body, keep the milestone at 2.x.

`b2cc87fe` is a **pure revert** — the five fields, the five `extract_request_meta_value` arms and
every mechanical `_meta: None` initializer restored **byte-identically** to the pre-plan baseline
(verified with `git diff 73a24cf1~1`, which came back empty for all ten affected files). It left
`tools_list_is_a_valid_v2_request` and `v2_nameless_method_empty_mcp_name_accepted` RED with the
original `"header claims v2 but _meta protocolVersion disagrees"` rejection — the same failure
those tripwires pinned before this plan started, which is what proves the raw route (and not some
side effect) is what turns them green again.

`f6735c03` wires the raw route:

| Change | Why |
|--------|-----|
| `Server::resolve_discover_protocol_context` → `resolve_raw_meta_protocol_context` | Same behavior, no longer discover-specific; it is now the era resolver for EVERY method on the HTTP path |
| `raw_params_meta(body)` | Reads `params._meta` off the request bytes. Prefers the spec spelling, falls back to `meta`, so it mirrors the `#[serde(rename = "_meta", alias = "meta")]` ingress contract D-113-A put on the typed structs and the two readers cannot disagree about what a `_meta` object IS |
| `run_v2_header_gate` absorbed `run_v2_header_gate_raw` + `finish_v2_gate` | **One** gate. `server/discover` is now just the one caller that passes a `body_method_override`, so its method stays pinned by classification and a body whose `method` field disagrees cannot fool the cross-check |
| `HttpIngress::Discover` dropped its `raw_meta` copy | It was a second, independent read of the same bytes — a drift surface |
| `SERVER_DISCOVER_METHOD` | Single-sources the method string the classifier and the transport both spell |

**This closes the "two ingress paths disagree" defect** that plan 02 flagged alongside D-113-A.
Before, the HTTP transport had a typed gate (which could only see the three structs with a
`_meta` FIELD) and a raw gate (discover only), so the two covered different method sets and a
`tools/list` fell through the gap. **There is now exactly one era-detection path in the HTTP
transport, reading the spec-spelled `_meta` from the raw body, and it covers every method** —
including ones that do not exist yet, which is why plan 10's `subscriptions/listen` is v2-capable
for free.

`Server::handle_request` — the non-HTTP dispatch entry used by stdio/WebSocket — still uses the
TYPED `extract_request_meta_value`, because it has no raw bytes at that seam. The two readers
agree on **spelling** (D-113-A pinned it) and differ only in **method coverage**; the HTTP path,
which is the one v2 targets, has full coverage. `extract_request_meta_value`'s rustdoc now says
all of this so a future reader does not mistake its three-method scope for a defect and "fix" it
back into a breaking change — and
`typed_extractor_scope_is_the_three_meta_bearing_methods` pins the boundary.

**Accepted cost, recorded so plans 06/09/10 do not re-litigate it:** a handler cannot read
`_meta` off a typed list-request struct, because those structs have no such field. The supported
route is the `ProtocolContext`-derived `RequestHandlerExtra` accessors (`era()`, `client_info()`,
`trace_context()`) that Phase 112 wired — the HTTP layer resolves the context from the raw body
and threads that SAME value into dispatch, so nothing about the handler-visible surface changed.

## Key Decisions

**The gate moved above session resolution, rather than resolving the era twice.** The
alternative — a second, cheaper era read just for the session decision — is exactly research
Pitfall 2 (dual negotiation). One resolution, threaded.

**Read the wire, not a typed field, when a typed field costs public API surface.** The
measurement (D-113-D) is what changed the design: five `Option` fields that could never alter a
single wire byte still failed `constructible_struct_adds_field`, because those `pub` structs are
constructible with all-`pub` fields. The raw-body read gets the same coverage for free, and it
generalizes to methods that do not exist yet. It also happens to be the more honest thing to
compare against: the header/body cross-check already reads the LITERAL wire bytes a WAF would
see, and now the era does too.

**`v1_unknown_method_still_200` asserts pmcp's two ACTUAL v1 paths.** The plan assumed one
("v1 unknown method → 200 + -32601"). Measurement shows pmcp has two: `server/discover` is
classified at ingress and reaches dispatch, answering `-32601` at HTTP **200**; an arbitrary
unknown method string fails the transport parse and answers `PARSE_ERROR` at HTTP **400** with
`id: null`. The test keeps the planned name for traceability and pins both, with the load-bearing
assertion being that **neither is 404**.

**The Phase-112 baseline's expected VALUES move with the codes.** Plan 04 is the plan that
routes the header-gate cells onto the spec-allocated codes, so `tests/v2_required_headers.rs`
now expects `HEADER_MISMATCH` where it expected `-32600` and `UNSUPPORTED_PROTOCOL_VERSION` where
it expected `-32602`. Every **status** in that file is unchanged, and the unsupported-version test
gained an assertion that `data.supported` is an array. All 25 pass.

## Deviations from Plan

### Authorized scope expansion (owner decision, recorded in the execution prompt)

D-113-A and D-113-B were implemented as Task 1 of this plan, adding
`src/types/{tools,prompts,resources,protocol/mod}.rs`, `src/server/core.rs` and several test
files to `files_modified`. Both tripwires were retired as instructed — inverted into permanent
regression guards, not deleted — and the dual-spelling workaround in `tests/common/v2.rs` was
removed.

### Mid-execution course correction (owner decision after the D-113-D measurement)

D-113-B's implementation was replaced. The owner reviewed the measured semver break and chose
option 3, so Task 5 reverted the five `_meta` fields and re-resolved D-113-B by reading
`params._meta` off the raw body at HTTP ingress. D-113-A was kept as-is. See Task 5 above; the
proof that the break is gone is in the Verification table.

### Auto-fixed Issues

**1. [Rule 3 — Blocking] Two clippy pedantic failures after the era threading**

- **Found during:** Task 2, `make lint`
- **Issue:** `needless_lifetimes` on `active_session_generator`; `too_many_arguments` (8/7) on
  `assemble_discover_response_with_middleware`
- **Fix:** elided the lifetimes; introduced `DiscoverResponseShape<'_>` bundling
  `(response_session_id, v2_outbound, sessions_on)` — which also guarantees the fast and
  middleware discover paths can never drift on session-header gating
- **Commit:** `2baf265f`

**2. [Rule 3 — Blocking] Two more clippy failures after the status mapping**

- **Found during:** Task 3, `make lint`
- **Issue:** `needless_pass_by_value` on `create_error_response_with_id(id: Value)` (the `json!`
  macro borrows, so `id` was never consumed); `large_futures` (16608 bytes) on the fast-path
  dispatch call once the status mapping was threaded through it
- **Fix:** built the envelope through a `serde_json::Map` so `id` is consumed; `Box::pin`ned the
  dispatch future, matching the treatment the two POST entrypoints already had
- **Commit:** `7ab9b205`

**3. [Rule 3 — Blocking] A `_meta`-addition script mangled a doctest**

- **Found during:** Task 1, `cargo test --doc`
- **Issue:** the mechanical initializer update collapsed a `CompleteRequest { … }` inside a
  rustdoc example onto one line, producing "unexpected closing delimiter"
- **Fix:** rewrote the example by hand; a repo-wide grep confirmed no other doc block was touched
- **Commit:** `47eaad68`

**4. [Rule 3 — Blocking] `v1_unknown_method_still_200` hit the v1 session gate**

- **Found during:** Task 4, first run
- **Issue:** on a stateful-config server, a v1 non-init request is rejected for a missing session
  id BEFORE the method is ever routed, so the test never reached the behavior it was asserting
- **Fix:** the test now mints a session via a real v1 `initialize` first — which is the correct v1
  flow anyway, and makes the assertion genuinely about method routing
- **Commit:** `860cb269`

### Plan Assumptions That Did Not Hold

**5. There is no single "v1 unknown method → 200" behavior.** See Key Decisions.

**6. `cargo test --test v2_required_headers` could NOT stay byte-for-byte unchanged.** The plan
asked both for the gate to emit `-32020`/`-32022` and for the Phase-112 suite to pass untouched.
Those are mutually exclusive: that suite pins the exact code values the plan changes. The
assertions were updated (values only — every status and every v1 byte is unchanged), which is the
reading consistent with the plan's own `<behavior>` block and its live `-32020` test.

**7. GET must not be driven to EOF in a test.** A v1 GET on a stateful config opens a real SSE
stream, and the harness reads the body to completion — so a naive `v1_get_delete_unchanged` would
hang forever. The test instead drives both verbs with an unknown `Mcp-Session-Id`, which produces
an immediate 404 on v1 and 405 on v2. That is a stronger assertion anyway: it proves the 405 guard
runs BEFORE session validation.

## Findings Surfaced

### D-113-D (HIGH) — D-113-B's field additions required a MAJOR semver bump · RESOLVED

Measured immediately after the first D-113-B implementation landed:

```
cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp
  223 checks: 222 pass, 1 fail, 0 warn, 30 skip
  --- failure constructible_struct_adds_field ---
    ListToolsRequest._meta, ListPromptsRequest._meta, ListResourcesRequest._meta,
    ListResourceTemplatesRequest._meta, CompleteRequest._meta
  Summary: semver requires new major version
```

The D-113-A half was clean (serde attributes only; `CallToolRequest` is already
`#[non_exhaustive]`). Only the five D-113-B structs were flagged: they are `pub` with all-`pub`
fields and not `#[non_exhaustive]`, so a downstream `ListToolsRequest { cursor: None }` stops
compiling. **The WIRE was never affected** — every added field was `Option` + `default` +
`skip_serializing_if`, and the "absent `_meta` emits no key" test pinned that. The break was
purely Rust source compatibility.

There is no way to add a field to a constructible struct without it: `#[non_exhaustive]` and a
private field are both flagged major by the same tool.

**Resolution — the owner chose option 3: revert the fields, read the raw body, keep 2.x.**
Shipped as Task 5 (`b2cc87fe` revert, `f6735c03` raw gate). Proof:

```
$ cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp
     Checked [   0.148s] 223 checks: 223 pass, 30 skip
     Summary no semver update required
```

The measurement is what changed the design — the field route looked additive and was not.

### Cross-version note for the release (D-113-A)

pmcp now emits `_meta` on egress. A pmcp **server** older than this change accepts only `meta`,
so a new pmcp client talking to a ≤2.17 pmcp server loses the per-request `_meta` payload
(progress token, task id, namespaced keys). The `alias` fixes the ingress direction only. That
older server was already non-conformant with the spec spelling.

### Known limitation handed to plan 06

A KNOWN method whose params fail to deserialize reaches the same `method_not_found` seam as a
genuinely unknown method, so on v2 it is reported as `-32601`/404 rather than `-32602`/400.
Distinguishing them needs a method-string table this layer does not own. Documented in code on
`map_unparsed_body_for_v2`; plan 06 owns the precise per-parameter mapping.

## Verification

All figures below are from the FINAL state (after the Task-5 revert + raw gate).

| Check | Result |
|-------|--------|
| `cargo test --test v2_stateless_http --features full` | **15 passed** |
| `cargo test --test v2_required_headers --features full` | **25 passed** (Phase-112 baseline) |
| `cargo test --test common_harness_smoke --features full` | **7 passed** (all three tripwires flipped) |
| `cargo test --test server_subscriptions --features full` | **6 passed** (v1 subscribe path untouched) |
| `cargo test --lib --features full -- streamable_http_server` | **37 passed** (was 25 pre-plan) |
| `cargo test --features full` (whole workspace suite) | **0 failures** |
| `cargo test --doc --features full` | **382 passed** |
| `make lint` (unproxied, absolute path) | **exit 0** |
| **`cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp`** | **`223 checks: 223 pass, 30 skip` / `Summary no semver update required`** — the D-113-D MAJOR break is gone |
| **`make quality-gate`** (unproxied `/usr/bin/make`) | **ALL TOYOTA WAY QUALITY CHECKS PASSED, exit 0** |
| `git status --porcelain -- src/ tests/` after the gate | **empty** — the gate ran against COMMITTED source |
| `git diff 73a24cf1~1` over the ten pure-revert files | **empty** — the reverted files are byte-identical to the pre-plan baseline |

### Acceptance greps

| Criterion | Result |
|-----------|--------|
| `streamable_http_server.rs` contains `fn sessions_active` | present (+ `sessions_active_for`, `active_session_generator`) |
| no bare `config.session_id_generator` read in the four session sites | 0 — only `sessions_active` and `active_session_generator` read it |
| `StatusCode::METHOD_NOT_ALLOWED` / `StatusCode::NOT_FOUND` in the v2 paths | present |
| `error_codes::HEADER_MISMATCH` + `error_codes::UNSUPPORTED_PROTOCOL_VERSION` | present |
| `Reject {` with a `data` field + the literal `"supported"` | present |
| `decode_header_value` | present |
| `grep -c 'fn logical_name_key' streamable_http_server.rs` | **0** |
| `grep -vE '^\s*(//\|///\|//!)' … \| grep -cE '\-32[0-9]{3}'` | **0** |
| `tests/v2_stateless_http.rs` contains `spawn_default_config` | 17 occurrences |
| `grep -c 'spawn_stateless_config' tests/v2_stateless_http.rs` | **0** |
| all fifteen planned test fn names present | 15/15 |
| `v2_unknown_method_404` uses `post_raw` | yes |
| `v2_unsupported_version_400_with_supported` asserts `.is_array()` | yes |

## TDD Gate Compliance

Every cycle completed a real RED→GREEN with the failure OBSERVED, not assumed.

| Task | RED commit | RED evidence | GREEN commit |
|------|-----------|--------------|--------------|
| 1 (D-113-A/B) | `73a24cf1` `test(113-04)` | 3 failing tests, e.g. `left: None, right: Some({"ns/key":"v"})` — the spec-spelled `_meta` was being dropped, and `tools/list` surfaced no signal | `47eaad68` `fix(113-04)` |
| 2 (`sessions_active`) | `2d16e23b` `test(113-04)` | `left: 400, right: 200` with body `{"error":{"code":-32600,"message":"Session ID required for non-initialization requests"}}` | `2baf265f` `feat(113-04)` |
| 5 (D-113-B redone) | `b2cc87fe` `revert(113-04)` | the revert put `tools_list_is_a_valid_v2_request` and `v2_nameless_method_empty_mcp_name_accepted` back to `left: 400, right: 200` with `"header claims v2 but _meta protocolVersion disagrees"` — the ORIGINAL defect, re-observed | `f6735c03` `feat(113-04)` |

The Task-5 RED is what proves the tripwires are green for the RIGHT reason: they failed again
the moment the fields went away, and only the raw-body gate brought them back.

The Task-1 RED was initially written against Rust fields that did not exist, which is a compile
error rather than an observable failure; it was restructured to drive everything from JSON so the
RED state COMPILES and FAILS. No REFACTOR commit was needed on any cycle.

## Threat Model Coverage

| Threat ID | Disposition | How this plan discharged it |
|-----------|-------------|------------------------------|
| T-113-06 | mitigate | `sessions_active` means the v2 path holds no session at all, so nothing downstream can treat a session id as identity. `v2_ignores_inbound_session_id` proves an attacker-supplied id is INERT (200, nothing echoed) rather than accepted |
| T-113-08 | mitigate | The Phase-112 cross-check is retained and now sentinel-aware; its reject cell emits `HEADER_MISMATCH` at HTTP 400, so an intermediary sees an unambiguous rejection status instead of a generic `-32600` |
| T-113-18 | mitigate | `v2_method_not_allowed` runs FIRST in both handlers — before header validation and before session validation. `v2_get_405` / `v2_delete_405` send a bogus session id that a v1 request answers 404 for, so the 405 proves session state was never consulted |
| T-113-19 | mitigate | The predicate returns `true` for `era == None` (not opted in) and `Era::V1`. `v1_session_unchanged` proves mint → validate → serve still works AND that a session-less v1 request is still rejected; the 25-test Phase-112 suite is the second guard |
| T-113-50 | mitigate | `map_unparsed_body_for_v2` runs at the RAW level on the body bytes; `v2_unknown_method_404` drives it with `"totally/unknown"`, a method string that cannot deserialize into any typed request, and asserts the original id survives |
| T-113-51 | mitigate | `V2GateOutcome::Reject` carries structured `data`; `-32022` always includes `data.supported`, asserted as an array both in a unit test and live over HTTP |

## Known Stubs

None. No `TODO`/`FIXME`/`unimplemented!()` was introduced; the `make quality-gate` zero-SATD
check passes. The one documented LIMITATION (a known method with malformed params reported as
`-32601` on v2) is a bounded, in-code-documented hand-off to plan 06, not a stub.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: status-surface | `src/server/streamable_http_server.rs` | v2 responses now emit HTTP 404 and 405 where the transport previously only emitted 200/400/401/404-for-session. Intermediaries, WAFs and load balancers that key on status may treat a 404 as "endpoint gone" and a 405 as a route misconfiguration. Intentional and spec-mandated; era-gated so v1 is unaffected |
| threat_flag: error-payload | `src/server/streamable_http_server.rs` | `-32022` rejections now disclose the server's full protocol accept-list in `error.data.supported`. This is spec-required (the client must be able to pick a mutually supported version) and reveals only version strings, but it is new outbound information at an unauthenticated boundary |
| threat_flag: schema-change | `src/types/{tools,prompts,resources}.rs` | Three public request types changed their `_meta` wire spelling from `meta` to `_meta`, accepting both on ingress. Trust-boundary relevant: `_meta` is the carrier for the era signal, client identity and W3C trace context, all of which Phase 112 documents as self-reported and NOT for authorization |
| threat_flag: parser-surface | `src/server/streamable_http_server.rs` | `raw_params_meta` parses the untrusted request body a second time (the header cross-check already did) to reach `params._meta` before any typed validation runs. It is non-panicking over arbitrary bytes and the resolver it feeds keeps every Phase-112 bound, but it does move era detection strictly earlier in the pipeline, onto bytes no typed layer has vetted |

## Follow-ups

1. **Plan 05** — the client must emit `Mcp-Name` through `mrtr::encode_header_value`, empty for
   name-less methods. The server half is now tested in both directions.
2. **Plan 06** — owns the precise `-32602` mapping for a known method with malformed params.
3. **Plan 09** — `-32021` reaches HTTP 400 for free; nothing to wire at the transport.
4. **Plan 10** — `subscriptions/listen` is v2-capable over HTTP for free: the raw reader does not
   need a typed `_meta` field. Only if the non-HTTP transports must also see its `_meta` does it
   need an `extract_request_meta_value` arm (that match is wildcard-free, so the compiler will
   ask). **Do not add a `_meta` field to a constructible public request struct** — that is what
   D-113-D measured as a MAJOR break.
5. **Plans 06/09/10** — the accepted cost of D-113-B's resolution: handlers reach the per-request
   `_meta` through the `ProtocolContext`-derived `RequestHandlerExtra` accessors, not through a
   typed field on a list-request struct. Settled; please do not re-litigate.
6. **Plan 12** — the semver gate is CLEAR (`223 pass, no semver update required`). The 113-01 spec
   verdict is still PENDING under a recorded exception; NO Phase-113 requirement (including
   HTTP-01) is marked complete by this plan.

## Self-Check: PASSED

- `tests/v2_stateless_http.rs` — FOUND (15 `#[tokio::test]`)
- `.planning/phases/113-.../deferred-items.md` — FOUND (D-113-A/B/C/D all RESOLVED)
- `src/server/streamable_http_server.rs` — FOUND (`fn sessions_active`, `v2_method_not_allowed`,
  `map_unparsed_body_for_v2`, `v2_status_for_code`, `raw_params_meta` all present;
  `run_v2_header_gate_raw` and `finish_v2_gate` correctly absent)
- `src/server/mod.rs` — FOUND (`resolve_raw_meta_protocol_context`,
  `supported_protocol_versions`)
- Commit `73a24cf1` — FOUND
- Commit `47eaad68` — FOUND
- Commit `2d16e23b` — FOUND
- Commit `2baf265f` — FOUND
- Commit `7ab9b205` — FOUND
- Commit `860cb269` — FOUND
- Commit `b2cc87fe` — FOUND
- Commit `f6735c03` — FOUND
