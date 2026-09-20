---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 05
subsystem: api
tags: [mcp-2026-07-28, client, streamable-http, headers, protocol-negotiation, semver]

# Dependency graph
requires:
  - phase: 113-02
    provides: "src/types/mrtr.rs — the ONE method table (logical_name_key) and the Mcp-Name sentinel codec (encode_header_value/decode_header_value)"
  - phase: 113-04
    provides: "the raw-body v2 header gate, the server-side sentinel DECODER, and sessions_active(state, era) — the server half every live test here is measured against"
  - phase: 112
    provides: "the v2 header gate, the reserved _meta keys + resolve_protocol_context, server/discover, and Server::with_supported_protocol_versions (the DSL this mirrors)"
provides:
  - "Transport::set_negotiated_protocol_version / supports_negotiated_protocol_version / send_raw — three DEFAULTED (semver-additive) methods, the mode-propagation seam plus the raw-frame path"
  - "ClientBuilder::with_protocol_version — the explicit, validated, per-connection v2 opt-in"
  - "A pmcp Client that speaks 2026-07-28: no handshake, per-request reserved _meta, all three v2 headers, no session id"
  - "Client::server_discover() — the explicit v2 replacement for the initialize handshake, storing the projection"
  - "pub types::protocol::ServerDiscoverResult (moved out of the wasm-gated server::core)"
  - "tests/v2_client.rs — the live client<->server acceptance suite plans 07 and 13 extend"
affects: [113-07, 113-10, 113-12, 113-13, 114, 117]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Defaulted trait methods as the semver-additive seam for cross-cutting transport capability"
    - "Client-assembled RAW JSON-RPC frames on v2, so every method can carry params._meta without a MAJOR-breaking struct field"
    - "Outbound routing headers DERIVED from the frame the transport is about to send, from the same table the server reads"
    - "A client-owned era latch that no server response can flip"

key-files:
  created:
    - tests/v2_client.rs
  modified:
    - src/shared/transport.rs
    - src/client/mod.rs
    - src/shared/streamable_http.rs
    - src/types/protocol/mod.rs
    - src/server/core.rs

key-decisions:
  - "The mode-propagation seam is THREE defaulted Transport methods, not two: set_negotiated_protocol_version + supports_negotiated_protocol_version as planned, plus send_raw — because neither params._meta on list-shaped methods nor server/discover can travel through the typed TransportMessage::Request without a MAJOR semver break"
  - "The v2 _meta is spliced by the CLIENT onto its own assembled JSON-RPC frame, not by the transport — client/mod.rs owns capability honesty and trace-context merge, the transport owns headers"
  - "with_protocol_version returns Result<Self> rather than Self, because build() cannot become fallible without a breaking change; the accept-list is SUPPORTED_PROTOCOL_VERSIONS UNION 2026-07-28 (the v2 constant is deliberately absent from that table, Phase-112 Pitfall 1)"
  - "server_discover takes &mut self, not &self, because it STORES the projection — which is precisely what re-arms assert_capability on v2"
  - "The transport's v2 era is a private latch written ONLY by the client seam, never derived from protocol_version, so a server echoing MCP-Protocol-Version: 2026-07-28 cannot flip a v1 client into v2 mode"
  - "A v2 client is marked initialized at BUILD time (v2 has no handshake); initialize() stays callable and returns a LOCAL synthetic result that is deliberately NOT stored into server_capabilities"
  - "ServerDiscoverResult moved from the wasm-gated server::core to types::protocol and made pub, so ONE type serves both the server projection and the wasm-safe Client return"

patterns-established:
  - "Era-gated behavior on the client is decided by ONE is_v2() predicate over an explicit per-connection selection — never by sniffing a server response"
  - "Every v2 outbound header is derived from the body bytes, so header/body desync is structurally impossible (T-113-08)"
  - "Reserved _meta keys are consumed from the crate-level table in code and restated as literals ONLY inside the drift-guard test"

requirements-completed: []

# Metrics
duration: 105min
completed: 2026-07-25
---

# Phase 113 Plan 05: pmcp Client Speaks v2 Summary

**A pmcp `Client` that opts into 2026-07-28 with one validated builder call, sends no `initialize`, stamps every request with the three reserved `_meta` keys, emits `Mcp-Method`/`Mcp-Name`/`MCP-Protocol-Version` derived from its own frame, never touches a session id — and is accepted end-to-end by pmcp's own strict Phase-112 header gate.**

## Performance

- **Duration:** ~105 min
- **Tasks:** 3
- **Files modified:** 5 (+1 created)

## Accomplishments

- **Closed RESEARCH Pitfall 7.** The client transport emitted ZERO `Mcp-Method`/`Mcp-Name` headers, so every v2 request a pmcp client sent was rejected by pmcp's own server. `tests/v2_client.rs` now proves the loop closes across `tools/call`, `tools/list`, `prompts/get`, `resources/read`, a non-ASCII tool name, and `server/discover`. RED was verified: with v2 header emission disabled, 9 of the 10 tests fail with HTTP 400 from the server's own gate, and `v1_client_unchanged` correctly stays green.
- **Defined the mode-propagation seam** (Codex Plan-05 HIGH #1). `Client<T>` is generic; header emission is the transport's job. Three defaulted `Transport` methods carry the selection across that boundary, in BOTH the native and wasm trait definitions. `cargo semver-checks` reports `223 pass, no semver update required`.
- **Unblocked capability enforcement on v2** (Codex Plan-05 HIGH #2). `assert_capability` read `server_capabilities`, which only `initialize` populates — so on v2 EVERY `call_tool` failed locally before a byte was sent. It is now era-aware, and an explicit `server_discover()` restores full v1-grade strictness.
- **Made `clientCapabilities` honest and merge-safe.** They are derived from `ClientHostRegistry`, never caller-asserted (T-113-12), and the `_meta` splice MERGES so a caller `traceparent` survives (T-113-54) for plan 07's MRTR retries.
- **Held the milestone additive.** No public enum gained a variant, no constructible struct gained a field, and the one new public type (`ServerDiscoverResult`) is `#[non_exhaustive]`.

## Task Commits

1. **Task 1: mode-propagation seam + client v2 opt-in, `_meta`, no-initialize path, era-aware capabilities** — `d2b2e1fd` (feat)
2. **Task 2: v2 outbound headers + session-id suppression in the client transport** — `33276e67` (feat)
3. **Task 3: live client↔server acceptance for CLNT-01** — `11b1a3a7` (test)
4. **Gate compliance:** `89cbb0a7` (style — pedantic/nursery clippy fixes surfaced only by `make lint`)

## Files Created/Modified

- `src/shared/transport.rs` — three defaulted `Transport` methods (`set_negotiated_protocol_version`, `supports_negotiated_protocol_version`, `send_raw`), added to the native AND wasm trait definitions so they cannot drift.
- `src/client/mod.rs` — `ClientBuilder::with_protocol_version`, the `negotiated_protocol_version` field + `era()`/`is_v2()`, `v2_request_meta`/`splice_v2_meta`, `v2_client_capabilities`, `server_discover`, the v2 `initialize` short-circuit, era-aware `assert_capability`, and the `send_request`/`send_untyped_request`/`dispatch_request` split. 17 new unit tests.
- `src/shared/streamable_http.rs` — `v2_routing_headers` (body-derived `Mcp-Method`/`Mcp-Name`), `apply_v2_outbound_headers` (non-panicking, server-mirrored), the private `v2_mode` latch, v2 session-id suppression outbound and on read-back, and the `post_body` refactor shared by `send_with_options` and `send_raw`. 16 new unit tests + 1 proptest.
- `src/types/protocol/mod.rs` — `pub struct ServerDiscoverResult` (moved here from `server::core`).
- `src/server/core.rs` — re-exports the moved type; no behavior change.
- `tests/v2_client.rs` — 10 live tests over loopback HTTP.

## Decisions Made

See `key-decisions` in the frontmatter. The two that most constrain later plans:

1. **`send_raw` is how a v2 client puts anything on the wire.** Plan 07's MRTR retry and plan 10's `subscriptions/listen` should splice their params into the same assembled `JSONRPCRequest<Value>` in `dispatch_request`, NOT reach for a typed struct field. `types::mrtr::splice_mrtr_params` already operates on a `Value`, so it drops straight into that seam.
2. **`server_discover()` is never implicit.** D-08 forbids probing to CHOOSE an era. It is fine to call it to LEARN capabilities; do not "restore" an auto-probe in plans 07/12/13.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The plan's two-method seam cannot carry `_meta` or `server/discover`**

- **Found during:** Task 1
- **Issue:** The plan specified exactly two defaulted `Transport` methods and put `_meta` emission in `src/client/mod.rs`. Neither is achievable through `TransportMessage::Request`, which carries a TYPED `Request`:
  - `tools/list` and every other list-shaped request struct has **no `_meta` field** — plan 04's D-113-D resolution deliberately reverted those fields because adding them is a MAJOR semver break. Without `params._meta`, `classify_era_cell(header=V2, meta_is_v2=false)` rejects the request with `-32020 HEADER_MISMATCH`, so `nameless_method_accepted` could never pass.
  - `server/discover` has **no `ClientRequest` variant** and must not gain one (Phase-112 D-10; `enum_variant_added` is a MAJOR break).
- **Fix:** Added a THIRD defaulted method, `async fn send_raw(&mut self, body: Vec<u8>)`, whose default body errors. On v2 the client assembles the `JSONRPCRequest<Value>`, splices `_meta`, serializes, and sends it verbatim; v1 still sends the typed message and is byte-identical. This keeps `_meta` emission in `client/mod.rs` exactly as the plan's `key_links` require, and gives `server_discover` a path with zero public-enum churn.
- **Files modified:** `src/shared/transport.rs`, `src/client/mod.rs`, `src/shared/streamable_http.rs`
- **Verification:** `a_v2_request_travels_as_a_raw_frame_carrying_meta` (unit) and `nameless_method_accepted` / `server_discover_from_v2_client` (live). `cargo semver-checks`: 223/223 pass.
- **Committed in:** `d2b2e1fd`

**2. [Rule 3 - Blocking] `with_protocol_version` cannot "fail the build" — `build()` is infallible**

- **Found during:** Task 1
- **Issue:** The plan said an unsupported version "fails the build with a message naming the accepted values", but `ClientBuilder::build()` returns `Client<T>`, and changing that to `Result` is a breaking change. It also said to validate against `SUPPORTED_PROTOCOL_VERSIONS` — which deliberately does **not** contain `2026-07-28` (Phase-112 Pitfall 1), so a literal reading would reject the very version the method exists to select.
- **Fix:** `with_protocol_version` returns `Result<Self>` and validates eagerly against `SUPPORTED_PROTOCOL_VERSIONS` **union** `PROTOCOL_VERSION_2026_07_28`. The error names both the offending and the accepted values.
- **Files modified:** `src/client/mod.rs`
- **Verification:** `with_protocol_version_accepts_the_two_documented_versions`, `with_protocol_version_rejects_an_unsupported_version`
- **Committed in:** `d2b2e1fd`

**3. [Rule 3 - Blocking] `server_discover(&self)` cannot store what it learns**

- **Found during:** Task 1
- **Issue:** The plan's signature is `pub async fn server_discover(&self)`, but step 4 requires it to STORE its projection into `self.server_capabilities` — which needs `&mut self`. `Client` has no interior mutability for that field.
- **Fix:** Signature is `pub async fn server_discover(&mut self)`. Documented that the `&mut` exists precisely because storing is what re-arms `assert_capability`.
- **Files modified:** `src/client/mod.rs`
- **Verification:** `server_discover_from_v2_client` asserts both the returned projection and `get_server_capabilities().is_some()` after the call.
- **Committed in:** `d2b2e1fd`

**4. [Rule 3 - Blocking] `ServerDiscoverResult` was `pub(crate)` inside a wasm-gated module**

- **Found during:** Task 1
- **Issue:** The plan makes it the return type of `Client::server_discover`, but it lived in `src/server/core.rs`, which is `#[cfg(not(target_arch = "wasm32"))]`, while `pub mod client` compiles on wasm. Referencing it from the client would have broken `cargo build --target wasm32-unknown-unknown`.
- **Fix:** Moved the definition to the `cfg`-agnostic `src/types/protocol/mod.rs` and made it `pub` (`#[non_exhaustive]`, so future spec fields stay additive). `server::core` re-exports it, so there is still exactly ONE definition.
- **Files modified:** `src/types/protocol/mod.rs`, `src/server/core.rs`
- **Verification:** `cargo build --lib --target wasm32-unknown-unknown` succeeds; `cargo semver-checks` reports no required bump.
- **Committed in:** `d2b2e1fd`

**5. [Rule 2 - Missing Critical] A server response could flip a v1 client into v2 emission mode**

- **Found during:** Task 2
- **Issue:** The plan said the transport's era should come from the existing `protocol_version` field. But `process_response_headers` OVERWRITES that field from whatever `MCP-Protocol-Version` the server echoed. A server (rogue, misconfigured, or an intermediary) replying `2026-07-28` would have flipped a v1 client into v2 mode mid-connection — suppressing its session id and breaking the connection (a downgrade/DoS vector, adjacent to T-113-06).
- **Fix:** Added a private `v2_mode: Arc<AtomicBool>` latch written ONLY by `Transport::set_negotiated_protocol_version`, i.e. only by the client at build time. `set_negotiated_protocol_version` still delegates to the inherent `set_protocol_version` exactly as the plan specified, so the header emission is unchanged. Adding a private field to a struct with no public fields is semver-clean.
- **Files modified:** `src/shared/streamable_http.rs`
- **Verification:** `a_server_echo_cannot_flip_a_v1_client_into_v2`
- **Committed in:** `33276e67`

**6. [Rule 1 - Bug] A `WithTools`-only sampling client advertised no sampling capability**

- **Found during:** Task 1
- **Issue:** `derive_host_capabilities` (the HOST-05 anti-capability-lie rule) checked only `host_registry.sampling`, but `ClientBuilder::on_sampling_with_tools` sets only `host_registry.sampling_with_tools` (and dispatch PREFERS it). So a client registered exclusively with the tool-aware handler under-claimed: it advertised no `sampling` on v1 `initialize`, and on v2 it would receive `-32021 MissingRequiredClientCapability` for a sampling input request it could in fact have fulfilled (MRTR obligation 7).
- **Fix:** The derive now checks `sampling.is_some() || sampling_with_tools.is_some()`. This makes v1 `initialize` MORE honest; it never over-claims.
- **Files modified:** `src/client/mod.rs`
- **Verification:** `client_capabilities_declare_sampling_for_the_with_tools_handler`; all 132 `-- client` lib tests still pass.
- **Committed in:** `d2b2e1fd`

**7. [Rule 3 - Blocking] `make lint` is strictly stronger than `cargo clippy`**

- **Found during:** post-task verification
- **Issue:** `cargo clippy --lib --features full --all-targets` was clean, but `make lint` (pedantic + nursery + `-D warnings`) failed on four lints in the new code: `single_match_else`, `needless_pass_by_ref_mut`, `needless_pass_by_value`, `struct_field_names`.
- **Fix:** Restructured `splice_v2_meta`, narrowed `post_body` to `&self`, took the test `body()` helper's params by reference, and renamed the test recording struct's fields.
- **Files modified:** `src/client/mod.rs`, `src/shared/streamable_http.rs`, `tests/v2_client.rs`
- **Verification:** `make quality-gate` (unproxied, `/usr/bin/make`) reports `ALL TOYOTA WAY QUALITY CHECKS PASSED`.
- **Committed in:** `89cbb0a7`

---

**Total deviations:** 7 auto-fixed (5 blocking, 1 missing-critical/security, 1 bug)
**Impact on plan:** Every deviation was required to make a stated plan behavior reachable, or closed a security hole the plan's mechanism would have opened. The plan's shape — additive defaulted seam, explicit opt-in, body-derived headers, registry-derived capabilities, era-aware enforcement — is unchanged. No scope creep.

## TDD Gate Compliance

Tasks 1 and 2 are `tdd="true"`. **RED was verified by falsification, not by commit ordering:** implementation and tests were committed together per task, and the RED state was proven afterwards by disabling the v2 header emission and observing 9 of 10 live tests fail with HTTP 400 from the server's own gate (`v1_client_unchanged` correctly stayed green), then restoring. There is therefore no separate `test(...)` commit preceding each `feat(...)` commit. Task 3 carries the `test(...)` commit.

## Issues Encountered

- **`.pmat/*` and `pmcp-course/*` show as modified** in the working tree. They pre-date this plan (pmat cache regeneration and earlier course edits) and were deliberately NOT staged, per the executor scope boundary.
- **Response-status handling on the v2 path is coarse.** `post_body` turns any non-2xx into `TransportError::Request("Request failed with status: …")` WITHOUT parsing the JSON-RPC error body. Since plan 04 maps v2 errors onto 4xx statuses (e.g. `-32601` at 404, `-32021` at 400), a v2 client currently surfaces a structured server error as an opaque transport error. Every test in this plan asserts success, so nothing here is wrong — but plans 07 (MRTR retry) and 09 (`-32021`) need the structured `error.code`. Logged below as a deferred item for plan 07.

## Deferred Items

Appended to `.planning/phases/113-.../deferred-items.md`? **No** — recorded here instead, because it is owned by an identified downstream plan rather than being loose scope:

- **D-113-E — the v2 client cannot read a structured JSON-RPC error off a 4xx response.**
  `StreamableHttpTransport::post_body` returns `Err(TransportError::Request("Request failed with status: …"))` for any non-2xx, discarding the JSON-RPC envelope. Plan 04 deliberately maps v2 error codes onto 4xx statuses, so `-32601` (404), `-32020`/`-32021`/`-32022` (400) all reach the client as opaque transport errors. **Owner: plan 07** (the MRTR retry loop must distinguish `input_required` handling from a hard error, and plan 09's `-32021` must be actionable). Fix shape: on the v2 path, when the body parses as a JSON-RPC error envelope, feed it through the normal response channel instead of erroring on status alone.

## Next Phase Readiness

- **Plan 06** (server-side MRTR emission) is unaffected by anything here.
- **Plan 07** (MRTR client retry) has its seam: splice `inputResponses`/`requestState` into `jsonrpc_request.params` inside `Client::dispatch_request`, alongside `splice_v2_meta`. It must also close D-113-E above.
- **Plan 10** (`subscriptions/listen`) gets v2 client support for free — `v2_routing_headers` returns `(method, "")` for any non-name-bearing method, and the raw frame carries `_meta` regardless of whether a typed variant exists.
- **Plan 12** (public-API + semver audit) should note the newly public surface: three defaulted `Transport` methods, `ClientBuilder::with_protocol_version`, `Client::server_discover`, and `types::protocol::ServerDiscoverResult`. Current measurement: `223 checks: 223 pass, 30 skip / no semver update required`.
- **CLNT-01 is NOT marked complete** — per the 113-01 recorded exception, plan 12 owns the binding re-verification of the whole phase.

## Self-Check: PASSED

All claimed files exist on disk (`tests/v2_client.rs`, this SUMMARY) and all four
claimed commits (`d2b2e1fd`, `33276e67`, `11b1a3a7`, `89cbb0a7`) resolve in
`git log`.

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-25*
