---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 10
subsystem: api
tags: [mcp-2026-07-28, subscriptions-listen, streamable-http, sse, capabilities, raii, backpressure, conformance, semver]

# Dependency graph
requires:
  - phase: 113-09
    provides: "inject_v2_result_envelope + own_reserved_result_fields — the shared v2 result envelope the terminal SubscriptionsListenResult is built through"
  - phase: 113-08
    provides: "envelope_for_live_request (the ONE direct-response constructor) and resumability_active, which the listen stream ASSERTS rather than re-derives"
  - phase: 113-04
    provides: "sessions_active, apply_v2_outbound_headers, v2_dispatch_response_status (code-driven 404 for -32601) and the raw-_meta single header gate"
  - phase: 112
    provides: "the crate-local HttpIngress classify-then-CONTINUE precedent, build_discover_response, and the error_codes table"
provides:
  - "src/types/subscriptions.rs — SubscriptionFilter / SubscriptionsListenParams / SubscriptionsListenResult / SubscriptionAcknowledgedParams with field types locked to 113-SPEC-RECHECK.md § A.6"
  - "advertises_subscriptions — the ONE predicate the server/discover projection and the subscriptions/listen route gate both read"
  - "HttpIngress::SubscriptionsListen — a crate-local ingress variant, so no public ClientRequest variant and semver stays MINOR"
  - "server::subscriptions::ListenRegistry / ListenKey / ListenEntry / ListenGuard / ListenFrame — a (principal, RequestId)-keyed, bounded, RAII-reclaimed v2 stream registry"
  - "Server::send_notification fans out to every live listen stream; Server::close_subscription_streams is the graceful-shutdown trigger"
  - "v2 retirement of resources/subscribe and resources/unsubscribe via ONE shared dispatch seam"
  - "tests/v2_subscriptions.rs — nine live-HTTP acceptances including the advertise-implies-serve tripwire"
affects: [113-11, 113-13, 114, 117, 118, 119]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "one shared capability predicate feeding both the advertisement and the implementation (drift tripwire)"
    - "RAII stream registration: the guard is stream STATE, so disconnect reclaims the entry and permits with no unregister call"
    - "bounded channel with a RESERVED final slot, so the overflow notice is deliverable into an otherwise-full buffer"
    - "ack-before-registration: frame ordering is structural, not conventional"

key-files:
  created:
    - src/types/subscriptions.rs
    - tests/v2_subscriptions.rs
  modified:
    - src/server/subscriptions.rs
    - src/server/streamable_http_server.rs
    - src/server/mod.rs
    - src/types/mod.rs

key-decisions:
  - "subscriptions/listen is routed as a crate-local HttpIngress variant carrying RAW params, never a public ClientRequest variant — semver-checks stays 223/223 with no enum_variant_added"
  - "the acknowledgement is pushed into the channel BEFORE register() inserts the entry, so 'ack is the first frame' is structurally true rather than a convention nothing enforces"
  - "the anonymous principal is a PER-STREAM counter (`anon#N`), not the plan's socket-address-plus-counter: it delivers the same registry-key isolation with zero ConnectInfo plumbing, at the stated cost that the per-principal cap does not bind for unauthenticated deployments (the global cap does)"
  - "the bounded channel is allocated at LISTEN_CHANNEL_CAPACITY + 1 and 'full' means one remaining slot, because the plan's terminal overflow comment is undeliverable into a genuinely full channel"
  - "the acknowledgement is a NOTIFICATION so it cannot carry the v2 RESULT envelope; only the terminal SubscriptionsListenResult goes through inject_v2_result_envelope, and both share one subscription_id_meta builder"
  - "a v1 subscriptions/listen now answers -32601@200 instead of the incidental pre-113 -32700@400 — the same deliberate, benign D-10 change Phase 112 made for server/discover, for the same reason (no conforming v1 client sends it)"
  - "the listen response deliberately does NOT run the HTTP response-middleware chain: that chain processes a complete Vec<u8> body and a stream has none by construction"

patterns-established:
  - "Pattern 1: capability advertisement and route gating read ONE predicate over ONE Server::capabilities() value, with a unit test flipping each capability in turn"
  - "Pattern 2: concurrency-bounded, self-cleaning stream registration — semaphore permits + registry entry both owned by an RAII guard moved into the stream's state"
  - "Pattern 3: a raw HTTP/1.1 TCP test client for incrementally-read bodies, with every read timeout-bounded"

requirements-completed: [HTTP-04]

# Metrics
duration: 60min
completed: 2026-07-25
---

# Phase 113 Plan 10: `subscriptions/listen` Summary

**An opt-in, capability-gated `subscriptions/listen` SSE stream — ack-first through the shared v2 envelope, `(principal, RequestId)`-keyed so callers reusing id `1` cannot cross-deliver, bounded with a disconnect-on-overflow policy, and RAII-reclaimed on client disconnect — with the advertise-nothing default answering `-32601`@404 conformantly and a local tripwire enforcing that advertising implies serving.**

## Performance

- **Duration:** ~60 min
- **Started:** 2026-07-25T23:15:02Z
- **Completed:** 2026-07-26T00:15:00Z
- **Tasks:** 3
- **Files modified:** 6 (2 created, 4 modified)

## Accomplishments

- **D-13 resolves D-12 in code.** Both conformant configurations ship: pmcp's stateless enterprise default advertises no subscription-delivered capability and answers `subscriptions/listen` with HTTP 404 + `-32601` (conformance records SKIPPED), while any advertised capability serves a real long-lived SSE stream. `advertise_implies_serve` runs the tripwire over each of the four capabilities individually.
- **All five cross-AI HIGH production-robustness findings are closed in the implementation, not deferred:** pair-keyed registry (#3), bounded channel with a documented lag policy (#4), RAII disconnect-safe unregistration (#5), `AuthContext` threaded into the registry from the existing POST resolution site (#6), and ack/result built through the shared v2 envelope + header helpers rather than a bespoke frame builder (MEDIUM).
- **`resources/subscribe` / `resources/unsubscribe` are retired on v2** through ONE shared dispatch seam both POST entrypoints call, with the v1 path byte-for-byte untouched (`tests/server_subscriptions.rs` still green).
- **Semver stays MINOR:** `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` reports `223 checks: 223 pass, 30 skip / Summary no semver update required`, with no `enum_variant_added`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Subscription types, the capability gate, and the advertise-implies-serve tripwire** — `8d4f138b` (feat)
2. **Task 2: The long-lived SSE listen stream — collision-free keys, bounded channel, RAII teardown, shared envelope** — `207ca356` (feat)
3. **Task 3: Live-SSE acceptance and the advertise-implies-serve tripwire test** — `1aa2aa1f` (test)
4. **Rule 2 addition: the agreed-filter intersection property (CLAUDE.md ALWAYS-PROPERTY)** — `26dcc169` (test)

## Files Created/Modified

- `src/types/subscriptions.rs` (new, 991 lines) — the wire nouns (`SubscriptionFilter`, `SubscriptionsListenParams`, `SubscriptionsListenResult`, `SubscriptionAcknowledgedParams`), the reserved-name constants (`SUBSCRIPTION_ID_META_KEY`, `ACKNOWLEDGED_METHOD`, `SUBSCRIPTIONS_LISTEN_METHOD`), the `advertises_subscriptions` predicate quoting the conformance suite's gating expression verbatim, the `SubscriptionNotificationKind` classifier (which excludes `progress`/`message` by having no variant for them), and the frame-tagging helper.
- `src/server/subscriptions.rs` (+692) — `ListenRegistry`, `ListenKey`, `ListenEntry`, `ListenGuard`, `ListenFrame`, `ListenRejection`, `anonymous_principal`, the three concurrency constants, `register`/`fan_out`/`close_all`/`disconnect_overflowed`, plus module docs recording D-11 and the instance-local caveat.
- `src/server/streamable_http_server.rs` (+764) — `HttpIngress::SubscriptionsListen`, its classification, `v2_retired_method_of` + `dispatch_request_or_retire` (the shared retirement seam), `listen_server_view`, `listen_rejection_response`, `listen_ack_frame`, `listen_terminal_result_frame`, `listen_sse_event`, `attach_listen_response_headers`, `resolve_agreed_filter`, and `assemble_subscriptions_listen`.
- `src/server/mod.rs` (+90) — the `listen_registry` field and accessor, `capabilities()`/`info()` accessors, `send_notification` fan-out, the public `close_subscription_streams()`, and the build-time instance-local `tracing::warn!`.
- `src/types/mod.rs` (+10) — `pub mod subscriptions;` and the narrow flat re-export.
- `tests/v2_subscriptions.rs` (new, 856 lines) — the nine live-HTTP acceptances plus the raw streaming SSE client.

## Verification

| Command | Result |
|---|---|
| `cargo test --lib --features full -- subscriptions` | 43 passed |
| `cargo test --test v2_subscriptions --features full` | 9 passed |
| `cargo test --test server_subscriptions --features full` | 6 passed (v1 baseline) |
| `cargo test --test v2_stateless_http --features full` | 23 passed |
| `cargo test --test v2_required_headers --features full` | 25 passed |
| `cargo test --test v2_client --features full` | 21 passed |
| `cargo test --test v2_mrtr_ingress --features full` | 10 passed |
| `cargo test --test common_harness_smoke --features full` | 7 passed |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | 223 pass, no update required |
| `make quality-gate` | PASSED (exit 0) |
| `grep -c 'unbounded_channel' src/server/subscriptions.rs` | `0` |

## Decisions Made

1. **Crate-local ingress, raw params.** `subscriptions/listen` classifies into `HttpIngress::SubscriptionsListen { id, params }` carrying the RAW `params` value. The classifier must never reject a body, so a malformed `params` becomes a structured `-32602` in the served branch — AFTER the header gate and auth — rather than a parse error before them.
2. **Ack-before-registration.** The transport creates the channel, `try_send`s the acknowledgement, and only THEN calls `register`. Nothing can reach the channel before the entry exists, so the spec's "MUST be the first message" is structural. `ack_is_first_frame` fires five notifications immediately after the request goes out and still reads the ack first.
3. **Reserved overflow slot.** The plan asks for "one terminal SSE comment noting the overflow", which is undeliverable into a genuinely full channel. The channel is therefore allocated at `LISTEN_CHANNEL_CAPACITY + 1` and `fan_out` treats `sender.capacity() <= 1` as full, reserving the last slot for the notice. Per-subscriber memory is still bounded by the named constant.
4. **Per-stream anonymous principal.** See Deviations — the isolation property is identical, the cap granularity differs, and the trade is documented at the definition site.
5. **The ack is a notification.** `inject_v2_result_envelope` returns early on a non-`Result` payload by design, so only the terminal `SubscriptionsListenResult` goes through it. Both frames build their `_meta` from the same `subscription_id_meta` helper, so the key spelling cannot diverge.
6. **`v1` `subscriptions/listen` answers `-32601`@200.** Pre-113 it fell out of the typed parse as `-32700`@400. This is the same deliberate, benign D-10 change Phase 112 made for `server/discover`: no conforming v1 client sends a v2-only method, so no v1-relied-upon response byte changes.
7. **No response middleware over the stream.** The chain processes a complete `Vec<u8>` body; a stream has none. Documented at the function, not left implicit.

## Deviations from Plan

### Auto-fixed / adjusted

**1. [Rule 3 - Blocking] The anonymous principal is a per-stream counter, not the socket address plus a counter**

- **Found during:** Task 2 (the listen registry)
- **Issue:** The plan specifies "the remote socket address plus a per-connection counter" as the anonymous `ListenKey` principal. The axum router has no `ConnectInfo<SocketAddr>` wired: reaching the remote address requires switching `StreamableHttpServer::start` to `into_make_service_with_connect_info` AND making the extractor optional for the `pmcp::axum::router()` composition path, which changes shared service construction for a value that adds nothing once a counter is already in the key.
- **Fix:** `anonymous_principal()` returns `anon#<monotonic counter>`. Each anonymous stream is its own principal, so two anonymous callers both using id `1` occupy DISTINCT `ListenKey`s — the exact isolation property the pair-keying exists for, and strictly stronger than a socket address (which collapses two callers behind one NAT onto one identity).
- **Stated cost:** `MAX_LISTEN_STREAMS_PER_PRINCIPAL` does not bind for an unauthenticated deployment; `MAX_LISTEN_STREAMS_TOTAL` is the operative bound there. Documented in the `anonymous_principal` rustdoc as an ACCEPTED COST alongside the same posture MRTR takes (`core::ANONYMOUS_PRINCIPAL`).
- **Files modified:** `src/server/subscriptions.rs`
- **Verification:** `anonymous_principals_are_never_shared` (unit) plus `two_callers_same_request_id_do_not_cross` (live, authenticated principals).
- **Committed in:** `207ca356`

**2. [Rule 2 - Missing Critical] `close_all` needed a reachable production trigger**

- **Found during:** Task 2
- **Issue:** The plan names "the server's shutdown signal firing" as a closure trigger, but `StreamableHttpServer::start` has no graceful-shutdown signal to hook, and a `pub(crate)` `close_all` reachable only from tests is dead code under the `-D warnings` gate.
- **Fix:** Added `Server::close_subscription_streams()` (additive public API, semver-minor) as the documented shutdown hook, and stored each stream's pre-built terminal frame in its `ListenEntry` so the registry needs no envelope logic of its own.
- **Files modified:** `src/server/mod.rs`, `src/server/subscriptions.rs`
- **Verification:** `close_all_sends_the_terminal_result_then_ends_each_stream`; `cargo semver-checks` still 223/223.
- **Committed in:** `207ca356`

**3. [Rule 2 - CLAUDE.md ALWAYS] Added the agreed-filter property test**

- **Found during:** post-task verification
- **Issue:** CLAUDE.md mandates PROPERTY testing for every new feature; the plan listed only example-based unit tests for the intersection.
- **Fix:** `the_agreed_filter_is_the_intersection_and_nothing_more` asserts over the full 2^8 (requested × supported) input space that the agreed filter covers a kind ONLY IF requested AND supported — the invariant T-113-34 rests on — and pins `advertises_subscriptions` against the same generated capabilities.
- **Files modified:** `src/types/subscriptions.rs`
- **Verification:** `cargo test --lib --features full -- subscriptions` → 43 passed.
- **Committed in:** `26dcc169`

**4. [Rule 3 - Blocking] The live test harness needed a raw HTTP/1.1 client**

- **Found during:** Task 3
- **Issue:** The plan specifies `reqwest`'s `bytes_stream()`; `reqwest` is declared in `Cargo.toml` WITHOUT its `stream` feature, and enabling it would perturb the shipped dependency tree the milestone measured as unchanged (113-01's 728→728 lockfile check).
- **Fix:** `tests/v2_subscriptions.rs` speaks HTTP/1.1 over a `tokio::net::TcpStream`, decoding both `Transfer-Encoding: chunked` (the served stream) and `Content-Length` (every rejection). Every read is wrapped in `tokio::time::timeout`, per the plan's requirement. This also buys a deterministic client disconnect with no connection pool holding the socket open — exactly what `disconnect_releases_registry_slot` needs.
- **Files modified:** `tests/v2_subscriptions.rs`
- **Verification:** 9 tests pass; zero `Cargo.toml` change.
- **Committed in:** `1aa2aa1f`

**5. [Rule 3 - Blocking] `Server` needed `capabilities()` / `info()` accessors**

- **Found during:** Task 1
- **Issue:** The listen gate must read the SAME `ServerCapabilities` the discover projection publishes, and the terminal result needs the server's `Implementation`; both fields were private with no accessor.
- **Fix:** Added `pub(crate) fn capabilities()` and `pub(crate) fn info()` to `Server` (in `src/server/mod.rs`, which Task 2's file list already covers).
- **Files modified:** `src/server/mod.rs`
- **Verification:** `discover_projection_and_listen_gate_read_the_same_predicate`.
- **Committed in:** `8d4f138b`

---

**Total deviations:** 5 (2 Rule 3 blocking-environment adaptations, 2 Rule 2 missing-critical additions, 1 Rule 3 mechanism substitution with an identical guarantee)
**Impact on plan:** No scope creep. Every acceptance criterion in the plan is met; deviations 1 and 4 substitute mechanisms that deliver the SAME stated guarantee without perturbing shared service construction or the measured dependency tree, and both are documented at the definition site rather than only here.

## Deltas against `113-SPEC-RECHECK.md`

**None.** Every field type matches the § A.6 record exactly:

- `SubscriptionFilter.resourceSubscriptions` is `Option<Vec<String>>` (`string[]`), pinned by BOTH a positive test deserializing the recorded declaration and a negative test asserting a boolean value is REJECTED.
- `SubscriptionsListenRequestParams.notifications` and `SubscriptionsAcknowledgedNotificationParams.notifications` are REQUIRED (no `?`), pinned by `listen_params_require_the_notifications_field`.
- `SubscriptionsListenResult._meta` is REQUIRED and carries a REQUIRED `io.modelcontextprotocol/subscriptionId`.

## Threat Register Coverage

| Threat | Mitigation as shipped |
|---|---|
| T-113-09 (DoS: stream exhaustion) | `MAX_LISTEN_STREAMS_PER_PRINCIPAL` + `MAX_LISTEN_STREAMS_TOTAL` semaphore permits released by RAII; stream OFF by default |
| T-113-34 (unrequested notification types) | agreed filter = requested ∩ supported computed once at registration; `fan_out` checks containment; `progress`/`message` excluded by construction; proptest over 2^8 inputs |
| T-113-29 (event replay onto the stream) | the listen branch never reads `Last-Event-ID` and never touches the store; `debug_assert!(!resumability_active(..))` asserts plan 08's rule rather than re-deriving it |
| T-113-35 (advertisement/implementation drift) | ONE `advertises_subscriptions` predicate + `advertise_implies_serve` over all four capabilities individually |
| T-113-36 (hung stream wedging CI) | ack is first by construction; keep-alive comments every 15s; graceful close sends the JSON-RPC result; every test read is timeout-bounded |
| T-113-61 (cross-caller delivery via id collision) | `ListenKey { principal, request_id }`; `two_callers_same_request_id_do_not_cross` is the live proof |
| T-113-62 (unbounded slow-subscriber buffer) | bounded `mpsc::channel(LISTEN_CHANNEL_CAPACITY + 1)` with a reserved notice slot, `warn`-logged disconnect-on-overflow |
| T-113-63 (leaked entries/permits on disconnect) | `ListenGuard` moved into the stream state; `Drop` removes the entry and releases both permits; `disconnect_releases_registry_slot` proves it live |
| T-113-64 (silent under-delivery behind a load balancer) | registry documented INSTANCE-LOCAL in the module docs; build-time `tracing::warn!` fires when a subscription capability is advertised |

## Known Stubs

**None.** No hardcoded empty values, placeholder text, or unwired data sources were introduced. Cross-instance notification delivery is explicitly OUT OF SCOPE for this phase (documented in the module docs and warned about at build time), not stubbed.

## Threat Flags

**None.** The only new network surface is `subscriptions/listen`, which is the plan's subject and is fully covered by the register above. It runs through the SAME session → v2 header matrix → legacy-version → auth pipeline as every other POST ingress — a held-open stream is not a way around auth.

## Issues Encountered

- **`Value::pointer` cannot address the reserved key.** `io.modelcontextprotocol/subscriptionId` contains a `/`, which JSON Pointer treats as a path separator unless escaped as `~1`. The first test run reported `None` for every `subscriptionId`. Fixed by indexing (`get("params")?.get("_meta")?.get(KEY)`) with a comment naming the trap, so nobody reintroduces a pointer here.
- **Rejections are not SSE.** The concurrency-refusal path returns a `Content-Length` JSON body, which the first version of the raw client (chunked-only) could not read, so `disconnect_releases_registry_slot` timed out. The client now handles both framings and a test reads the first frame identically either way.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Plan 13 (HTTP-04 client half) is unblocked.** The server wire contract is now fixed and test-pinned: `subscriptions/listen` params shape, the ack method and its `notifications` field, the `subscriptionId` `_meta` key on every frame, and the terminal `SubscriptionsListenResult`. The public types the client needs are re-exported from `pmcp::types::subscriptions`.
- **Plan 11 (conformance)** can rely on `absent_capability_is_conformant` matching the suite's gating logic: the default pmcp server records SKIPPED, not FAILURE, and the conjunction (observed discover AND no advertisement) is what the local test asserts.
- **Open, deliberately out of scope:** a cross-instance notification backend. Advertising a subscription capability behind a non-sticky load balancer under-delivers silently; the build-time warning names the constraint but does not prevent the configuration.

## Self-Check: PASSED

Files verified on disk: `src/types/subscriptions.rs` (927 lines, min 130, contains `SubscriptionFilter`), `tests/v2_subscriptions.rs` (856 lines, min 220), `src/server/subscriptions.rs`, `src/server/streamable_http_server.rs` (contains `subscriptions/listen`), `src/server/mod.rs`, `src/types/mod.rs`, `113-10-SUMMARY.md`.

Commits verified in `git log`: `8d4f138b`, `207ca356`, `1aa2aa1f`, `26dcc169`, `53efc0f2`.

Key links verified: `advertises_subscriptions` (6 sites in the transport, incl. the gate and the agreement test), `ListenKey` (8 sites in the registry), `inject_v2_result_envelope` (called in the listen terminal-result builder).

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-25*
