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

## DEFERRED (118.2-15): per-id response ROUTING, and the cost of discard-on-mismatch

**Recorded by plan 15 (CR-02), 2026-08-17. Explicitly not this closure's work.**

CR-02's second half is now closed the way the gap-closure scope fenced it:
`Client::dispatch_request` compares the popped response's `id` against the `request_id` it is
awaiting, and on a mismatch it logs a `tracing::warn!` naming BOTH ids and **keeps looping**.
Nothing is removed from `active_requests` on that path — the request is still pending — so the
WR-04 single-exit cleanup remains the only other place the entry is removed.

**The stated limitation.** A discarded frame is *gone*. If it was the answer to some OTHER
in-flight request on the same `Client`, that request now waits out its caller's timeout instead
of receiving its answer. That is strictly better than the pre-fix behaviour — one caller
silently receiving another caller's tool result is a cross-request data leak inside one process
(T-118.2-15-01) — and it is bounded by the caller's own timeout rather than unbounded. But it
is a real cost, accepted deliberately and recorded as `T-118.2-15-03` (Denial of Service,
medium, **accept**) rather than papered over.

Two things were considered and rejected *for this plan*:

- **Re-queueing the mismatched frame.** There is nowhere to put it back: a consumer of
  `Transport` holds no producer handle to the receive queue. Handing one out to `Client` would
  be a new public affordance minted to work around a routing gap.
- **An orphan-response buffer on `Client`.** A holding pen fixes the symptom while leaving one
  consumer loop deciding for every caller, and it introduces its own unbounded-growth question
  (how long does an orphan live, and who evicts it?).

**The correct long-term shape — the actual follow-up.** Per-id response ROUTING: the
`active_requests` map carries a response channel per id, so a popped frame is *delivered to its
owner* instead of being discarded, and no request can be starved by a frame that arrived while
a different request happened to hold the consumer loop. That is a redesign of how `Client`
consumes its transport, not a comparison, and it belongs in its own plan with its own
concurrency fences. It is **not** attempted here.

**Why the missing check was in scope for 118.2 at all**, since it pre-dates the phase: what
118.2 supplied is the *stock of out-of-band queue entries* that made the desync reachable with
no server-side bug whatsoever. Task 2's latch removed that stock; without the id check the
poison would still be in the FIFO, and without the latch the check alone leaves it there. The
plan's claim that neither half alone suffices was **measured**: with the latch and no id check,
`binary(client_sse_stream)` ran 17 tests, 16 passed, 1 failed — fence 16 green, fence 17 still
red.

### Fallout the fix exposed: three test mocks were answering with ids no client ever sent

Not deferred — **fixed in plan 15's task 3**, recorded here because the shape is worth not
re-deriving. Adding the correlation check turned 25 pre-existing tests red at once:

| Mock | Tests |
|---|---|
| `src/client/mod.rs::tests::MockTransport` | 17 |
| `tests/common/mock_paginated.rs::MockTransport` (shared by `list_all_pagination` + `property_tests`) | 7 |
| `tests/property_tests.rs::CaptureTransport` | 1 |

Each replayed a canned response carrying a hand-written id (`1`, `2`, `i+2`, ...) while
`Client` mints its own — a `RequestId::String` holding a UUID for `call_tool`, a counter
elsewhere. They passed *because* `dispatch_request` did not compare ids: they were asserting on
the defect. The fix was to make each mock **echo the id of the request it was sent**, which is
what a conformant server does (JSON-RPC 2.0: the response id "MUST be the same as the value of
the id member in the Request Object"). No canned payload, cursor chain or pop order changed,
and the correlation check was not weakened. A mock that answers with a fabricated id is
precisely the hostile-peer shape fence 17 now refuses.

---

## GAP-CLOSURE ROUND (2026-08-17) — the review findings this closure DECLINES to fix

**Recorded by plan `118.2-16`, the closure's leftovers ledger.** `118.2-REVIEW.md` returned thirteen
findings. This gap closure (plans 14–18) fixed **four** of them: **CR-01** (plan 14, the unbounded
reconnect loop), **CR-02** (plan 15, the response/request desync and the terminal-error FIFO poison),
**WR-01** (plan 17, the reader parked on an idle-but-open stream) and **WR-02** (plan 17, the
cross-stream resumption cursor). The remaining **nine** — four Warnings and five Info items — are
real, were confirmed by a reviewer, and are **shipping unfixed**. They are indexed below so that a
later audit can tell a scoped decision from an oversight.

Each entry is an index into `118.2-REVIEW.md`, not a copy of it: the review holds the full analysis,
the reproduction and the suggested fix. **Line spans are as the review recorded them, against the
pre-closure tree** — plans 14, 15 and 17 have since shifted line numbers inside
`src/shared/streamable_http.rs`, so locate by symbol name rather than by line.

`WINDOWS.md` entries 4, 5, 6, 8 and 9 are **not** restated here; they are already correctly disclosed
(three open, two fixed) and re-recording them would double-book them.

### WR-03 — `start_sse` has a TOCTOU on `abort_handle` that can leave a second, untracked session reader

- **Review:** `118.2-REVIEW.md:327`.
- **Source:** `src/shared/streamable_http.rs:1153-1194` (`start_sse`); reachable off `&self` from
  `post_body`'s 202 branch (`:1818`) and `send_with_options`'s resumption branch (`:1605-1607`).
- **Consequence of leaving it:** two clones of a `Clone` transport can interleave the
  take-abort-spawn-store sequence, so two live GET session streams exist and the client sees each
  server-initiated message twice.
- **Reason deferred:** the fix is a new `tokio::sync::Mutex` field plus a serialised open sequence in
  the one function plans 14, 15 and 17 all edit; it needs its own concurrency fence (fence 4 exercises
  the sequential case only) and it was out of the closure's fenced scope.
- **Touches code this closure DOES modify — and the finding is now strictly NARROWER.** Plan 17 closed
  the reader-shutdown half (WR-01) and un-shared the cursor (WR-02), so the two readers no longer
  corrupt each other's reconnect cursor (each owns a local) and **both** are now stoppable by `close()`
  or by a dropped transport. WR-03 has therefore been reduced from *"a second GET may exist, poison the
  cursor, and outlive the transport"* to *"a second GET may exist"* — duplicate delivery only. The
  TOCTOU that can create that second reader stays open. Plan 17 routed this reduction here as
  `T-118.2-17-06` (disposition **transfer**).

### WR-04 — a reconnect GET gets no 401 refresh, unlike every POST

- **Review:** `118.2-REVIEW.md:370`.
- **Source:** `src/shared/streamable_http.rs:2706-2730` (`open_sse_once`) versus `:1696-1739`
  (`post_once`, which does implement the single-shot `provider.on_unauthorized()` retry).
- **Consequence of leaving it:** when a bearer token expires and the gateway drops the idle session
  stream, the reissued GET is answered `401`, the reconnect loop treats it as terminal, and the session
  stream is lost **permanently** — server-to-client requests (sampling, roots, elicitation) are dead
  with no recovery short of rebuilding the client, while POSTs on the same transport keep working after
  their own refresh.
- **Reason deferred:** it needs the refresh factored out of `post_once` plus a harness mode that
  answers the first GET `401` and the reconnect `200`; that is an auth-path change with its own fence,
  not part of the two Criticals the verification demanded.
- **Touches code this closure DOES modify:** plan 14 changed the reconnect loop's **timing** (a
  `MIN_SSE_RECONNECT_DELAY` floor and an uptime-gated budget) and not its **response handling**, so a
  `401` on a reconnect GET remains terminal exactly as the review describes.

### WR-05 — an ordinary EOF on the session stream escalates to an application-visible error, contradicting the documented taxonomy

- **Review:** `118.2-REVIEW.md:405`.
- **Source:** `src/shared/streamable_http.rs:2054` (the `Transport::receive` taxonomy row) versus
  `:2266-2268` and `:2796-2804` (the behaviour).
- **Consequence of leaving it:** a server that answers GET with `200` + immediate EOF — permitted by
  the spec — costs the client three GETs and then a hard `Err` on the receive queue on every
  connection, while the documented row promises the reader "exits silently". The doc and the code
  disagree, and the doc is the one an integrator reads.
- **Reason deferred:** resolving it means choosing between two contracts (stop escalating a clean EOF,
  or correct the table row and say escalation is intended). That is a contract decision for the
  transport's owner, not a bug fix, and picking one inside a closure scoped to CR-01/CR-02 would settle
  it by accident.
- **Touches code this closure DOES modify:** plan 15 rewrote how a terminal reason is **delivered**
  (off the response FIFO onto a sticky latch) while deliberately keeping the ordinary-EOF row of the
  taxonomy exactly as it stood, so the divergence WR-05 names survives untouched rather than
  half-fixed. Plan 15's own summary records that non-change explicitly.

### WR-06 — `logging/setLevel` reports success while silently discarding the level (stateless deployments, and batches)

- **Review:** `118.2-REVIEW.md:432`.
- **Source:** `src/server/streamable_http_server.rs:1627-1650` (`capture_v1_set_level`) and
  `src/server/core.rs` (`set_logging_level_response`).
- **Consequence of leaving it:** a stateless v1 deployment answers the mandated literal `{}` to every
  `logging/setLevel` and then applies `DEFAULT_LOG_LEVEL` forever — a client that asked for `error`
  keeps receiving `info` chatter it explicitly declined, which is unsolicited data on a channel it
  tried to quiet. Same function, second instance: a `logging/setLevel` inside a JSON-RPC **batch** (a
  top-level array) is never captured, because the method is read as `body.get("method")`.
- **Reason deferred — this one is explicitly out of scope for this closure, not merely unscheduled.**
  Fixing it would reopen **CONF-10** territory that plans 07, 08 and 13 already argued to a booked
  conclusion, with two prior verdicts in this very file (the `118.2-08` `logging/setLevel` verdict and
  the `118.2-13` `LogMessageParams` resolution) and two `WINDOWS.md` entries behind it. A transport
  closure carrying none of that context is the wrong place to relitigate a settled requirement, and the
  behaviour was left untouched on purpose. It also sits directly on top of four locked decisions —
  D-10 (filter on both eras), D-11 (the v1 level lives in the per-session `V1State` map), D-12 (default
  level `info`) and D-13 (both dispatch roots agree about `logging/setLevel`) — which recording rather
  than fixing is what keeps intact.

### IN-01 — the structural fences strip `//` comments only

- **Review:** `118.2-REVIEW.md:466`.
- **Source:** `tests/log_emitter.rs:557-568` (`code_lines`), `:1848` (`squeezed_code`).
- **Consequence of leaving it:** a `/* … */` block comment or a string literal containing
  `attach_request_log_sink(extra, None)` would satisfy `both_dispatch_roots_attach_the_log_sink`
  without either root calling anything — a fence that can be satisfied by prose.
- **Reason deferred:** low likelihood and test-only; the fix is to reuse the stronger stripper
  `tests/v1_severability_tripwire.rs:435-568` already implements, which is a test-infrastructure
  consolidation rather than a defect fix.

### IN-02 — fences 13 and 14 are timing-fragile under load

- **Review:** `118.2-REVIEW.md:478`.
- **Source:** `tests/client_sse_stream.rs` (`closing_during_reconnect_backoff_issues_no_further_get`,
  `dropping_the_transport_during_backoff_issues_no_further_get`).
- **Consequence of leaving it:** both wait for `get_lines() >= 2` and then assert `observed == 2`
  inside a 1.5 s backoff window, so a loaded CI box can fail them for a scheduling reason rather than a
  defect — a flake source in a suite that already carries one known conformance flake. It fails in the
  safe (loud) direction, not vacuously.
- **Reason deferred:** the fix is a harness change (hold the second GET open until the test signals) to
  two **existing** fences, and rewriting a green fence mid-closure voids its evidence for no
  correctness gain.
- **Touches code this closure DOES modify:** plan 14's new fence 15 is built to exactly the anti-flake
  discipline IN-02 asks for — bounded counts and monotonic **lower** bounds, never an upper wall-clock
  bound, measured server-side at accept time — and plans 15 and 17 followed the same rule for fences
  16–20. The two **existing** fences IN-02 names were not rewritten.

### IN-03 — the request body is fully re-parsed a third time per POST

- **Review:** `118.2-REVIEW.md:490`.
- **Source:** `src/server/streamable_http_server.rs:1687` (`raw_body_json` inside
  `resolve_request_log_level`), called at `:4629` and `:5421`.
- **Consequence of leaving it:** a third full `serde_json` deserialization of untrusted input per POST
  on the hot ingress path, when the parsed `Value` is already in hand at both call sites' enclosing
  scope. Correctness-neutral.
- **Reason deferred:** a performance refactor on the server ingress, explicitly outside v1 performance
  scope and outside this client-transport closure.

### IN-04 — `pub` closure fields with `#[allow(dead_code)]`

- **Review:** `118.2-REVIEW.md:500`.
- **Source:** `src/server/cancellation.rs:225` (`pub log_sink`), `:231` (`pub log_level`).
- **Consequence of leaving it:** two `pub` fields carrying live capability handles are part of the API
  surface — `examples/s54_v2_dual_conformance.rs` already reads `extra.log_level` directly, pinning the
  field name — so any code holding a `&mut RequestHandlerExtra` can swap the sink.
- **Reason deferred:** moving to accessors is an API-shape change; the struct is `#[non_exhaustive]` so
  nothing prevents it later, and doing it here would put a public-surface decision inside a closure
  scoped to two client defects.

### IN-05 — the v1 capability arm now spends ~60 s waiting out a dispatch budget

- **Review:** `118.2-REVIEW.md:511`.
- **Source:** `crates/pmcp-team-servers/tests/era_matrix.rs` (`v1_capability_arm`,
  `DISPATCH_TIMEOUT_MARKER`).
- **Consequence of leaving it:** two tools each wait out the server's ~30 s dispatch budget, turning a
  sub-second test into a minute-long one in a suite CI runs with `--test-threads=1`. The arm is a
  deliberate, documented record of the open client-lifecycle deadlock and is non-vacuous.
- **Reason deferred:** the improvement is a shorter server-side dispatch budget in the fixture (if one
  is configurable), which belongs with the client-lifecycle deadlock item it documents
  (`WINDOWS.md` #6/#9, already disclosed and not re-booked here) rather than with this closure.

### IN-06 — `parse_peer_log_level`'s "no rejection" rule is right; one consequence is worth stating in the rustdoc

- **Review:** `118.2-REVIEW.md:522`.
- **Source:** `src/server/streamable_http_server.rs:1603` (`parse_peer_log_level`).
- **Consequence of leaving it:** the function returns `None` for both "absent" and "garbage". When the
  already-recorded SEP-2575 v2 default is fixed, that two-way answer must become three-way
  (`Absent` / `Invalid` / `Level`) or a v2 client will re-enable logging it is prohibited from receiving
  simply by sending a malformed level — i.e. the SEP-2575 fix can land halfway and look complete.
- **Reason deferred:** it is a forward-looking rustdoc note whose natural owner is the SEP-2575 fix
  itself (CONF-10 territory, same reasoning as WR-06); writing it here without that fix in hand would
  document a shape nobody is yet implementing.

### Process defects this closure surfaced (not review findings, recorded so they do not recur)

Two defects in the closure's own **plan files** were found by running the commands those plans
specified. Both are already booked as OPEN deviations in `.planning/WINDOWS.md` (ids **10** and **11**,
which are new in this round and are not among the entries this appendix declines to restate). Neither
plan file was edited, by design — plan 18 owns the phase's traceability amendment.

- **The `pmat` jq path is wrong for pmat 3.15.0.** `118.2-15-PLAN.md:391` and `118.2-17-PLAN.md:341`
  both specify
  `pmat analyze complexity --format json --max-cognitive 25 | jq -e '[.violations[] | select(.path | startswith("src/"))] | length == 0'`.
  On pmat 3.15.0 — the version CLAUDE.md pins for CI — the top-level keys are exactly
  `["files", "summary", "top_files_limit"]`: there is no top-level `.violations`, and violation records
  key the path as `.file` with a `./` prefix. The command emits `Cannot iterate over null` and exits
  **5**. It fails **closed** rather than false-greening, but it measures nothing. The verified-correct
  form is
  `jq -e '[.summary.violations[] | select(.file | startswith("./src/"))] | length == 0'` → `true`,
  exit 0. **Related genuine false green, not to be used as a workaround:** scoping with
  `--files <path>` exits 0 while reporting `total_files: 1, total_functions: 0`.
- **A plan verify written as `--max-cognitive 25` is LOOSER than the CI gate it claims to predict.**
  This is a plan-template defect, not a source defect, and it will recur wherever the spelling is
  copied. Measured direction, established as fact: plan 17 found `read_sse_body` at cognitive
  complexity **exactly 25**, which **passes** `pmat analyze complexity --max-cognitive 25` while
  **failing** `pmat quality-gate --fail-on-violation --checks complexity` — the PR-blocking command
  CLAUDE.md names. So the plan's own verify block would have let a CI-failing tree through. Plan 17
  resolved its instance by **extracting** `end_of_frame_stop`, not by an `#[allow]`, holding the
  zero-`#[allow]` rule. **Unconfirmed, do not cite as established:** plan 17 reported the gate's
  effective cognitive threshold as **23**; that specific number could not be confirmed from pmat
  3.15.0's help output, which documents only `--max-complexity-p99` (default 50) and
  complexity-entropy (default 2.0) and lists no cognitive default. Treat the *direction* as measured
  and the *number* as unverified — and in either case run the **gate**, not the report.

### Disposition — who should own the remainder

Recorded without minting phase numbers this plan has no authority to assign.

**A client-transport hardening plan** is the natural owner of **WR-03**, **WR-04** and **WR-05**: all
three live in `src/shared/streamable_http.rs`, all three need a harness mode plus a fence in
`tests/client_sse_stream.rs`, and WR-05 additionally needs a contract decision (stop escalating a clean
EOF, or correct the taxonomy row) that should be taken once for the whole reader rather than per
defect. **A CONF-10 follow-up** is the natural owner of **WR-06** and **IN-06**, together with the
already-disclosed SEP-2575 v2 default (`WINDOWS.md` #5) that IN-06's three-way answer exists to serve;
that plan carries the D-10/D-11/D-12/D-13 context this closure deliberately did not load. **IN-01,
IN-02, IN-03, IN-04 and IN-05** are independent hygiene items with no ordering constraint between them
and no correctness consequence, and can ride along with whichever plan next touches their file.

Separately, and **not re-booked here:** the per-id response **ROUTING** redesign that plan 15's
discard-on-mismatch decision generates is already recorded above under
`## DEFERRED (118.2-15): per-id response ROUTING, and the cost of discard-on-mismatch`, together with
its accepted cost (`T-118.2-15-03`). See that entry rather than a duplicate here.

## CLOSING VERIFICATION (118.2-18) — findings the closing gate run surfaced, none of them the closure's

Appended by `118.2-18`, the gap-closure round's closing plan, **append-only**: nothing above is
rewritten, and plan 16's declined-findings appendix is cross-referenced rather than re-litigated.
These three are recorded because a closing verification that quietly re-runs a red gate until it is
green, and says only the green number, is the failure mode this whole round exists to refuse.

### A pre-existing `.expect` turns a transient keychain read failure into a panic

- **Where:** `src/shared/streamable_http.rs:1070` (and its HTTP/2 twin at `:1061`) —
  `.with_native_roots().expect("Failed to load native root certificates")`.
- **Provenance: NOT this closure's.** `git blame` attributes both lines to `1564e6226`
  ("fix: Improve HTTP transport compatibility for MCP server composition"), which predates Phase 118.2
  entirely. None of this closure's nine source commits touches them.
- **Measured here:** the first `make quality-gate` run of `118.2-18` failed with **5 of 5**
  `streamable_http_oauth_integration` tests panicking at that exact line, all with
  `Os(Error { code: -36, message: "I/O error." })` from the macOS keychain — the user, admin AND
  system trust stores all failing to load at once, while the volume was under the pressure the gate's
  own example-build step created (free space fell from 104Gi to 40Gi during that step). Preserved at
  `target/118.2-18-gate-keychain-fail.log`.
- **Proven environmental, not a regression:** the identical source re-ran **5 passed / 0 failed**
  (`target/118.2-18-oauth-rerun.log`), and the full `make quality-gate` then re-ran **exit 0**, "ALL
  TOYOTA WAY QUALITY CHECKS PASSED" (`target/118.2-18-gate.log`). Failing took 40.86 s; passing took
  4.78 s.
- **Consequence of leaving it:** a transient OS-level trust-store read failure — disk pressure, a
  wedged `syspolicyd`, a sandbox without keychain access — **panics inside library code** rather than
  returning an `Err` the caller can handle. A panic in a transport constructor is not something a
  downstream application can recover from or even diagnose from the message.
- **Reason deferred:** out of scope. It is pre-existing, it is not on the client SSE path this closure
  fixed, and turning an `.expect` into a propagated error changes a public constructor's failure mode —
  a decision that belongs with whoever owns the connector's error contract, not with a transport
  safety closure. Recorded so the next reader does not re-diagnose it as a code regression, which is
  exactly what it looks like at first sight.

### The conformance runner has two fail-closed environment guards; both fired before any scenario ran

Recorded because both produce a **non-zero exit having measured nothing**, and a closing record that
reported either as a suite result would be reporting a different referee — or no referee at all.

1. **Node version.** The shell's default `node` is `v20.8.1`; the suite needs `>= 22` because it
   imports `globSync` from `node:fs` at module scope. The script refuses up front with that
   explanation rather than dying in a stack trace that names neither Node nor this repo. Resolved by
   putting `~/.nvm/versions/node/v22.22.2/bin` on `PATH` — the script's own suggested remedy.
2. **`PMCP_REQUEST_STATE_KEY`.** Unset, and the script refuses rather than letting the example derive
   a fresh per-process key, because the MRTR surface the gate blocks on would then be measuring a
   different server than CI does. Resolved with an ephemeral non-production
   `openssl rand -hex 32` value, never echoed and never committed.

Both guards are **correct behaviour and worth keeping** — each is a "fails closed instead of measuring
nothing" control of exactly the kind this phase has been recording. They are noted here only so the
next person to run the suite locally does not read either refusal as a conformance failure.

### The D-16 gate went RED on the first closing run, and it is reported rather than accommodated

- **Run 1** (`target/118.2-18-conf.log`): the `2025-11-25` leg scored **72 passed / 1 scored failure**
  and the gate exited **1**. The failing scenario was `tools-call-sampling`, with the error message
  **verbatim** the one `WINDOWS.md` entry 9 already records: `MCP error -32603: Internal error:
  Protocol error: -32603 - Dispatch oneshot channel closed`. Its `WireSchemaValid` check **passed** —
  5 messages validated, 0 violations — so nothing about the emitted wire shape moved.
- **Run 2** (`target/118.2-18-conf-run2.log`), identical source, identical pin: `2025-11-25`
  **73 passed / 1 failed** (the pre-existing unscored `json-schema-2020-12`), **30 scored scenarios,
  floor 30, 0 failing, leg exit 0**; `2026-07-28` **142 passed / 36 failed**, 37 scored, floor 37, leg
  exit 0; all six floor assertions OK; script exit **0**. Byte-identical to `118.2-12`'s closing
  figures.
- **Disposition:** this is the SAME server-to-client request-lifecycle race already booked OPEN as
  `WINDOWS.md` entries 6 and 9, landing this time on `tools-call-sampling` rather than
  `tools-call-elicitation` — both are server-to-client request scenarios, and entry 9 already frames
  the finding as the race rather than as one scenario, so it is **not re-booked here and entry 9 is
  not edited**. What IS new is a second measured instance, which makes entry 9's "1 of 9 fresh runs"
  a floor rather than a curiosity.
- **Nothing was weakened, and D-21 carries forward verbatim.** No allowlist, no `--expected-failures`,
  no known-failure baseline, and `2025-11-25` was not removed from `FULLY_SCORED_GREEN_REVISIONS`:
  `git status --porcelain -- scripts/run-conformance-suite.sh conformance/` is **empty**, and
  `git diff --stat 83e46b68..HEAD -- scripts/ conformance/` is **empty** across the entire closure.
  The gate red-ing on a fresh run is a fact about an open defect, not a reason to move the gate.
- **The suite cannot attribute this closure's fixes either way.** It exercises pmcp only as a SERVER —
  CONF-09's own stated limitation (i) — and this closure changed CLIENT code only
  (`src/shared/streamable_http.rs`'s client half and `src/client/mod.rs`). Run 2's green is a
  regression check on the server-side gate, not validation of CR-01, CR-02, WR-01 or WR-02.
