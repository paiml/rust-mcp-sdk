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
