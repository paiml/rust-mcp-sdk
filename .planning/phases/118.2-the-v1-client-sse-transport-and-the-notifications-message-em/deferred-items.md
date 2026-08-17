# Phase 118.2 — deferred items

Out-of-scope discoveries logged rather than fixed, per the executor scope boundary
(only issues DIRECTLY caused by the current task's changes are auto-fixed).

## Pre-existing `cargo fmt --all -- --check` violations (found during 118.2-01)

`make quality-gate`'s `fmt-check` step is `cargo fmt --all -- --check`, which is
workspace-wide. At the branch tip **before** 118.2-01's first commit (`e08cdb4c`)
it already reported seven diffs in four files that 118.2-01 does not touch:

| File | Introduced by |
|------|---------------|
| `cargo-pmcp/src/commands/configure/add.rs:84` | `c339293d` (`fix(pmcp-run): resolve the GraphQL endpoint when the discovery cache is cold`) |
| `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:1012` | `c339293d` |
| `cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs:2, :418, :1830, :1838` | `c339293d` |
| `src/server/ui.rs:295` | earlier than `c339293d` |

Verified pre-existing: none of these paths appears in
`git diff --name-only e08cdb4c~1..HEAD`, and every file 118.2-01 DID touch is
`rustfmt`-clean.

**Disposition:** a one-line `cargo fmt --all` fixes all seven, but it would put
four unrelated files into this plan's commits. Whoever next touches `cargo-pmcp`
(or a dedicated formatting commit) should clear them — until then
`make quality-gate` fails at `fmt-check` for a reason unrelated to this phase.

## Withheld `log_info` / `log_warn` / `log_error` severity wrappers (118.2-05, Task 2)

The reviewed draft of plan 05 shipped five emitter methods. Per the cross-AI
review (MEDIUM finding, ACCEPTED) plan 05 ships exactly two —
`RequestHandlerExtra::log` and `RequestHandlerExtra::log_with_data`.

**Withheld:** `log_info(msg)`, `log_warn(msg)`, `log_error(msg)`.

**Why:** they are pure sugar over `log(level, message)`. Under CLAUDE.md every
public method carries doc, doctest, contract and property cost, and under this
repo's semver policy it is permanent surface. `log(level, message)` plus one
structured overload satisfies CONF-10 with strictly less exposure.

**Disposition:** additive at any later time with no breaking change. Add them
when a real call site shows the sugar earns its keep, not before.

## `RequestHandlerExtra::log` has no "sink attached but misplumbed" diagnostic (118.2-05, D-08)

`log(..)` with no sink returns `Ok(())` and emits nothing (D-08), which is what
keeps a handler callable outside a server. The accepted cost: a MISPLUMBED
transport is indistinguishable from a quiet handler at the call site.

**Candidate follow-up:** a `tracing::warn!`-once variant on the first emission
attempt with no sink, so the silence is visible in server logs without changing
the `Ok(())` contract or breaking `RequestHandlerExtra::default()` usage in unit
tests. Not in scope for 118.2-05.

## `LogMessageParams` diverges from the vendored schema: `message` vs `data` (118.2-05, Task 3)

Found while deriving the wire fence from the spec rather than from pmcp's own
serializer (118.1 D-04). `schema/vendored/core-2026-07-28/schema.ts:2031`
declares:

```ts
export interface LoggingMessageNotificationParams extends NotificationParams {
  level: LoggingLevel;
  logger?: string;
  data: unknown;      // REQUIRED — and there is no `message` member
}
```

pmcp's `LogMessageParams` (`src/types/notifications.rs:161`) instead carries a
REQUIRED `message: String` and an OPTIONAL `data: Option<Value>`, skipped when
`None`. So a plain `extra.log(level, "text")` emits
`{"level":"warning","message":"text"}` — a payload with no `data` member at all,
where the spec marks `data` required, plus a `message` member the spec does not
define.

**Not fixed here, deliberately.** Changing the serde shape of `LogMessageParams`
is a breaking change to a public type well outside plan 05's
`files_modified`, and picking the replacement shape (does `message` become
`data`? does `log(..)` set `data` to a JSON string?) is a design decision, not a
bug fix.

**Pinned, not hidden:** `a_log_record_serializes_as_the_spec_notifications_message_shape`
in `tests/log_emitter.rs` asserts the shape pmcp emits TODAY against literals, so
the divergence is a visible recorded fact and any future change to it is a
deliberate, reviewed edit rather than a silent drift.

**Owner:** plan 08 (the CONF-10 conformance fence) is where this must be
confronted — if the official suite validates `params.data`, the current shape
fails it and the type change becomes in-scope work with its own semver verdict.

## Pre-existing D-19 plan-lint violations block `make quality-gate` at step 1 (found during 118.2-05)

`make quality-gate`'s FIRST step is `lint-plans`
(`./scripts/lint-plan-verify-commands.sh`), which runs before `fmt-check`. At the
branch tip it reported **8** RULE 1 violations — a build/test invocation piped
into `tail` with no `pipefail`, so the pipeline reports the tail's status and a
FAILING build reads as PASS. All 8 live in 118.2 PLAN.md files authored at
`cb5d1365`, i.e. before any 118.2 execution commit:

| Plan file | Line |
|-----------|------|
| `118.2-04-PLAN.md` | 468 |
| `118.2-05-PLAN.md` | 277 | **fixed by 118.2-05** |
| `118.2-06-PLAN.md` | 256 |
| `118.2-07-PLAN.md` | 216 |
| `118.2-07-PLAN.md` | 307 |
| `118.2-08-PLAN.md` | 157 |
| `118.2-09-PLAN.md` | 174 |
| `118.2-09-PLAN.md` | 228 |

118.2-05 fixed only its OWN line (277), rewriting it as two
`bash -o pipefail -c '... | tee <log>'` invocations. The remaining **7** belong
to plans 04, 06, 07, 08 and 09 and are out of 118.2-05's scope.

**Disposition:** each plan's executor should fix its own line as its first act,
or a single sweep should fix all 7. Until then `make quality-gate` fails at
`lint-plans` for a reason unrelated to any source change — a SECOND pre-existing
blocker stacked on top of the `fmt-check` one recorded above.

## `make audit` fails on pre-existing dependency advisories (found during 118.2-03)

`make quality-gate`'s `audit` step (`cargo audit`) exits 2 at the branch tip on
advisories in third-party crates. One VULNERABILITY plus six unmaintained /
unsound warnings:

| Crate | ID | Class |
|-------|----|-------|
| `webbrowser` | RUSTSEC-2026-0257 | vulnerability |
| `paste` | RUSTSEC-2024-0436 | unmaintained |
| `smartstring` | RUSTSEC-2026-0249 | unmaintained |
| `anyhow` | RUSTSEC-2026-0190 | unsound |
| `event-listener` | RUSTSEC-2026-0221 | unsound |
| `lru` | RUSTSEC-2026-0253 | unsound |
| `rand` | RUSTSEC-2026-0097 | unsound |
| `scc` | RUSTSEC-2026-0205 | unsound |

**Provenance:** plan 118.2-03 changed four `.rs` files and NO manifest —
`git status --short -- Cargo.toml Cargo.lock` is empty across all of its commits
— so none of these can have been introduced by it. They are a dependency-tree
fact of the branch tip, and closing them means bumping or dropping third-party
crates.

**Disposition:** out of scope for phase 118.2, which touches no dependency. It is
a THIRD pre-existing blocker on `make quality-gate`, stacked on the `fmt-check`
and (now-cleared) `lint-plans` ones above. Needs its own dependency-hygiene pass.

## `make lint` fails on a `clippy::let_underscore_future` in `src/shared/streamable_http.rs` (found during 118.2-06)

`make lint` (and therefore `make quality-gate`) exits 101 at the branch tip:

```
error: non-binding `let` on a future
    --> src/shared/streamable_http.rs:1718:13
     |
1718 |             let _ = self.spawn_sse_reader(response.into_body());
     = note: `-D clippy::let-underscore-future` implied by `-D clippy::all`
```

**Provenance:** authored by plan **118.2-03** at commit `8b19602d`
("read the POST-response SSE body incrementally, retire feed_complete_body").
`git status --short -- src/shared/streamable_http.rs` is empty during 118.2-06,
so this plan did not touch the file.

**It is a LINT issue, not a functional one.** `spawn_sse_reader` returns a
`tokio::task::JoinHandle`, and dropping a `JoinHandle` detaches the task rather
than cancelling it — which is exactly the "DETACHED rather than stored in
`self.abort_handle`" behaviour the surrounding comment describes. The task does
run. Clippy fires only because `JoinHandle` happens to implement `Future`.

**The fix is one token:** `drop(self.spawn_sse_reader(response.into_body()));`
in place of the `let _ =`. Left to 118.2-03's owner rather than swept into this
plan's commits, per the executor scope boundary.

**Disposition:** a FOURTH pre-existing blocker on `make quality-gate`, stacked on
`fmt-check` and `audit` above. Unlike those two it is trivially closable and it
gates every remaining plan in the phase, so it should be swept first.

**Verified during 118.2-06:** re-running the exact `make lint` clippy invocation
with `-A clippy::let_underscore_future` appended exits **0** with zero warnings,
so this is the ONLY lint blocker on the branch and plan 06's own surface is clean.

## An `extra.log(..)` EXAMPLE is deferred to plan 08 (recorded during 118.2-06)

CLAUDE.md's ALWAYS-requirements ask every new feature for a working
`cargo run --example`. Plan 06 wires the sink; plan 07 wires the LEVEL source.
An example shipped now would demonstrate a half-wired feature — records emit,
but the level a client asked for is not yet honoured, so what the example
"shows" would change under the reader's feet one plan later.

**Owner: plan 08**, alongside the CONF-10 conformance fence, which is the first
point at which a client-visible end-to-end story is true.

### Executor note: a `make` target auto-formats `src/server/ui.rs`

During 118.2-06, running the in-scope gate steps individually
(`make build / test-unit / test-doc / test-integration / check-todos /
check-unwraps / purity-check / comply`) left `src/server/ui.rs` MODIFIED with
exactly the `fmt-check` diff recorded above — i.e. one of those targets shells
out to `cargo fmt --all` rather than `--check`. It was reverted with
`git checkout -- src/server/ui.rs` so the pre-existing state is preserved and the
fix is not partially smuggled in (the three `cargo-pmcp` diffs are untouched by
the root formatter, so committing only `ui.rs` would half-close the blocker).

A later executor running the same targets will see the same phantom
modification. It is not their change.

## A malformed v1 `logging/setLevel` is rejected `400` / `-32601`, not answered `{}` (recorded during 118.2-07)

**MEASURED** by `a_malformed_level_value_is_ignored_and_not_echoed` in
`tests/log_emitter.rs`, which now asserts the observed behaviour rather than the
behaviour plan 07 predicted.

Plan 07's Task 2 states that a malformed `logging/setLevel` `params.level`
"stores-or-ignores and still answers `{}`". The v2 `_meta` half of that claim
HOLDS and is fenced: a bogus `io.modelcontextprotocol/logLevel` value yields a
normal `200`, the `info` default applies, and the peer's bytes appear nowhere in
the response.

The v1 RPC half does not, for a reason that predates this plan: a
`logging/setLevel` whose `level` is not one of the eight `LoggingLevel` spellings
fails TYPED PARSING inside `parse_transport_message_fast`, so the whole message
never becomes a `ClientRequest` and the transport answers

```
400  {"error":{"code":-32700,"message":"Invalid JSON: … -32601 - Method not found: logging/setLevel"}}
```

long before plan 07's ingress capture runs. Making it answer `{}` means changing
the deserialization of the PUBLIC `ClientRequest::SetLoggingLevel` variant (e.g.
tolerating an unknown level string), which is out of this plan's `files_modified`
and carries its own semver verdict.

**What plan 07 DOES claim of that path holds and is fenced:** no panic, the
peer's value is never echoed into the rejection (`T-118.2-07-04`), and nothing is
stored — a later tool call on the same session is still filtering at the `info`
default.

**Owner:** unassigned. Natural home is plan 08, which already owns the
`LogMessageParams` `message`-vs-`data` spec divergence and the `{}`-response pin
(Pitfall 8).

## `set_session_log_level`'s no-op-for-unknown-id is not wire-reachable (recorded during 118.2-07)

`v1::set_session_log_level` is a NO-OP for an unknown session id rather than an
insert, which is the T-118.2-07-02 denial-of-service control. On the wire that
control is currently UNREACHABLE: `v1::validate_non_init_session` answers `404
Unknown session ID` before the ingress capture runs, so a caller cannot get as
far as the write with an id the server never issued.

`a_set_level_for_an_unknown_session_id_inserts_no_session` therefore fences the
END-TO-END property (no row is minted; the id is still unknown on the next
request) rather than the no-op itself. The no-op remains as defence in depth for
a future call site that reaches the write without the validation — the case the
rustdoc names — and a mutation of it does NOT turn any fence red today.

## VERDICT (118.2-08): the `LogMessageParams` `message`-vs-`data` divergence stays DECLARED

Plan 05 recorded the divergence and named plan 08 as its owner, with an explicit
trigger: *"if the official suite validates `params.data`, the current shape fails
it and the type change becomes in-scope work with its own semver verdict."*

**That trigger has NOT fired. Measured, not assumed.**

The pinned suite does bundle the schema — its `LoggingMessageNotification`
definition carries `required: ['data','level']`, and the vendored
`schema/vendored/core-2026-07-28/schema.ts:2031` says the same — but **no
scenario in the suite validates the params of an emitted
`notifications/message`**:

| Scenario | What it inspects |
|---|---|
| `2025-11-25:logging-set-level` | ONLY the `logging/setLevel` RPC's response (`Object.keys(r).length > 0`). It never reads a notification. |
| `sep-2575-server-no-log-without-loglevel` | NEGATIVE: asserts NO `notifications/message` frame is emitted for a request that did not set `_meta["io.modelcontextprotocol/logLevel"]`. It looks for absence, and reports `untestable` when the server does not expose the diagnostic tool `test_logging_tool`. |

Those are the only two logging scenarios in
`conformance/node_modules/@modelcontextprotocol/conformance/dist/index.js`. So
pmcp's `{"level":"warning","message":"hello"}` costs **zero conformance points
today**.

**Against changing it now:**

1. It is a **breaking change to a public type** (`LogMessageParams`'s serde
   shape). pmcp is at 2.18.0 with no open breaking-change window; the v2.0
   cleanup philosophy this repo records applies to a breaking-change WINDOW, and
   there is not one.
2. Choosing the replacement is a **design decision**, not a bug fix: does
   `message` become `data`? does `log(..)` set `data` to a JSON string and keep
   `message` as an extension (legal — the schema does not close
   `additionalProperties`)? does `log_with_data` become the only shape?
3. `src/types/notifications.rs` is outside 118.2-08's `files_modified`, and the
   arm this plan changed has nothing to do with the notification payload.

**Disposition:** DECLARED, and now MECHANIZED.
`the_vendored_schema_requires_data_where_pmcp_emits_message` in
`tests/log_emitter.rs` reads the in-repo vendored schema and the live emitted
payload and asserts BOTH sides of the disagreement, so the day either side moves
it is a red test rather than a note in a summary nobody re-reads. The real fix
belongs to a dedicated semver phase alongside the next major bump.

## VERDICT (118.2-08): the malformed-`logging/setLevel` `400`/`-32601` finding stays OPEN, not fixed

Recorded during 118.2-07 (above) with plan 08 named as its natural owner.
Reviewed here; **not fixed**, for the same reason as the verdict above.

Making a malformed `params.level` answer `{}` means loosening the
deserialization of the PUBLIC `ClientRequest::SetLoggingLevel` variant — e.g.
tolerating an unknown level string — which changes what every pmcp CLIENT and
every third-party consumer of that type accepts. That is a semver-relevant
change to a public type, and it is a *strictly more permissive* parse of
attacker-supplied input, which is not a change to make in passing: today a
misspelled level is refused with the peer's bytes never echoed
(`T-118.2-07-04`), which is a defensible answer.

It is also **not** a conformance gap: the suite's `logging-set-level` scenario
sends `{ level: 'info' }`, a well-formed value, and that path answers a literal
`{}` — pinned over the wire by
`v1_set_logging_level_answers_a_literal_empty_object` (both ingress paths) and
in-process by `the_v1_answer_is_an_object_with_zero_keys_on_both_roots`.

**Disposition:** left OPEN in `.planning/WINDOWS.md` deliberately. A human at
ship time should see it, because it IS a divergence between a plan's stated
contract and the code — it is just not one this phase should close.

## `set_session_log_level`'s unreachable no-op — reviewed by 118.2-08, unchanged

Recorded during 118.2-07 with plan 08 as owner. Reviewed; nothing to do.

The no-op is defence in depth for a call site that reaches the write without
`validate_non_init_session`. Removing it because no fence can red it today would
delete a control for the exact reason it is currently working — and 118.2-08
touches neither the session store nor the ingress. Left as-is, with 118.2-07's
end-to-end fence continuing to pin the property that IS wire-observable.

## FINDING (118.2-09): on v2, `DEFAULT_LOG_LEVEL` makes pmcp violate SEP-2575

**Measured, not inferred.** With the fixture's guard removed, a v2 `tools/call`
that carried NO `params._meta["io.modelcontextprotocol/logLevel"]` came back with
a `notifications/message` frame on the POST body — recorded as RED mutation 2 in
`118.2-09-SUMMARY.md`, artifact `target/118.2-09-log-records.json`.

The vendored schema is explicit: *"If absent, the server MUST NOT send any
`notifications/message`."* pmcp's `resolve_request_log_level`
(`src/server/streamable_http_server.rs:1677`) returns `None` in that case, and its
own rustdoc states the intent — *"`None` is not 'no logging' — it is 'nothing
overrode the default'"* — so `DEFAULT_LOG_LEVEL` (`info`, D-12) applies at emit
time and every `extra.log(Info, ..)` in the handler reaches the client.

That default is **correct on v1** (2025-06-18: "the server MAY decide which
messages to send automatically") and **wrong on v2**, where absence is a
prohibition rather than a non-answer. The two eras disagree about what "nobody
asked" means, and one constant currently answers for both.

**Not fixed here.** This plan's `files_modified` are two examples and
`Cargo.toml`; the fix is an era branch inside `resolve_request_log_level` (or a
`None`-means-silent rule applied only when `era == Some(V2)`) — a behaviour
change in `src/` that alters what every v2 pmcp server puts on the wire.

**Disposition:** the FIXTURE honours SEP-2575 by emitting from `test_logging_tool`
only when `extra.log_level.is_some()`, with the gap named in a comment at the
guard so the next reader cannot mistake the guard for the fix. Appended to
`.planning/WINDOWS.md` so it is visible at ship time. Natural owner: the phase
that measures the suite (118.2-11) or a follow-on `src/` plan.

## DEFERRED (118.2-10): there is no `Client`-level notification observation API

**Measured, not inferred.** `Client::notification_tx: Option<mpsc::Sender<Notification>>`
is initialised to `None` at ALL THREE construction sites — `src/client/mod.rs:406`,
`:453` and `:496` — `ClientBuilder` has no field for it and no setter, and the only
other reference in the file is the `Clone` impl at `:5263`. The forwarding branch
inside `dispatch_request`'s wait loop (`:3764`) is therefore permanently dead: a
`notifications/message` that reaches a `pmcp::Client` while one of its own requests is
outstanding is run through the middleware chain and then **dropped on the floor**.

**Consequence for a consumer.** Phase 118.2 gave the SDK a complete server-to-client
log channel — the handler emits (`extra.log`, plan 05), the sink is attached at both
dispatch roots (plan 06), the level is resolved per era (plan 07), the record is framed
onto the v1 session stream, and pmcp's own transport now READS that stream live
(plans 01-04). `tests/pmcp_both_ends_logging.rs` proves the whole path end to end. But
an application built on `pmcp::Client` still cannot SEE those records without dropping
to `Transport::receive()` itself, which means giving up the `Client` API entirely.

**Why 118.2-10 did not build it.** This plan's `files_modified` are two test files, and
the API is genuinely additive NEW SCOPE the phase did not plan for: it needs a design
decision (a builder callback `ClientBuilder::on_notification(..)` versus a
`Client::subscribe_notifications() -> Receiver<Notification>` versus both), a semver
verdict, and its own CLAUDE.md ALWAYS-requirement package (fuzz / property / unit /
example). Folding it in under the joint fence would have been a new public surface
added under time pressure — exactly T-118.2-10-04.

**Disposition:** the joint fence asserts at the TRANSPORT layer instead, which satisfies
D-15.3's "pmcp on both ends" literally and adds zero public API, so the phase's
`cargo semver-checks` verdict stays clean. This entry is the record that the DX gap is
real and known. Natural owner: a follow-on client-DX plan, not a conformance phase.

## FINDING (118.2-10): a pmcp CLIENT cannot ANSWER a server-to-client request issued during its own call

**Measured with a scratch probe during 118.2-10, and it moves the residual that
`crates/pmcp-team-servers/tests/era_matrix.rs` has been recording.**

The probe drove a real `StreamableHttpTransport` against a real `StreamableHttpServer`
whose tool reaches for `extra.peer().sample(..)`, issuing the `tools/call` POST on one
transport clone and draining `Transport::receive()` on another. What the client's own
queue saw, verbatim:

```
PROBE frame 0: Request { id: String("dispatch-1"), request: Client(CreateMessage(CreateMessageParams { .. })) }
PROBE frame 1: TIMEOUT
```

So **delivery is fixed**: the server's `sampling/createMessage` request reaches pmcp's
own client over the live v1 session stream. What still fails is the ANSWER, and the
reason is a lifecycle deadlock rather than a missing stream:

* the server holds the `tools/call` POST open for the whole duration of the handler,
  answering `202` only after it returns;
* `Client::dispatch_request` (`src/client/mod.rs`) awaits `transport.send(..)` to
  COMPLETE before it enters the receive loop that would dispatch the inbound request;
* so the client is parked inside `send()`, cannot answer, and the server's peer request
  expires — `Protocol error: -32001 - Server request dispatch-1 timed out`.

Measured in the first probe run: `send within bound: Elapsed(())` at a 10 s bound, on a
call whose handler was waiting for the client.

**Not fixed here, and not fixable inside this plan's `files_modified`.** Both candidate
fixes are `src/` behaviour changes to a core path with their own ALWAYS-requirement
package and semver verdict: either `Client::dispatch_request` must overlap its outbound
send with its receive loop, or the server must answer the request POST `202` before
running the handler (detached dispatch). Choosing between them is a design decision, not
a bug fix, which is deviation Rule 4 territory.

**Disposition:** `deprecated_capabilities_complete_under_both_eras` therefore keeps
asserting `no-live-stream` — the value `era_target::undelivered()` reports for ANY peer
error — but 118.2-10 STRENGTHENED it to also pin the `detail`, so the fence now records
WHICH hop is missing instead of implying the stream is dead. The module doc records the
fixed state, the pre-fix state, and this residual. Appended to `.planning/WINDOWS.md`.
Natural owner: a follow-on client-lifecycle plan.

---

## `LogMessageParams` emits `message`, the specification requires `data` (118.2-11)

**Found:** 2026-08-17, plan `118.2-11` Task 1, by re-measuring the official suite at the
held `0.2.0-alpha.11` pin. **Not a discovery from reading — the referee said it.**

`src/types/notifications.rs:161-172` declares:

```rust
pub struct LogMessageParams {
    pub level: LoggingLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logger: Option<String>,
    pub message: String,                 // NOT in the specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,             // REQUIRED by the specification
}
```

`schema/vendored/core-2026-07-28/schema.ts:2031-2044` declares `level`, optional `logger`
and `data: unknown` — required, with **no `message` member at all**. The suite validated
against the `2025-11-25` schema and reported the same requirement, so the divergence is
era-independent.

`extra.log(..)` (`src/server/cancellation.rs:822-825`) passes `None` for `data`, so the
ergonomic emitter — the one plan 118.2-09's fixture and every future pmcp user calls —
puts a schema-invalid frame on the wire.

**Two independent checks fail on that one divergence, both measured:**

1. `WireSchemaValid` rejects all three frames:
   `LoggingMessageNotification/params: must have required property 'data'`.
2. `ToolsCallWithLogging` reports `logCount: 0`. The frames reach the wire — check 1 saw
   them — but the official reference client's zod schema has `data: z.unknown()`, which
   under the bundled zod v4 is **non-optional**, so the client DROPS every record.
   Reproduced against the pinned bundle: without `data` `parse ok: false`, with `data`
   `parse ok: true`.

Net suite effect: `2025-11-25:tools-call-with-logging` **1/1 → 0/2**, the v1 leg **72/2 →
71/3**, `GAP_ATTRIBUTABLE_FAILURES` **1 → 2**. The v2 leg is byte-identical to baseline.

**Not fixed here, and Rule 4 rather than Rule 1.** It is a wire-format change to a public
type — the same class as G-1, which `118-CONFORMANCE-GAPS.md` already defers for exactly
that reason — and this plan's declared `files_modified` are a shell script and two
planning documents. Three candidate fixes, each with a different compatibility story:

| Candidate | Shape | Cost |
|---|---|---|
| A | `emit_log_record` defaults `data` to the message when `None` | No Rust API change at all; changes emitted bytes; every existing `extra.log` caller becomes conformant. `message` stays as a pmcp extension |
| B | Serialize `message` INTO `data` as `{"message": …}` | Same as A but a structured payload; a consumer reading `data` as a string sees an object |
| C | Drop `message` from `LogMessageParams` in favour of `data` | Breaking Rust API change; needs a semver decision |

Choosing between them — and deciding what existing pmcp clients that read `message` should
see — is a design decision, not a bug fix.

**Also measured while checking:** at this pin the `2026-07-28` leg runs **no**
`tools-call-with-logging` and **no** `sep-2575-*` scenario, so the open SEP-2575 v2 default
finding is not observable by the official suite and stays an in-tree unmet truth.

**Disposition:** G-3 sub-item (d) stays **OPEN** with its cause fully localised. The gate
was NOT hardened and CONF-09 was NOT booked, per the plan's own STOP instruction and D-21.
Appended to `.planning/WINDOWS.md`. Natural owner: a follow-on plan that owns `src/`.

---

## RESOLVED (118.2-13): the `message`-vs-`data` divergence, by Option A

**Resolved:** 2026-08-17, plan `118.2-13`. This closes the two entries above — the
118.2-08 VERDICT ("DECLARED, not fixed") and the 118.2-11 measurement that refuted its
premise. It does NOT close the SEP-2575 v2 default or the client-lifecycle deadlock, both
of which remain open above.

**Option A, taken by the user at plan 118.2-11's blocking-human checkpoint.**
`emit_log_record` (`src/server/cancellation.rs`) now populates `data` with the message
string when the caller supplied none:

```rust
let data = data.unwrap_or_else(|| serde_json::Value::String(message.clone()));
let params = crate::types::LogMessageParams::new(level, message).with_data(data);
```

* **`log_with_data(..)` is untouched** — an explicitly supplied value moves through
  verbatim and is never overwritten. The clone lives in the `None` arm only.
* **Both early returns stay ahead of it** — a below-bar record and a no-sink record still
  construct no payload at all.
* **No Rust API change.** `cargo semver-checks --baseline-rev cb5d1365 -p pmcp` reports
  `no semver update required`; the diff on `src/types/notifications.rs` is 33 lines, all
  `///`. That is the whole reason Option A was chosen over C: it buys conformance with no
  semver event.
* **`message` stays on the wire** as a pmcp extension. The schema does not close
  `additionalProperties` and the reference client strips unknown members; the pinned
  bundle parses the `data`-bearing frame `ok: true` with `message` also present.

**Options B, C and D were rejected at the checkpoint.** D — changing only the conformance
fixture — was rejected as gaming the referee: it turns the suite green while every real
`extra.log` caller keeps emitting non-conformant frames.

**Correcting the record.** `118.2-08` concluded "no suite scenario validates an emitted
notification's params" and DECLARED the divergence on that basis. The premise is FALSE:
`WireSchemaValid` is not a scenario, it is a check that runs inside scenarios over every
frame the implementation sends. Any in-tree comment or summary still saying "do not change
`LogMessageParams`" is stale guidance written on a falsified premise.

**What this plan does NOT claim.** The suite SCORE. Re-measuring
`2025-11-25:tools-call-with-logging` (expected 0/2 → 1/1) and `WireSchemaValid` belongs to
plan `118.2-11` when it resumes its tasks 2-3. `WINDOWS.md` entry 8 therefore stays OPEN
for that measurement to close; entry 4 (the in-tree divergence) is marked `fixed`.

**Fences:** `tests/log_emitter.rs` —
`a_plain_log_emits_the_required_data_member_carrying_the_message`,
`an_explicitly_supplied_data_value_survives_verbatim`,
`a_below_bar_record_never_reaches_the_sink_and_builds_no_payload`,
`the_emitted_frame_satisfies_the_vendored_schemas_required_data_member` (the rewritten
118.2-08 fence, still reading the in-repo vendored schema), and
`property_every_delivered_log_frame_carries_a_data_member`.

---

## Deferred by `118.2-11` (2026-08-17) — out of scope, not fixed

**`RUSTSEC-2026-0257`, `webbrowser` — resolved as a stale LOCAL lockfile, not deferred.**
`make quality-gate`'s `audit` stage reported one vulnerability: `webbrowser 1.2.0`, "Unix
`BROWSER` handling allows browser argument injection", advisory dated 2026-07-29, solution
"upgrade to >= 1.2.2". It reached a direct `pmcp` dependency (`webbrowser = { version = "1",
optional = true }`, `Cargo.toml:138`, behind the `oauth` feature).

Recorded here because the diagnosis is worth not repeating: **`Cargo.lock` is gitignored in
this repo** (`.gitignore:3`), and the declared constraint `"1"` already admits `1.2.2`. So
this was never a repository defect and never affected CI, which resolves fresh on every run —
it was one stale local lockfile entry. `cargo update -p webbrowser` moved `1.2.0 -> 1.2.4`
(pulling `dispatch2 0.3.1` and `objc2-app-kit 0.3.2`), `cargo audit` went clean, and **no
committed file changed**. Nothing to defer, and no dependency decision was taken on anyone's
behalf.

**The 7 `cargo audit` allowed-warnings are untouched and remain out of scope**, including
`RUSTSEC-2024-0436` (`paste 1.0.15` unmaintained, via `umya-spreadsheet 3.0.0` ->
`pmcp-workbook-compiler`). They are pre-existing, allowed by the repo's audit configuration,
and unrelated to this plan.
