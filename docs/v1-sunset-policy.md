# MCP v1 (2025-11-25) Sunset Policy

## Overview

This document is the **normative** policy for how and when PMCP will stop supporting the
MCP 2025-11-25 protocol era ("v1"). It states a **condition**, not a date.

PMCP is a dual-version SDK: it speaks both MCP 2025-11-25 and the 2026-07-28 ("v2") spec,
negotiated per request. v2 is the strategic primary path. v1 remains fully supported, and
this document exists so that "fully supported" and "cheaply removable later" are both true
at the same time — and so that a future contributor can tell which is which.

## What `v1-compat` gates

`v1-compat` is a cargo feature. It is **default-on**: it is a member of both `default` and
`full`, so every existing consumer already has it and nothing changes for them.

The list below is the **post-cut reality** as of Phase 117, not a plan. It was cross-read
against the source in both directions — every path named here is gated in the code, and
every gated site in `src/` is named here. The equally important second list, **what is
deliberately NOT severed**, follows it: a policy that overstates the severance is worse than
one that understates it, because a consumer plans a 3.0 migration against it.

### Server

| Surface | Path | Why it is v1-only |
|---|---|---|
| The whole v1 session lifecycle: `initialize`-driven session creation, session validation, the session map, the per-session negotiated version, SSE stream registry | `src/server/streamable_http_server/v1_session.rs` (paired with `v1_session_off.rs`) | v2 is stateless — `initialize` is removed from the spec and there is no session to track |
| SSE **resumability**: `Last-Event-ID` replay, the transport-local event store, the `EventStoreHandle` alias | `src/server/streamable_http_server/v1_session.rs` | v2 removes `Last-Event-ID`; a dropped v2 stream is re-issued as a new request rather than replayed |
| The `GET /` and `DELETE /` **bodies** — SSE stream setup and session teardown | `src/server/streamable_http_server/v1_session.rs` (`handle_get_sse_body`, `handle_delete_body`) | Both verbs answer `405` on 2026-07-28. See “Refused, not unrouted” below |
| The reader of the inbound `Mcp-Session-Id` request header on the **fast** POST path | `src/server/streamable_http_server/v1_session.rs` (`incoming_session_header`) | A build with no sessions never parses an inbound session id **on the fast POST path**. The middleware POST path is a separate reader — see “deliberately NOT severed” below |
| The standalone resumption-token store and its tokens | `src/shared/event_store.rs` (whole module, plus its `pub use` re-exports in `src/shared/mod.rs`) | Same reason as resumability above |
| Four `StreamableHttpServerConfig` **public fields**: `session_id_generator`, `event_store`, `on_session_initialized`, `on_session_closed`, plus the private `SessionCallback` alias that types two of them | `src/server/streamable_http_server.rs` | They configure machinery that does not exist on `full-v2`. See “Semver” below — this is a public API change, and it is safe for exactly one reason |

`enable_json_response`, `http_middleware`, `allowed_origins` and `max_request_bytes` are
era-neutral and present on every build.

### Client

Gated in `src/shared/streamable_http.rs` unless noted:

| Surface | Kind |
|---|---|
| `SendOptions::resumption_token` | public field |
| `StreamableHttpTransportConfig::session_id` | public field |
| `StreamableHttpTransportConfig::on_resumption_token` | public field |
| `StreamableHttpTransportConfigBuilder::with_session_id` | public method |
| `StreamableHttpTransportConfigBuilder::on_resumption_token` | public method |
| `StreamableHttpTransport::session_id()` | public method |
| `StreamableHttpTransport::set_session_id()` | public method |
| The `Last-Event-ID` writer `apply_resumption_header` | private fn, gated |
| The `v1-compat` halves of `resumption_cursor`, `resumption_callback`, `outbound_session`, `capture_session_header`, `terminate_session` and both `debug_v1_fields` | private surface |

The observable consequence: on `full-v2` the client stores no `Mcp-Session-Id`, sends no
`Mcp-Session-Id`, writes no `Last-Event-ID`, and contains **no DELETE construction site at
all**.

### Constants

`src/shared/http_constants.rs` is gated **per constant**, never per module — `MCP_METHOD` and
`MCP_NAME` in that same file are v2-REQUIRED (VERS-05).

| Constant | Disposition |
|---|---|
| `LAST_EVENT_ID` | **GATED**, together with its two readers (the server's replay path, now in the pair, and the client's writer) in one edit |
| `MCP_SESSION_ID` | **UNGATED** — deliberately, and measured. See below |

## What is deliberately NOT severed

These are known limitations, not oversights. Each was measured and each carries its reason in
the source itself.

| Item | Where | Why it is still compiled on `full-v2` |
|---|---|---|
| **`Client::initialize`** | `src/client/mod.rs` | It is DUAL-era, not v1-only: its `is_v2()` branch is a deliberate compatibility affordance that sends nothing and exists so v1-shaped application code keeps compiling after opting into v2. Gating it would delete v2 behaviour. **SMPL-01's “initialize” clause is therefore met on the SERVER side only** |
| **`src/composition/mcp_client.rs`'s handshake** | `src/composition/` | `composition` is in the `full-v2` feature list yet performs a `Client::initialize` handshake unconditionally. Whether `composition` belongs in `full-v2` at all is an open FEATURE-LIST question, raised here rather than silently resolved |
| **`http_constants::MCP_SESSION_ID`** | `src/shared/http_constants.rs` | Two of its readers are on the shared v2 POST path, and one of them also reads `MCP-Protocol-Version`, which v2 requires. The v2 test surface additionally reads the constant precisely to assert the header's ABSENCE — gating it would delete the vocabulary v2 needs to say “no session header was sent”. The name of a header a build refuses to honour is not v1 machinery |
| **`StreamableHttpTransport::last_event_id` field and `last_event_id()` accessor** | `src/shared/streamable_http.rs` | A client-LOCAL SSE cursor written inside two SHARED SSE-parse closures. On `full-v2` nothing reads it and it never reaches the wire — the writer is gone — but gating it would thread `#[cfg]` into shared code. Residual, non-wire-visible resumability state |
| **`start_sse`'s cursor parameter on `full-v2`** | `src/shared/streamable_http.rs` | Present but INERT (`_ignored_cursor`, never read), kept at the same arity so no caller needs a `#[cfg]` |
| **`session_id: Option<String>` threading the server POST pipeline** | `src/server/streamable_http_server.rs` | Kept at the same arity through ~10 functions and always `None` on `full-v2`. Dropping it would mean a call site that no longer has to supply a session id — one edit away from deciding for itself whether sessions apply, which is the second era decision the design forbids |
| **The `EventStore` trait and `InMemoryEventStore`** | `src/server/streamable_http_server.rs` | Public API on both feature sets. What is gated is the config field that used to pin them and every path that reaches them; the type declarations themselves remain nameable |
| **`build_middleware_context`'s read of `MCP_SESSION_ID`** | `src/server/streamable_http_server.rs` | Ungated, and it runs on the middleware POST path for **every** request on **both** feature sets — so a `full-v2` server DOES parse an attacker-supplied `Mcp-Session-Id` and hands it to every registered HTTP middleware as `ServerHttpContext::session_id`. Nothing downstream acts on it (no session map exists to look it up in), but "a `full-v2` build never parses an inbound session id" is true only of the fast path, and this row is why |
| **The server-side `initialize` handler** | `src/server/core.rs` | A `full-v2` server still ANSWERS `initialize`. `v2_verb_rejection` is wired only to GET and DELETE, so an `initialize` POST reaches `Server` core and is dispatched normally: it returns a normal `InitializeResult`, and its `MCP-Protocol-Version` response header carries the negotiated version (pinned by `tests/v2_initialize_negotiated_version_header.rs`). What IS severed is the session BOOKKEEPING around it — nothing is minted, recorded, or echoed. Read the `v1_session_off.rs` prose accordingly: "there is no `initialize`" there means "no session is created by one", not "the endpoint refuses the method" |

## Refused, not unrouted

On `full-v2`, `GET /` and `DELETE /` remain **routed** and answer `405 Method Not Allowed`.
The routes are deliberately *not* removed: an unrouted verb answers `404`, which says “no such
endpoint” rather than “this endpoint does not take this verb”, and that is a different wire
answer. `tests/v2_verbs_405_on_severed_build.rs` asserts the numeric `405` and separately
rejects `404` for exactly this reason.

## Semver: why gating public fields is safe *today*

Removing a public field is normally a MAJOR break. Gating the four
`StreamableHttpServerConfig` fields and the nine client items above is safe for exactly one
reason: the build that lacks them, `full-v2`, is a **brand-new feature that no published
consumer builds with**. Every shipped configuration enables `v1-compat` — it is in `default`
and in `full` — so no existing code loses anything.

**That argument expires the moment `full-v2` enters any published crate's default feature
set.** At that point this becomes a semver break and must be scheduled as one (SMPL-F1, pmcp
3.0). Do not widen `full-v2`'s reach without re-reading this paragraph.

## SSE framing and parsing are SHARED — do NOT gate them

This section exists to stop a well-meaning future contributor from "finishing the job".

**`src/shared/sse_parser.rs` and `src/shared/sse_optimized.rs` are NOT v1-only and must not be
feature-gated.** v2 uses Server-Sent Events too: `subscriptions/listen` is a **v2-only** method
that returns a long-lived `text/event-stream`, framed server-side and parsed client-side by the
same shared code v1 uses.

Only SSE **resumability** — the `Last-Event-ID` request header and the event store that makes
replay possible — is v1-only. Framing and parsing are shared infrastructure. Gating
`sse_parser` behind `v1-compat` would break the v2 subscription path, and the severance build
would catch it as a compile error rather than as a silent regression, but the intent should be
clear before anyone tries.

## The removal condition

Removal of v1 support is tracked as **SMPL-F1** and is:

- **Gated on public-client adoption of v2.** When the client ecosystem has moved, v1 comes out.
- **Landing in a future pmcp 3.0**, as a major-version breaking change.
- **Not scheduled.** There is **no date and no committed window** in this policy, deliberately.
  A dated window would commit the project to a deadline the ecosystem may not meet, and the
  roadmap has always described this as adoption-gated.

Until that condition is met, v1 is a supported path, not a legacy one.

## Three deprecated v1 MECHANISMS (Roots, Sampling, Logging)

**What is deprecated below is the v1 WIRE MECHANISM, not the capability.** Roots, Sampling
and Logging all remain fully functional on 2026-07-28 through their v2 replacements. If you
read this heading and concluded "Sampling is being removed", read the table: nothing is being
removed, and planning a migration away from Sampling would be planning the wrong migration.
What changes is *how* the capability travels on the wire.

| Capability | v1 mechanism (deprecated) | v2 mechanism (the replacement) | Status of the capability on v2 |
|---|---|---|---|
| **Logging** | the `logging/setLevel` RPC | the per-request `_meta` key `io.modelcontextprotocol/logLevel`, which the 2026-07-28 schema describes verbatim as *"Replaces the former `logging/setLevel` RPC"* | **Fully functional.** The level travels on every request instead of being set once for a session |
| **Sampling** | a server→client `sampling/createMessage` request issued mid-call | an `InputRequiredResult` continuation (SEP-2322): the server RETURNS `resultType: "input_required"` with an `inputRequests` entry, the client answers it and resends | **Fully functional.** v2 is stateless and has no server→client request channel mid-call, so the continuation is the spec-blessed shape |
| **Roots** | a server→client `roots/list` request issued mid-call | the same `InputRequiredResult` continuation (SEP-2322), with a roots-kind `inputRequests` entry | **Fully functional.** Identical mechanism to Sampling, applied to Roots |

### The window: 12 months, advisory

The advisory deprecation window for these three mechanisms is **12 months**, anchored to the
2026-07-28 final-spec date, and therefore runs to **2027-07-28**.

**Advisory means exactly this: nothing changes at the end of the window without a separate,
explicit decision.** No mechanism is removed on 2027-07-28. No build breaks on 2027-07-28. No
new output appears on 2027-07-28. The date exists so that a downstream project can plan its
own migration against a stated horizon rather than against silence — it is not a scheduled
removal, and this policy commits to no removal at all (see the reconciliation below).

### This is NOT v1 removal, and NOT capability removal

Three different claims live in this document and they must not be collapsed into one:

1. **Mechanism deprecation** (this section) — three named v1 wire mechanisms whose v2
   replacements have already landed, with a 12-month advisory window.
2. **v1 removal** ([The removal condition](#the-removal-condition), SMPL-F1) — removing MCP
   2025-11-25 support entirely, in a future pmcp 3.0. That is **adoption-gated with no date
   and no committed window**, and this section does not change it. A narrower advisory window
   for three mechanisms is not a schedule for the whole era, and the clause above stands
   exactly as written.
3. **Capability removal** — *not claimed anywhere in this document.* Roots, Sampling and
   Logging are not being removed from MCP or from PMCP in any era.

### What is measured today, not merely intended

The `_meta` replacement mechanism is live and proven (see the verification block below).
The **retirement half is not**: this SDK still SERVES `logging/setLevel` on a 2026-07-28
connection rather than answering `-32601`, which is recorded as gap **G-5** in
`.planning/phases/118-conformance-against-the-official-suite/118-CONFORMANCE-GAPS.md`. That
is stated here rather than omitted, because a deprecation notice that overstates what already
happened is the same defect as a severance claim that overstates what was severed.

### How to verify this section yourself

```bash
cargo test -p pmcp-team-servers --features http --test era_matrix
```

`crates/pmcp-team-servers/tests/era_matrix.rs` drives all three capabilities to COMPLETION
under both eras against one live streamable-HTTP endpoint: under v1 through the v1 mechanisms,
and under v2 through the `_meta` log-level key and two `InputRequiredResult` continuations. It
asserts the tool RESULT and its content, not merely that a response arrived.

`--features http` is **load-bearing**: the whole file is behind
`#![cfg(all(feature = "conformance", feature = "http"))]`.

Four things that can **never** prove this, and why:

- **A passing run with a ZERO test count.** That is exactly what omitting `--features http`
  produces: the file compiles to no tests, the harness prints `running 0 tests`, and cargo
  exits `0`. The test COUNT is part of the evidence — the same trap the severance proofs
  above document, in a different file.
- **A matrix that observed no era difference at all.** Two independent era runs against a
  dual-era server MUST differ; an empty comparison means the v2 arm never ran. `compare_eras`
  surfaces that as a `suspicion`, and `era_matrix_is_conformant` consumes it as a FAILURE
  rather than rendering a clean bill of health.
- **The official conformance suite's v2 scenario set.** It contains no direct Roots or
  Sampling server scenario, so it cannot prove this claim even in principle — passing it says
  nothing either way about these three mechanisms.
- **A fixture under `contracts/team-servers/fixtures/`.** That format expresses a
  `tools_list` and a single `tool_call` and nothing else, so it cannot carry a preceding
  `logging/setLevel`, a host-handler installation, a server→client exchange, or a
  gather-and-resend round trip. The evidence therefore lives in probes and a typed client, and
  the fixture format was deliberately NOT extended to carry it.

## What you have to do today

**Nothing.** `v1-compat` is in `default`. An ordinary `pmcp = "2"` dependency keeps the full
2025-11-25 behavior with no code change, no feature flag, and no build change.

## How to verify severability yourself

```bash
RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2
```

`full-v2` is the `full` feature list minus exactly `v1-compat`. If that command succeeds, the
crate compiles — with the real HTTP/streamable-HTTP transport present — while the v1 layer is
absent. That is the severability guarantee, as a compile-time fact rather than a claim.

Three things that can **never** prove severance, and why:

- **`--all-features`** enables `full-v2` *and* `v1-compat` at the same time. Cargo features are
  additive; they cannot be subtracted. This is also why there is no inverted `v2-only` feature —
  any crate anywhere in the dependency graph enabling a negative feature would silently strip v1
  for every other consumer in the build.
- **Workspace-wide builds** unify features across members, several of which depend on `pmcp`
  with `full`. The `-p pmcp` scope is load-bearing.
- **`--no-default-features` alone** strips `http` and `streamable-http` too, because
  `default = ["logging"]`. It would "prove" severability by never compiling the transport that
  holds the code being severed. Hence the parallel `full-v2` list.

`RUSTFLAGS="-D warnings"` is also load-bearing: without it, code stranded by the cut emits a
`dead_code` warning and the build still reports success.

`full` and `full-v2` are two enumerated lists and can drift, so
`tests/v1_severability_tripwire.rs` derives both from `Cargo.toml` at test time and fails if
they differ by anything other than `v1-compat`.

### The aggregate command

```bash
cargo test -p pmcp --no-default-features --features full-v2
```

Same three fences, applied to the TEST build: it compiles every test target and every example
under `full-v2`. This is the command a developer naturally reaches for, and it was a hard
BUILD failure until Phase 117's fix pass — the severed configuration had no working test build
at all. It now reports a non-zero passing count.

### A compile is not a runtime answer

The build above proves what the crate CONTAINS. It proves nothing about what the server
ANSWERS. Three test files close that gap by RUNNING on the severed build:

```bash
cargo test --test v2_verbs_405_on_severed_build --no-default-features --features full-v2
cargo test --test v2_client_carries_no_session_on_severed_build --no-default-features --features full-v2
cargo test --test v2_initialize_negotiated_version_header --no-default-features --features full-v2
```

Or all of them plus the aggregate, with the zero-count guard applied:

```bash
make test-severance          # -> ./scripts/run-severance-proofs.sh
```

Both files are gated by a file-level `#![cfg(..., not(feature = "v1-compat"), ...)]`. That is a
`cfg` PREDICATE inside pmcp's own test compilation, not a negative cargo feature — it selects
whether the binary contains any tests and no dependency graph can observe it.

**The corollary is the trap to watch for.** On a build that carries `v1-compat` these files
compile to ZERO tests and `cargo test` still exits `0`. A run reporting `0 tests` is a
*failure* of the proof, not a pass, so the test COUNT is part of the evidence. This is not
hypothetical: a dev-dependency of `pmcp` was taking `pmcp`'s default features and unifying
`v1-compat` back on for every `cargo test`, which made a severed-build test silently vacuous
while the build proof stayed green. `cargo build -p pmcp` never sees dev-dependencies;
`cargo test` does.

That criterion is **enforced**, not merely stated: `scripts/run-severance-proofs.sh` greps
each harness run for `running N tests` with N >= 1 and fails otherwise, and CI runs the script
from the `v1-severance` job, which is in `gate.needs` and therefore blocks merge. It cannot be
enforced from inside a proof file — a test in a `#![cfg]`-selected file can never observe its
own absence, which is why the earlier `assert!(!cfg!(feature = "v1-compat"))` attempt was a
tautology (`cfg!` expands to a bool literal, so the assertion was `!false`) and was deleted.
`tests/ci_severance_gate_wiring.rs` pins the script's contents and its wiring into the
blocking gate.

## Explicit non-commitments

This policy deliberately does **not** do any of the following, and adding them later would be a
change to this policy rather than an implementation detail:

- **No `#[deprecated]` attributes.** A `#[deprecated]` marker would emit compiler warnings at
  effectively every current user of a path that is still fully supported, and would force
  `allow()` suppressions throughout the SDK's own source.
- **No runtime warning on v1 negotiation.** Logging a warning when a client negotiates v1 would
  change v1 runtime behavior, which is exactly what the next point forbids.
- **No runtime warning on a still-supported MECHANISM.** The three deprecated mechanisms above
  emit nothing: no warning, no notice, no new field. A warn on a still-supported mechanism
  trains users to ignore warnings, and this one would fire on every affected call for a full
  year — by the end of which it would carry no information at all. Deprecation here is
  documented, never emitted. There is also a maintenance cost on the other side of the
  keyboard: a warning nobody wants to regress has to be pinned by a warn-capture assertion,
  and this repository has already measured what those cost — the intermittent
  `oauth_store_wiring` DCR issuer-change test (Phase 116 surface, CI record 3 pass / 2 fail)
  is an open flake in precisely that class. Adding a signal here would mean adding another.
- **No behavior change on the wire — *on a `v1-compat` build*.** v1 request/response bytes stay
  identical there; feature-gating moves where code lives, it does not change what a v1 client
  observes. `tests/v1_byte_identity_after_cut.rs` pins nine of those responses byte-for-byte.

  The qualifier is load-bearing and was missing until Phase 117's code review. A **`full-v2`**
  build necessarily answers a v1 client differently — that is the severance, not a defect —
  and the differences are enumerated above: `GET` and `DELETE` become `405`, no
  `Mcp-Session-Id` is minted or echoed, no `Last-Event-ID` is honoured, and an `initialize`
  POST is served statelessly rather than refused. This bullet is a promise about the DEFAULT
  build every published consumer gets, not about the severed one.

## Scope of this document

This is the **normative** policy: what `v1-compat` is, what removal is conditioned on, and what a
consumer must do. It is intentionally short and precise.

The **narrative** v2 migration guide — how to opt into v2, the dual-version story, and the Tasks
extension migration — is tracked separately as DOCS-05 and links back here as the authority. It
does not restate or override anything above.
