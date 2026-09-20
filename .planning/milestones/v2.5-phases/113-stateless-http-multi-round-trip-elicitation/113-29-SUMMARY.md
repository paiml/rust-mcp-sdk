---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 29
subsystem: protocol
tags: [error-codes, conformance, era-gating, tripwire, mcp-2026-07-28, finding-11]

# Dependency graph
requires:
  - phase: 113 (plan 04)
    provides: "`resolve_ingress_protocol_context` and the `sessions_active_for` predicate shape — 'one small pure era predicate, every decision site routed through it' — which both guards here follow"
  - phase: 113 (plan 21)
    provides: "`tests/v2_bounded_reads_tripwire.rs`, the house source-scanning idiom (runtime discovery, comment/literal stripping with a line map, `cfg(test)` exclusion, justified allowlist) this plan's tripwire re-states"
  - phase: 113 (plan 24)
    provides: "`MAX_MRTR_ROUNDS` in `core.rs`; untouched here"
  - phase: 113 (plan 26)
    provides: "the fallible AAD canonicaliser in `core.rs`; untouched here"
  - phase: 113 (plan 27)
    provides: "`Continuation.kinds` and the kind-directed ingress in `core.rs`; untouched here"
provides:
  - "an EXECUTED reachability verdict for both pmcp `-32002` emission sites on the v2 path — both reachable, neither by the route the source comments implied"
  - "`v1_initialize_gate_applies` — the named era predicate that keeps `ServerCore`'s server-not-initialized `-32002` off the v2 wire"
  - "`is_v1_task_era` — the named era predicate that routes a v2 `tasks/result` pending refusal to `METHOD_NOT_FOUND` instead of `-32002`"
  - "`tests/v2_prohibited_error_codes.rs` — the executed probes, the two v1 negative controls, the SHOULD-NOT-range inventory, and a source tripwire that fails on any new or unguarded `V1_TASK_PENDING` emission site"
  - "the measured, previously-unwritten fact that `UNSUPPORTED_CAPABILITY` has ZERO emission sites, which is what makes it safe on the same prohibited number"
affects: [HTTP-01, VERS-06, 114 (TASK-03 owns the real v2 task semantics this plan names but does not implement)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "a prohibition declared as DATA (`MUST_NOT_EMIT`, `SHOULD_NOT_USE_RANGE`) rather than as prose in an assertion message, so the assertion is over the RULE and not over the one number the codebase happens to use"
    - "prove branch arrival on the v1 leg BEFORE claiming anything about the v2 leg, against the SAME fixture — a probe that never reached its branch then fails loudly instead of passing vacuously"
    - "a reachability trace that crosses the surface the era actually travels on: an in-process typed probe reported one site UNREACHABLE and was wrong about the system, not about itself"
    - "a scanner idiom re-stated with its ONE divergence measured and documented, rather than copied and assumed compatible"

key-files:
  created:
    - tests/v2_prohibited_error_codes.rs
  modified:
    - src/server/core.rs
    - src/server/task_dispatch.rs
    - src/server/mod.rs
    - src/types/protocol/error_codes.rs

key-decisions:
  - "BOTH sites are v2-reachable. Recorded by execution, with the emitted `error.code` read off a real response — not by inspection. The plan allowed for a negative result; the measurement did not produce one."
  - "The `task_dispatch.rs` site is reachable only over HTTP, and the in-process typed probe says the opposite. `ClientRequest::TasksResult` is an enumerated NON-`_meta`-bearing variant, so `ServerCore::handle_request` can never classify a `tasks/result` as v2. Had the trace stopped at the surface the plan proposed, it would have concluded 'v1-only by construction' and shipped a conformance violation. The HTTP ingress reads `params._meta` off the RAW body, which is where the era actually arrives."
  - "The `core.rs` fix skips the gate on v2 rather than changing its code. A v2 request carries no `initialize` handshake by design (HTTP-01), so a different constant would still be refusing a conformant request."
  - "`stateless_mode` could never have covered the `core.rs` site. `ServerCoreBuilder::build` resolves it as `unwrap_or_else(Self::detect_stateless_environment)` — ENVIRONMENT auto-detection, not an era decision — so the era had to be a separate clause."
  - "`is_v1_task_era` gates ONLY the `-32002` emission. `tasks/get|list|cancel` are unchanged on every era: the real v2 task semantics belong to Phase 114 / TASK-03, and re-deciding them here would be a redesign rather than the removal of a prohibited wire value."
  - "No new wire value was invented. The v2 `tasks/result` branch answers the already-existing `METHOD_NOT_FOUND`, which is TRUE today — the tasks extension is not in pmcp's advertised `capabilities.extensions` map."
  - "The `-32000`..`-32019` SHOULD-NOT residual is inventoried, not changed. `RATE_LIMITED` on the `subscriptions/listen` refusals is plan 113-18's deliberate decision and stays."
  - "HTTP-01 stays `[~]`; `.planning/REQUIREMENTS.md` was NOT edited and no checkbox was flipped."

patterns-established:
  - "Pattern: when a plan names the surface to probe on, verify that surface can CARRY the signal being probed for. A probe on a surface that structurally cannot express the input under test measures the probe, not the system."
  - "Pattern: re-stating a shared code idiom in a second crate is fine, but the divergence must be measured. This scanner's one divergence from 113-21's was found by the scan reporting a file as naming a symbol it visibly declares."

requirements-completed: []  # HTTP-01 and VERS-06 both untouched — the STATE.md publication gate forbids flipping HTTP-01..09 / CLNT-01/02/05 this round

# Metrics
duration: 78min
completed: 2026-07-27
---

# Phase 113 Plan 29: Trace and Close the Prohibited `-32002` Emissions Summary

**Both pmcp `-32002` call sites were commented as v1-scoped and neither had been traced; driving a real v2 request at each one read `-32002` off both responses, and the `tasks/result` site turned out to be reachable only over HTTP — the in-process typed probe the plan proposed reports it unreachable, because `ClientRequest::TasksResult` has no `_meta` field for an era signal to ride on. Both are now era-gated behind named predicates, both v1 wires are byte-identical, and a source tripwire fails on any future `V1_TASK_PENDING` emission site that is not allowlisted with the guard that keeps it off the v2 path.**

## Performance

- **Duration:** ~78 min
- **Started:** 2026-07-27T12:54:49Z
- **Completed:** 2026-07-27T14:13:00Z
- **Tasks:** 2 (2 commits + this metadata commit)
- **Files:** 1 created, 4 modified

## Task Commits

1. **Task 1: Trace both sites by execution and build the prohibited-code tripwire** — `eb6988a9` (test)
2. **Task 2: Era-gate the reachable sites and restate the constant's contract** — `08af76db` (fix)

## The reachability trace — recorded verbatim, task 1, against the UNGUARDED tree

This is the artifact the plan exists to produce. It replaces "both look v1-scoped".

```
running 18 tests
test site_a_v1_uninitialised_request_still_emits_minus_32002 ... ok
test site_a_v2_request_must_not_elicit_a_prohibited_code ... FAILED
test site_b_v1_pending_tasks_result_still_emits_minus_32002 ... ok
test site_b_typed_surface_cannot_carry_a_v2_era_signal ... ok
test every_code_a_v2_request_elicits_here_is_inventoried ... FAILED
test site_b_v2_http_request_must_not_elicit_a_prohibited_code ... FAILED
test every_v1_task_pending_site_is_allowlisted_and_era_guarded ... FAILED
test the_cfg_test_exclusion_is_load_bearing_on_a_real_file ... ok
test unsupported_capability_is_declared_twice_and_emitted_never ... ok
(+ 9 scanner / allowlist-hygiene tests, all ok)

---- site_a_v2_request_must_not_elicit_a_prohibited_code stdout ----
panicked at tests/v2_prohibited_error_codes.rs:176:5:
src/server/core.rs not-initialized gate: a v2 request elicited -32002, which protocol
version 2026-07-28 MUST NOT emit (docs/specification/draft/basic/index.mdx § Error Codes;
Finding 11). Era-gate the site — do not change the v1 wire value, which is frozen.

---- site_b_v2_http_request_must_not_elicit_a_prohibited_code stdout ----
panicked at tests/v2_prohibited_error_codes.rs:521:5:
src/server/task_dispatch.rs tasks/result: a v2 request elicited Some(-32002), which
protocol version 2026-07-28 MUST NOT emit (Finding 11). body was
{"jsonrpc":"2.0","id":2,"error":{"code":-32002,"message":"task result not available: task not completed"}}

---- every_code_a_v2_request_elicits_here_is_inventoried stdout ----
panicked at tests/v2_prohibited_error_codes.rs:573:9:
a v2 request elicited the prohibited code -32002

---- every_v1_task_pending_site_is_allowlisted_and_era_guarded stdout ----
Finding 11 tripwire — the V1_TASK_PENDING emission population changed:
  MISSING era guard: src/server/core.rs no longer contains `v1_initialize_gate_applies`.
  MISSING era guard: src/server/task_dispatch.rs no longer contains `is_v1_task_era`.

test result: FAILED. 14 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s
```

**Both sites: v2-REACHABLE.** Both v1 negative controls GREEN from the start.

### What each probe actually drove

| Probe | Fixture | Request | Result |
|---|---|---|---|
| A | `ServerCore` via `ServerCoreBuilder`, `stateless_mode(false)`, `2026-07-28` opted in, NOT initialised | `tools/call` with `_meta.io.modelcontextprotocol/protocolVersion = "2026-07-28"`, through the PUBLIC `ProtocolHandler::handle_request` | `-32002` |
| B | real `StreamableHttpServer` + `InMemoryTaskStore`, no router, default (stateful) config | POST `tasks/result` with the three v2 headers and the reserved `_meta` era key | `-32002` |

Branch arrival was proven positively BEFORE either code assertion, against the SAME fixture: probe A re-drives the v1 request and asserts the `"Server not initialized. Call initialize first."` message; probe B mints a v1 session and asserts the `"task result not available: task not completed"` message. A probe that had silently taken some other path would fail on the message, not pass on the code.

## The correction that matters: the plan's probe surface would have produced the WRONG answer for site B

The plan specified probe B as an in-process `ServerCore` probe — "these are in-process `ServerCore` probes, so a streamable-http gate is likely unnecessary". **On that surface, site B is unreachable on v2, and that conclusion is false about the system.**

`ClientRequest::TasksResult` is one of the explicitly enumerated NON-`_meta`-bearing variants in `extract_request_meta_value` (`core.rs:3302-3316`). A `tasks/result` presented to `ServerCore::handle_request` or `Server::handle_request` has **nowhere to put an era signal**, so `resolve_ingress_protocol_context` always returns v1 for it. A trace that stopped there would have recorded "v1-only by construction", left the site unguarded, and shipped a conformance violation.

The era does reach that code path — it just arrives somewhere else. `run_v2_header_gate` reads `params._meta` off the **RAW request body** before deserialization, so the JSON `_meta` survives even though the typed `GetTaskPayloadRequest` silently drops it. That resolved context is threaded into `Server::handle_request_with_context`, which intercepts `tasks/*` and calls `route_tasks_endpoint`.

Both facts are pinned in the suite rather than left as prose:

- `site_b_typed_surface_cannot_carry_a_v2_era_signal` executes the typed path and asserts it still lands on the v1 branch — with an anti-vacuity leg first, proving the SAME core does resolve `Era::V2` for a `tools/call`, so the measurement is about the VARIANT and not about the fixture;
- `site_b_v2_http_request_must_not_elicit_a_prohibited_code` is the decisive probe, over a real socket.

This is the generalisable lesson: **a probe on a surface that structurally cannot express the input under test measures the probe, not the system.**

## What changed

### `src/server/core.rs` — `v1_initialize_gate_applies`

```rust
pub(crate) const fn v1_initialize_gate_applies(
    stateless_mode: bool,
    era: Option<crate::types::protocol::Era>,
) -> bool {
    !stateless_mode && !matches!(era, Some(crate::types::protocol::Era::V2))
}
```

Follows plan 113-04's `sessions_active_for` shape — one small pure predicate, every decision site routed through it — with the same truth table in its rustdoc.

The v2 clause is a rule change, not a code change: a v2 request carries **no `initialize` handshake at all** (HTTP-01), so demanding one before serving it is simply wrong for that era. A different constant would still be refusing a conformant request.

`stateless_mode` was never going to cover this. `ServerCoreBuilder::build` resolves it as `self.stateless_mode.unwrap_or_else(Self::detect_stateless_environment)` — **environment auto-detection**, not an era decision. A plain non-Lambda process gets `false`, which is exactly the configuration probe A drove.

This site is not behind the streamable-HTTP transport whose era gating Phase 113 built: `ProtocolHandler` is a **public trait**, consumed in-repo by `src/server/wasi_adapter.rs` and `crates/pmcp-tasks` and by any downstream embedder. D-113-C's `sessions_active` predicate governs SESSION resolution in `streamable_http_server.rs` and does not reach here.

### `src/server/task_dispatch.rs` — `is_v1_task_era` + `V2_TASKS_NOT_NEGOTIATED`

The pending refusal became a three-arm match:

| `task_store.is_some()` | `is_v1_task_era(era)` | answer |
|---|---|---|
| `true` | `true` | `-32002 "task result not available: task not completed"` — **byte-identical** |
| `true` | `false` | `METHOD_NOT_FOUND` + `V2_TASKS_NOT_NEGOTIATED` |
| `false` | — | `METHOD_NOT_FOUND "tasks/result not supported"` — unchanged |

`METHOD_NOT_FOUND` is the truthful answer, not a convenient one: on 2026-07-28 the task lifecycle is an **extension** that has to be negotiated through `capabilities.extensions`, and pmcp advertises no `io.modelcontextprotocol/tasks` entry (TASK-01, Phase 114, `[ ]`). The message says so. **No new wire value was invented** — spending a second exception against VERS-06 was explicitly out of fence.

The predicate gates **only** the `-32002` emission. `tasks/get`, `tasks/list` and `tasks/cancel` are unchanged on every era; their v2 semantics are Phase 114 / TASK-03's, and the source comment names that ownership as a fact rather than as a TODO (zero SATD).

The era reaches the module as a parameter on `route_tasks_endpoint`, passed from the two call sites that already hold the ingress-resolved `ProtocolContext` (`core.rs` dispatch, `mod.rs` adapter (a)). This module still runs **no era resolver of its own** — it consumes the one authoritative per-request verdict, per D-11 / Pitfall 2.

### `src/types/protocol/error_codes.rs` — the constant's contract, restated

`V1_TASK_PENDING`'s rustdoc now states all four required things: the value is FROZEN and **v1-only**; 2026-07-28 **MUST NOT** emit it, quoted with its source and its post-RC provenance; a table of the two call sites and the era guard each carries; and the names of the tests that enforce it.

`UNSUPPORTED_CAPABILITY` records the fact that makes it safe on the same prohibited number, which was **written down nowhere before**: it has **no emission site**. It is declared here, re-declared once as the delegating `ErrorCode::UNSUPPORTED_CAPABILITY`, and used nowhere in compiled `src/`.

## The tripwire, and the two things that make it non-vacuous

`every_v1_task_pending_site_is_allowlisted_and_era_guarded` scans every `.rs` file under `src/` (discovered at runtime, so a NEW file cannot escape by omission), strips comments and string/char literal contents, excludes `cfg(test)` regions, and matches `V1_TASK_PENDING` as a **whole token**. The shipped population must equal a declared allowlist of three:

| Path | Kind | Guard |
|---|---|---|
| `src/types/protocol/error_codes.rs` | `Definition` | — (declares the number; never writes it) |
| `src/server/core.rs` | `Emission` | `v1_initialize_gate_applies` |
| `src/server/task_dispatch.rs` | `Emission` | `is_v1_task_era` |

`Definition` and `Emission` are different enum variants precisely so a declaration site cannot be mistaken for an emission site. An unlisted file fails; a stale entry fails; a missing guard string fails.

**Anti-vacuity is enforced on real files, not only on fixtures.** `the_cfg_test_exclusion_is_load_bearing_on_a_real_file` asserts that `src/server/streamable_http_server.rs` DOES name the token with `cfg(test)` regions included and does NOT with them excluded, and that the definition site survives the exclusion. Without that, an over-eager exclusion could empty the whole scan and every check would report success — which is the failure mode plan 113-09 found twice in this phase.

The scanner's own six unit tests cover: a bare emission counted, comment/doc-comment/inner-doc/nested-block-comment not counted, string and raw-string contents not counted, `cfg(test)` bodies excluded while code AFTER them stays in scope, a longer identifier not counted, and the `cfg` predicate classifier.

### The scanner's one divergence from 113-21's, found by measurement

`tests/v2_bounded_reads_tripwire.rs` **removes** whitespace, because its needles are method chains (`.collect().await`) that rustfmt breaks across lines. This scanner **collapses whitespace to a single space** instead, because it matches identifiers, which need word boundaries.

That divergence was not designed, it was measured. With whitespace removed, `pub const V1_TASK_PENDING` becomes `pubconstV1_TASK_PENDING`, the character before the token is `t` — an identifier character — and the whole-token filter silently rejects the **definition site**. The first run of this file reported `error_codes.rs` as naming `UNSUPPORTED_CAPABILITY` nowhere, which is visibly false, and that is how the bug surfaced. The rustdoc on `strip` records it so the next person does not re-derive it.

## Negative controls — each guard proven load-bearing, recorded verbatim

Each guard was disabled in turn against the LANDED code, the suite re-run, and the file restored from a byte-for-byte backup (md5 verified before and after; `git status` clean; `grep -rn "let _ = era" src/ tests/` empty).

### NC 1 — `core.rs`'s `v1_initialize_gate_applies` with its era clause removed

```
test site_a_v1_uninitialised_request_still_emits_minus_32002 ... ok
test site_a_v2_request_must_not_elicit_a_prohibited_code ... FAILED
test site_b_v2_http_request_must_not_elicit_a_prohibited_code ... ok
test every_code_a_v2_request_elicits_here_is_inventoried ... FAILED

---- site_a_v2_request_must_not_elicit_a_prohibited_code stdout ----
src/server/core.rs not-initialized gate: a v2 request elicited -32002, which protocol
version 2026-07-28 MUST NOT emit …

test result: FAILED. 16 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s
```

### NC 2 — `task_dispatch.rs`'s `is_v1_task_era` forced to `true`

```
test site_a_v2_request_must_not_elicit_a_prohibited_code ... ok
test site_b_v1_pending_tasks_result_still_emits_minus_32002 ... ok
test site_b_v2_http_request_must_not_elicit_a_prohibited_code ... FAILED
test every_code_a_v2_request_elicits_here_is_inventoried ... FAILED

---- site_b_v2_http_request_must_not_elicit_a_prohibited_code stdout ----
src/server/task_dispatch.rs tasks/result: a v2 request elicited Some(-32002) … body was
{"jsonrpc":"2.0","id":2,"error":{"code":-32002,"message":"task result not available: task not completed"}}

test result: FAILED. 16 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
```

**The orthogonality is the evidence.** Disabling site A's guard fails ONLY site A's probe; disabling site B's fails ONLY site B's. Neither test is restating the other, and in both runs **every v1 negative control stayed GREEN** — the guards touch only the v2 leg.

## Blast radius — stated plainly

Two wire behaviours change, both on v2 only, and neither on a path a `pmcp::Client` exercises today.

1. **A v2 request to an uninitialised, non-stateless `ServerCore`** now **succeeds** where it previously got `-32002`. This is the intended HTTP-01 behaviour: v2 is handshake-free, so there was never anything to wait for. The reachable population is embedders calling the public `ProtocolHandler` directly (`wasi_adapter`, `crates/pmcp-tasks`, downstream users) on a v2-opted-in build.
2. **A v2 `tasks/result` for an incomplete task** now gets `METHOD_NOT_FOUND` with a stated reason, where it previously got `-32002`. Reachable only over the HTTP transport on a v2-opted-in server with a `TaskStore` and no `TaskRouter`.

**Nothing on v1 changes at all**, at either site, proven by two negative controls that were GREEN before the change and stayed GREEN after, plus the pre-existing `pending_tasks_result_preserves_minus_32002` locking test, which is GREEN and **unedited** (`git diff HEAD -- src/server/task_dispatch_tests.rs` empty).

No existing test needed modification to accommodate the change: the full gate is exit 0 across the whole workspace with zero edits to any pre-existing test.

## The SHOULD-NOT residual — inventoried, not ignored

The same spec section says new implementations SHOULD NOT use `-32000`..`-32019` at all. This plan changes **no** code because of it, and records the residual instead so that "we did not look" and "we looked and decided" stay distinguishable.

| Code | Symbol | Disposition |
|---|---|---|
| `-32005` | `RATE_LIMITED` | **KEEP.** Plan 113-18 deliberately routed all three `subscriptions/listen` refusals here at HTTP 200; the v2 transport has no spec-allocated resource-exhaustion code and inventing a `-3202x` would spend a second VERS-06 exception. SHOULD NOT, not MUST NOT. |
| `-32002` | `UNSUPPORTED_CAPABILITY` | **Never emitted** — measured, see above. |
| `-32001`, `-32003`, `-32004`, `-32006` | `REQUEST_TIMEOUT`, `AUTHENTICATION_REQUIRED`, `PERMISSION_DENIED`, `CIRCUIT_BREAKER_OPEN` | Not elicited by any v2 path this suite drives. Untouched. |

`every_code_a_v2_request_elicits_here_is_inventoried` re-drives the v2 probes, COLLECTS the emitted codes at runtime rather than declaring them, asserts the set disjoint from `MUST_NOT_EMIT`, and requires an allowlist entry with a ≥40-character justification for any member landing in the SHOULD-NOT range.

## Verification

| Check | Result |
|---|---|
| `cargo test --features full --test v2_prohibited_error_codes` | **18 passed**, 0 failed |
| `cargo test --features full --lib -- pending_tasks_result` | **1 passed** — the locking test, GREEN and unedited |
| `cargo test --features full --lib -- server::task_dispatch_tests server::core` | **130 passed**, 0 failed |
| `cargo test --features full --test v2_mrtr` | 34 passed (113-27's, unchanged) |
| `cargo test --features full --test v2_mrtr_ingress` | 12 passed |
| `cargo test --features full --test v2_stateless_http` | 23 passed |
| `cargo test --features full --test v2_required_headers` | 25 passed |
| `cargo test --features full --test v2_bounded_reads_tripwire` | 13 passed (113-21's tripwire, unweakened) |
| `cargo clippy --features full --lib --tests -- -D clippy::all` | exit 0 |
| **`make lint`** (pedantic + nursery + examples, the real gate) | **exit 0** — `✓ No lint issues` |
| `cargo fmt --all -- --check` | exit 0 |
| **`make quality-gate`** (background job, polled) | **exit 0** — **252 test-result lines, 4487 passed, 0 failed** |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks: 223 pass, 30 skip — no semver update required** |
| `make wasm-build` | `✓ WASM build complete`; **85 → 85 warnings, ZERO new** |
| `Cargo.toml` / `Cargo.lock` | **untouched** — zero new dependencies |
| `git diff --name-only -- .planning/REQUIREMENTS.md` | **empty** |

Totals were read from the raw gate log with `awk` and every cargo invocation went through `$HOME/.cargo/bin/cargo`, since the rtk shell proxy compresses `test result:` lines.

**semver-checks is unmoved and that is not an accident**: everything added is `pub(crate)` (`v1_initialize_gate_applies`, `is_v1_task_era`, `V2_TASKS_NOT_NEGOTIATED`) or a widened `pub(crate)` signature (`route_tasks_endpoint`, `handle_tasks_result`). No public type, signature or constant changed.

**Zero new wasm warnings** — unlike 113-26 and 113-27, which each added some. The baseline was measured the safe way (`tar` backup → `git checkout HEAD~2 -- <files>` → measure → restore from tar → md5 diff → `git restore --staged`), and is **85** both before and after. Everything added is `#[cfg(not(target_arch = "wasm32"))]` or inside `task_dispatch.rs`, whose entire module is non-wasm.

## Threat Register Disposition

| Threat ID | Disposition | Evidence |
|---|---|---|
| T-113-139 (Tampering, `core.rs` gate emitting `-32002` on v2) | **mitigated** | probe A traced it reachable; `v1_initialize_gate_applies` skips on `Era::V2`; NC 1 shows the suite fails without the guard; `site_a_v1_...` proves the v1 wire byte-identical |
| T-113-140 (Tampering, `task_dispatch.rs` `tasks/result` emitting `-32002` on v2) | **mitigated** | probe B traced it reachable over real HTTP; the v2 branch answers `METHOD_NOT_FOUND`; NC 2 shows the suite fails without the guard; no new wire value invented |
| T-113-141 (Repudiation, a reachability claim resting on inspection) | **mitigated** | both probes assert branch arrival on the v1 message BEFORE the code, against the same fixture; probe A additionally asserts the handler echoed `Era::V2` on the success path, so "the gate no longer fires" cannot be satisfied by the request failing some other way |
| T-113-142 (Tampering, a FUTURE prohibited-code emission site on a v2 path) | **mitigated** | `every_v1_task_pending_site_is_allowlisted_and_era_guarded` — unlisted site, missing guard and stale entry all fail; six scanner unit tests plus a real-file anti-vacuity check keep it from passing over nothing |
| T-113-143 (Info Disclosure, the SHOULD-NOT range left unmeasured) | **accept, inventoried** | the table above; `RATE_LIMITED` kept with 113-18's recorded justification |
| T-113-144 (DoS, an era guard catching v1 traffic) | **mitigated** | two v1 negative controls GREEN before AND after; `pending_tasks_result_preserves_minus_32002` GREEN and unedited; full gate exit 0 with no test modified |

## Threat Flags

None. No new network endpoint, auth path, file access pattern or schema at a trust boundary. The change is two era predicates and a docs restatement on existing code paths.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Probe B on the plan's prescribed surface gives the WRONG reachability answer**

- **Found during:** Task 1
- **Issue:** The plan specified probe B as an in-process `ServerCore` probe. `ClientRequest::TasksResult` is an enumerated non-`_meta`-bearing variant, so on that surface a `tasks/result` can never be classified v2 and the site reports UNREACHABLE. The site IS reachable — over HTTP, where `run_v2_header_gate` reads `params._meta` off the raw body.
- **Fix:** Kept the typed probe as an explicitly-named test recording that the typed surface is v1-by-construction (with an anti-vacuity leg proving the same core does resolve `Era::V2` for a `tools/call`), and added the decisive probe over a real `StreamableHttpServer`. The file's gate was widened to `feature = "http-client"` accordingly.
- **Files modified:** `tests/v2_prohibited_error_codes.rs`
- **Committed in:** `eb6988a9`

**2. [Rule 1 - Bug] The re-stated scanner silently dropped the definition site**

- **Found during:** Task 1
- **Issue:** 113-21's `strip` REMOVES whitespace, which is correct for method-chain needles and wrong for identifier tokens: `pub const V1_TASK_PENDING` becomes `pubconstV1_TASK_PENDING`, so the whole-token boundary check rejects the declaration. Measured, not predicted — the scan reported `error_codes.rs` as naming `UNSUPPORTED_CAPABILITY` nowhere.
- **Fix:** Whitespace runs collapse to a single space. The divergence and the reason are in `strip`'s rustdoc, and `the_cfg_test_exclusion_is_load_bearing_on_a_real_file` asserts the definition site survives so a regression cannot hide.
- **Committed in:** `eb6988a9`

**3. [Rule 2 - Missing critical functionality] The v1 HTTP control needs a session**

- **Found during:** Task 1
- **Issue:** The default `StreamableHttpServerConfig` is STATEFUL, so the first v1 `tasks/result` was refused with `-32600 "Session ID required for non-initialization requests"` and never reached the tasks route — the control was measuring the session gate. Caught by the branch-arrival assertion, which is exactly what it exists for.
- **Fix:** `v1_session()` mints a session with a v1 `initialize` first. The v2 leg needs none, which is HTTP-01 and is stated in the helper's rustdoc.
- **Committed in:** `eb6988a9`

**4. [Rule 3 - Blocking] `Server::handle_request` is private**

- **Found during:** Task 1
- **Issue:** The plan's typed-surface probe assumed `Server::handle_request` was callable from an integration test. It is `pub(crate)`, as is `handle_request_with_context`.
- **Fix:** The typed probe drives `ServerCore` (whose `ProtocolHandler::handle_request` IS public) with a `task_store`, which is the typed surface an embedder can actually reach.
- **Committed in:** `eb6988a9`

**5. [Rule 3 - Blocking] `src/server/mod.rs` is not in the plan's `files_modified`**

- **Found during:** Task 2
- **Issue:** The era has to reach `route_tasks_endpoint`, and one of its two call sites is the `tasks/*` adapter in `src/server/mod.rs`.
- **Fix:** Added the argument at both call sites from the `protocol_context` each already holds. `TaskDispatch` itself is unchanged, so this is a parameter, not a new field, and the module still runs no era resolver of its own.
- **Committed in:** `08af76db`

**6. [Rule 2 - Missing critical functionality] The plan's `unsupported_capability_has_no_emission_site` assertion is not true as written**

- **Found during:** Task 1
- **Issue:** The plan asked to "check whether IT has any emission site at all before assuming it does not". A bare `.is_empty()` assertion fails: `src/error/mod.rs:178` names the symbol. That line is the delegating `ErrorCode::UNSUPPORTED_CAPABILITY` DECLARATION, not an emission.
- **Fix:** The test now measures the decidable claim in two parts — the symbol appears in exactly its two declaration files, and `ErrorCode::UNSUPPORTED_CAPABILITY` has zero use sites. That second half is the one that makes it safe, and it is now asserted rather than assumed.
- **Committed in:** `eb6988a9`

---

**Total deviations:** 6 auto-fixed (2× Rule 1, 2× Rule 2, 2× Rule 3)
**Impact on plan:** No scope creep; no requirement checkbox flipped; no new wire value invented. Deviation 1 is the substantive one — the plan's prescribed probe surface would have produced a false negative and closed Finding 11 with the violation still shipping.

## Issues Encountered

One process note, worth recording because it nearly corrupted a measurement: **zsh does not word-split unquoted variables**, so a `FILES="a b c"; cmd $FILES` idiom passed the whole string as one argument. The wasm-baseline script silently no-op'd its `git checkout` and would have reported the CURRENT tree as the baseline. Caught because `tar`/`md5` errored loudly on the same line. Redone with explicit file lists.

No `git stash` was run at any point. No `git clean`, no `git reset --hard`. The one index-only operation used was `git restore --staged <paths>`, to undo the `git checkout HEAD~2 -- <paths>` staging after the baseline measurement; the working tree was restored from a `tar` backup and md5-verified byte-identical.

## Known Stubs

None.

## Requirements

**HTTP-01 stays `[~]` and VERS-06 stays `[x]`.** `.planning/REQUIREMENTS.md` was **NOT edited** and no checkbox was flipped — the STATE.md publication gate forbids flipping HTTP-01..09 / CLNT-01/02/05 this round. `requirements-completed` in the frontmatter is deliberately empty for the same reason.

The three landed v2 constants (`-32020`/`-32021`/`-32022`) were **not** touched: they are pre-final values under a written exception and re-deciding them is not this plan's business.

## What This Does NOT Close

- **The v2 tasks surface.** `tasks/get`, `tasks/list` and `tasks/cancel` still serve v1 semantics to a v2 caller. This plan only removed a prohibited wire value from one branch of one of them. TASK-01..06 (Phase 114) own the real answer, and `is_v1_task_era`'s rustdoc names that ownership.
- **Whether `-32042` is reachable.** It has never had a name or a call site in pmcp; it is in `MUST_NOT_EMIT` so that the assertion is over the RULE, but there was nothing to trace.
- **The SHOULD-NOT range.** `RATE_LIMITED` remains v2-reachable by deliberate decision. The residual is inventoried, not removed.
- **The tripwire is a change detector over one SYMBOL.** A future site that writes the bare literal `-32002` rather than reading `V1_TASK_PENDING` would not be caught by the source scan — it would be caught by the executed probes only if it sat on a path this suite drives. The centralized-table discipline (VERS-06) is what makes the symbol scan meaningful, and it remains the load-bearing convention.
- **Still open and unowned in this phase:** D-113-Q (`sse_optimized.rs` unbounded `reqwest` body), D-113-R (`drain_complete_lines` quadratic), D-113-S (no stdio listen), D-113-T (intermittent `LEAK` in four pre-existing `v2_subscriptions.rs` tests).
- No blockers introduced.

## Next Phase Readiness

- **113-28 (the decision checkpoint) is unblocked.** This was the last autonomous plan of the gap-closure round; Finding 11 is now closed by execution rather than left `untraced`.
- **A re-verifier should drive site B over HTTP, not through `ServerCore`.** The typed surface reports the site unreachable and is wrong about the system; a re-verification that used the typed path would report a false GREEN against an unguarded tree.
- **Phase 114 inherits a named seam.** `is_v1_task_era` and `V2_TASKS_NOT_NEGOTIATED` are the two places TASK-03 will replace when the v2 task surface is actually negotiated.

## Self-Check: PASSED

- `tests/v2_prohibited_error_codes.rs` — FOUND (1400 lines, ≥260 required), contains `MUST_NOT_EMIT`
- `src/server/core.rs` — FOUND, contains `V1_TASK_PENDING` and `v1_initialize_gate_applies`
- `src/server/task_dispatch.rs` — FOUND, contains `V1_TASK_PENDING` and `is_v1_task_era`
- `src/types/protocol/error_codes.rs` — FOUND, contains `MUST NOT` and `v2_prohibited_error_codes`
- `src/server/mod.rs` — FOUND, era threaded to `route_tasks_endpoint`
- commit `eb6988a9` — FOUND (`test(113-29): trace both -32002 sites on the v2 path by execution`)
- commit `08af76db` — FOUND (`fix(113-29): era-gate both -32002 sites so no v2 request can elicit one`)
- `git status --short -- src/ tests/` empty after both negative controls were reverted; both files md5-verified against their pre-control backups
- `grep -rn "let _ = era" src/ tests/` — no matches (no negative-control residue)
- `.planning/REQUIREMENTS.md` — NOT in the diff

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-27*
