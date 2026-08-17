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
