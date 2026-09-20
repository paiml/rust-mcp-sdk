---
phase: 114-tasks-extension-migration
plan: 09
subsystem: server/tasks
tags: [security, auth, tasks, v2, owner-binding, fail-closed, TASK-05]
requires:
  - "114-08: era predicates (tasks_list_serves_on_era, tasks_result_serves_on_era) and V2_TASKS_METHOD_RETIRED"
  - "114-07: store-layer check_owner returns NotFound, never revealing another owner's task"
  - "114-02: tests/common/v2.rs shared harness incl. OptionalBearer + AuthPosture"
  - "113: resolve_mrtr_principal, MrtrPrincipal, ANONYMOUS_PRINCIPAL"
  - "112: ProtocolContext resolved once at ingress"
provides:
  - "TaskDispatch::resolve_owner — era-aware, fail-closed v2 owner binding (TASK-05)"
  - "OwnerBinding enum — Owner(String) | Refused"
  - "route_tasks_endpoint's five-case ordered refusal chain"
  - "missing_tasks_declaration_refusal — the -32021 client-declaration refusal"
  - "tests/v2_tasks_owner_binding.rs — 8 live-socket ordering probes"
affects:
  - "114-11 (tasks/get + tasks/cancel v2 response shape) — owner is now bound before those routes"
  - "114-12 (whether a v2 tools/call mints a task at all, DQ1)"
  - "114-15 (live cross-caller proof) — see § For 114-15 below"
  - "114-18 (requirement booking) — MUST carry the TASK-05 qualification"
tech-stack:
  added: []
  patterns:
    - "One identity table per server: the v2 arm CALLS resolve_mrtr_principal, it does not copy its match"
    - "Two-variant OwnerBinding rather than Option<String>, so 'refused' cannot alias 'no backend'"
    - "Refusal codes read from named constants; no numeric error-code literal in task_dispatch.rs"
    - "Ordering proven by requests that satisfy TWO refusal conditions at once"
key-files:
  created:
    - tests/v2_tasks_owner_binding.rs
  modified:
    - src/server/task_dispatch.rs
    - src/server/core.rs
    - src/server/mod.rs
    - tests/v2_tasks_era_gates.rs
    - .planning/phases/114-tasks-extension-migration/114-SPEC-RECHECK.md
    - .planning/phases/114-tasks-extension-migration/deferred-items.md
decisions:
  - "OwnerBinding enum replaces Option<String>: None already meant 'no task backend', so reusing it for 'refused' would make the fail-closed row indistinguishable from a configuration fact at every call site"
  - "ANONYMOUS_PRINCIPAL and MrtrPrincipal lose their streamable-http cfg (now not(wasm32) only) so task_dispatch can REUSE the value instead of spelling a second \"\" literal"
  - "The workflow-continuation recording SKIPS with a warn on the refuse row rather than refusing the whole response — the path is already fire-and-forget and the tool has already run"
  - "tests/v2_tasks_era_gates.rs test 7 now authenticates both legs: it creates on v1 and reads on v2, which only worked while both eras shared the 'local' bucket"
  - "Test 4 does not use the plan's malformed-params probe — measured unobservable; see D-114-F"
metrics:
  duration: "~2h (resumed mid-plan from an interrupted executor)"
  completed: 2026-07-28
  tasks: 3
  commits: 3
  tests_added: 16
---

# Phase 114 Plan 09: v2 Task Owner Binding Summary

Era-aware `resolve_owner` binding a v2 task owner to the authenticated OAuth subject — or
refusing outright — via the SAME `resolve_mrtr_principal` table Phase 113 minted, wrapped in a
five-case ordered refusal chain whose order is proven by four two-condition probes.

## Resume decision: FINISH the in-flight work, not discard it

This plan was resumed from an interrupted executor. Task 1 was committed (`e1836884`); the
working tree held ~404 insertions / 125 deletions of uncommitted Task-2 work that **did not
compile** (4 errors: E0308, E0061 ×3), introducing an `OwnerBinding` enum.

**Decision: (a) finish it.** Made on evidence, before writing any code:

1. **`OwnerBinding` is required by the plan's semantics, not an invention of the interrupted
   run.** Task 2's identity table has a *refuse* outcome. The pre-existing `Option<String>`
   already used `None` to mean "no task backend"; reusing `None` for "refused" would make the
   fail-closed row indistinguishable from a configuration fact at every call site — precisely
   the 114-08 failure mode where two things answer identically and neither is load-bearing for
   any test.
2. **It already satisfied Task 2's acceptance criteria verbatim**: `resolve_mrtr_principal`
   reused as a *function call* (not a copied match), the `ANONYMOUS_PRINCIPAL` cfg widened
   exactly as the plan's action text permits ("widen the cfg or move the constant; do NOT copy
   the literal"), no new `""` literal, the v2 arm never calling `TaskRouter::resolve_owner`,
   and the rustdoc already carrying the D-07 caveat plus the `allow_anonymous: false`
   cross-reference.
3. **All four errors were mechanical call-site migration**, zero design rework.

Discarding would have meant re-deriving the same design from the same plan text. The 404 lines
were kept because they *were* the design the plan calls for — not because they existed.

Migration applied: `ServerCore::resolve_task_owner` (private `fn`, so no semver event) returns
`OwnerBinding` and takes `era`; `build_task_created_response` and `maybe_build_task_created`
take `era`; both `tools/call` create paths capture the already-resolved era before
`protocol_context` is moved.

## What shipped

### The identity table (one per server)

| `authenticated_subject` | `has_auth_provider` | v2 owner |
|---|---|---|
| `Some(sub)` | any | `sub` |
| `None` | `true` | **`OwnerBinding::Refused`** → `-32003` |
| `None` | `false` | `ANONYMOUS_PRINCIPAL` (`""`) |

The v2 arm **is** `crate::server::core::resolve_mrtr_principal` — the same function MRTR calls,
not a second match over the same two inputs. A task record and an MRTR continuation are both
server-held state a later request redeems, so "who may redeem it" has exactly one answer per
server.

**v1 is frozen.** Router delegation and the `"local"` fallback are byte-identical; the only
addition is D-10's migration `tracing::warn!` on the unauthenticated row.
`tests/v1_tasks_golden.rs` unchanged and green on both backends.

The v2 arm never calls `TaskRouter::resolve_owner`. That chain reaches a **session id**
(TASK-05 forbids it outright) and a **`client_id`** (per-application OAuth `azp`, which would
collapse per-user isolation into per-app isolation). Both disqualified sources are named in the
rustdoc so a future "unify the two paths" cleanup has to argue with a written reason.

### The ordered refusal chain

| case | condition | code | where |
|---|---|---|---|
| 1 | method RETIRED on this era | `-32601` | `retired_method` |
| 2 | server has no task backend | `-32601` | per-endpoint handlers (frozen messages) |
| 3 | client never declared the tasks extension | `-32021` | `declares_tasks_extension` |
| 4 | unauthenticated on an auth-configured server | `-32003` | identity table row 2 |
| 5 | the params, finally | — | the `match request` tail |

Cases 3 and 4 are **skipped for a backendless server** (they sit inside the has-backend guard):
such a server advertises no tasks extension, so telling that caller to declare one — or to
authenticate — would send it to fix the wrong thing.

The owner is bound **exactly once**, in `route_tasks_endpoint`, and passed down as `&str`. No
handler resolves a second one.

Neither `-32021` nor `-32003` appears as a numeric literal in `task_dispatch.rs`; all four
textual occurrences of `32021` are rustdoc prose. `error.data.requiredCapabilities` is built by
serializing the real `ClientCapabilities` / `TasksExtensionCapability` types, so the key and the
empty-object value cannot drift from `TASKS_EXTENSION_KEY`.

## For 114-15 (the live cross-caller proof)

Stated precisely so 114-15 need not guess:

**Identity table** — as tabulated above. Inputs are exactly two: `AuthContext::subject` (via
`Option<&AuthContext>`) and `has_auth_provider` (`Server::get_auth_provider` /
`ServerCore::auth_provider`, both `.is_some()`). Nothing else is read. No session id, no
`client_id`, no `clientInfo`, no params.

**Fail-closed rule** — row 2 only. On a server with an auth provider, an unauthenticated v2
`tasks/*` request binds NO owner and is answered `-32003 AUTHENTICATION_REQUIRED` at **HTTP
200**, original id echoed, `result` null, before any store or router is consulted. On a server
with **no auth provider at all**, row 3 applies and every v2 caller shares ONE bucket
(`ANONYMOUS_PRINCIPAL` = `""`) by design.

**Bucket keys are DISJOINT.** v1's unauthenticated owner is `"local"`; v2's is `""`.
`GenericTaskStore::is_anonymous_owner` treats the two *identically* for the `allow_anonymous`
decision, but `make_key` prefixes every record by owner, so they are separate key spaces. These
two facts are easy to conflate and 114-15 should assert them separately. **Measured
consequence:** a task created by an unauthenticated caller on v1 is *not* reachable by an
unauthenticated v2 caller on the same server. This is intended, and it is what forced the
`v2_tasks_era_gates.rs` test-7 fixture change below.

**Per-method reachability of the refusal:** `tasks/get` and `tasks/cancel` reach case 4 on both
eras. `tasks/list` and `tasks/result` never reach it on v2 — case 1 returns first. On v1 no
method reaches it (the v1 arm never refuses).

**Harness posture:** use `AuthPosture::Optional` (`OptionalBearer`). `BearerSubjects` returns
`Err` for a missing token, so the transport answers 401 long before dispatch and case 4 is
unreachable. Every v2 `tasks/*` probe must also DECLARE the tasks extension
(`v2_body_with_client_extensions(..., &[TASKS_EXTENSION_KEY])`) or it will hit case 3 first.

**Not asserted here, still open for 114-15:** two *authenticated* callers cannot see each
other's tasks (this plan proves the refusals; 114-15 proves the isolation), and that two
anonymous callers on a no-auth server *do* share a bucket (row 3, test 8).

## Deviations from Plan

### 1. [Rule 1 — Bug] `v2_tasks_era_gates.rs` test 7 depended on cross-era bucket sharing

- **Found during:** Task 2, after the identity table landed.
- **Issue:** `v2_tasks_get_and_cancel_are_not_gated` created a task on **v1** (owner `"local"`)
  and read it on **v2** (owner `""`), unauthenticated. Before this plan both eras used
  `"local"`, so it worked by accident. With disjoint buckets it got `-32603 task not found` and
  its `!result.is_null()` assertion failed.
- **Assessment:** the disjointness is *intended* — 114-09's own plan text flags `""` vs
  `"local"` as disjoint keys and schedules 114-15 to assert it. So the fixture, not the code,
  was wrong.
- **Fix:** both legs now authenticate as one subject (`FENCE_SUBJECT`) under
  `AuthPosture::Optional`, so v1 and v2 bind the same owner. **Every assertion was kept**,
  including `!result.is_null()`; the fence is if anything sharper. The harness helpers gained
  single-implementation `_as` variants (no duplicated bodies).
- **Commit:** `d0b12d84`

### 2. [Rule 1 — Bug] `v2_tasks_era_gates.rs` probes did not declare the tasks extension

- **Found during:** Task 3, after case 3 landed. `tasks/get`/`tasks/cancel` reached case 3 and
  answered `-32021`.
- **Fix:** `v2_post` and `v2_task_post_as` now declare `TASKS_EXTENSION_KEY`, so that suite
  keeps measuring the retirement it is named for rather than the declaration gate — matching
  the unit-level `context_for` fixture, which already declared.
- **Commit:** `d2594744`

### 3. [Rule 3 — Blocking] The plan's malformed-params ordering probe is unobservable

- **Found during:** Task 3.
- **Measured:** a `tasks/get` with `{"taskId": 12345}` is answered
  `-32601 "Method not found: tasks/get"`, not `-32602`. `ClientRequest` is deserialized at
  **ingress**; a params-shape mismatch makes the variant fail to match and the request never
  reaches `route_tasks_endpoint` at all.
- **Consequence:** the plan's acceptance criterion —
  *"`an_unauthenticated_caller_is_refused_before_the_params_parse` sends malformed params and
  asserts `error.code == -32003` (NOT `-32602`)"* — **cannot be satisfied as written**.
- **Fix:** the test keeps its name and proves the ordering a stronger way: identical
  *well-formed* params sent twice to the same server, differing only in the credential. The
  authenticated leg reaches the store and hears `"task not found"`; the unauthenticated leg
  hears `-32003` and learns nothing about the task's existence. That proves not merely that the
  params were not parsed but that **no store or router was consulted**, which is what T-114-37
  requires, and it doubles as the existence-oracle guard. Leg C pins the measured caveat.
- **Logged:** `D-114-F` in `deferred-items.md` (the `-32601`-for-bad-params behaviour is
  transport-wide and out of this plan's scope).
- **Commit:** `d2594744`

### 4. [Rule 2 — Missing critical functionality] Both `tools/call` create paths bind through the table

- The plan's Task-2 `<files>` named only `task_dispatch.rs`, but `ServerCore`'s router-minted
  task path and the workflow-continuation recording both resolved an owner with
  `.unwrap_or_else(|| "local")`. Left alone, a v2 unauthenticated caller on an auth-configured
  server could still MINT a router-backed task — defeating T-114-37 on the create path.
- Router-minted `tools/call` now returns `-32003` on the refuse row. The fire-and-forget
  workflow-continuation recording **skips with a warn** instead of refusing the whole response:
  that path is already fire-and-forget, the tool has already run, and filing the continuation
  into a bucket the caller was just refused would be worse than not recording it.
- Both are unreachable on v1, where the table never refuses.
- **Commit:** `d0b12d84`

### 5. [Rule 1] Two clippy lints under the gate's pedantic set

`doc_link_code` on the rustdoc table row, and `field_reassign_with_default` on the
`ClientCapabilities` construction. Both fixed; gate re-run green.

## Negative Controls

All four required by the plan were run and reverted.

| # | Change | Test that FAILED | Observed |
|---|---|---|---|
| 1 | Identity table row 2 → `Some(ANONYMOUS_PRINCIPAL)` | `v2_unauthenticated_with_auth_provider_is_refused` | `Owner("")` vs `Refused` |
| 2 | Case 4 hoisted above cases 1–3 | `a_retired_method_answers_32601_even_when_unauthenticated` | `-32003` vs `-32601` |
| 2b | *(same change)* | `a_non_declaring_client_gets_32021_not_32003` | `-32003` vs `-32021` |
| 3 | Case 5 hoisted above case 4 | `an_unauthenticated_caller_is_refused_before_the_params_parse` | `-32603 "task not found"` vs `-32003` |

Control 3 is the ordering proof the plan asked for, adapted to the measured reality of
deviation 3: the failure mode it catches is *the store was consulted for a caller who should
have been refused first*, which is the actual threat.

Each control was reverted and the suite re-run green before proceeding.

## Verification — commands run and exit codes

| Command | Exit | Result |
|---|---|---|
| `make quality-gate` | **0** | 4694 passed / 0 failed / 80 ignored across 270 result lines (baseline 4670/0/80 across 267; +24 = 16 new tests, lib suite counted twice) |
| `cargo nextest run --features full` | 0 | 2420 passed, 0 failed, 2 skipped (pre-Task-3 checkpoint) |
| `cargo nextest run --features full -E 'binary_id(pmcp::v2_tasks_owner_binding)'` | 0 | 8 tests, all pass |
| `cargo nextest run --features full -E 'binary_id(pmcp::v1_tasks_golden) or binary_id(pmcp::v2_tasks_era_gates)'` | 0 | 22 tests, all pass — v1 bytes frozen |
| `cargo test --lib --features full owner_binding_tests` | 0 | 8 unit tests, all pass |
| `make wasm-build` | **0** | 93 warnings, **zero** naming any symbol this plan touched; no new warning class |
| `pmat analyze complexity --format json --max-cognitive 25` | 0 | `.summary.violations` = **4**, all pre-existing, all under `crates/` (mcp-tester, pmcp-server-toolkit, pmcp-agent). **None in `src/`**, none in `task_dispatch.rs`/`core.rs`/`mod.rs` |
| `cargo semver-checks check-release --baseline-rev 27364eb1 --only-explicit-features --features full` | **0** | "no semver update required"; 223 checks pass, 30 skip |

`--baseline-rev` was used per the known inherited crates.io-baseline failure from 113.1-03.
`nextest -E` used `binary_id(...)`, never `test(/…/)`.

**`resolve_task_owner` is a private `fn`**, and `OwnerBinding` / `route_tasks_endpoint` /
`resolve_mrtr_principal` / `MrtrPrincipal` / `ANONYMOUS_PRINCIPAL` are all `pub(crate)` — so the
`Option<String>` → `OwnerBinding` change is **not** a public-API event. Confirmed by
semver-checks.

## Constraint compliance

- `.planning/REQUIREMENTS.md` — **0-byte diff**, untouched. `requirements mark-complete` NOT run.
- No `tasks/*` row added to `MRTR_METHODS`.
- `src/server/streamable_http_server.rs` — **untouched by this plan** (`git diff e1836884 --stat`
  empty); `v2_status_for_code` unmodified, so `-32003` answers at HTTP 200 deliberately.
- No packages installed; `Cargo.toml` / `Cargo.lock` byte-unchanged.
- Row 23 (`own_reserved_result_fields` deleting `inputRequests`) left for 114-10.

## Wasm

`src/server/task_dispatch.rs` is declared `#[cfg(not(target_arch = "wasm32"))]` in
`src/server/mod.rs:104`. **The wasm path does not reach tasks at all** — there is no wasm mirror
of this logic to keep in sync. `ANONYMOUS_PRINCIPAL`'s cfg widened from
`all(streamable-http, not(wasm32))` to `not(wasm32)`, which adds it to non-wasm builds *without*
`streamable-http` and changes nothing on wasm.

## Known Stubs

None. No hardcoded empty values, placeholders or unwired data paths were introduced.

## Threat Flags

None. Every file touched is inside the plan's declared `<threat_model>` surface; no new network
endpoint, auth path, file access pattern or schema change at a trust boundary was introduced.

## Deferred

- **D-114-F** (new, logged): a known method with malformed params answers `-32601`, not
  `-32602`, because `ClientRequest` deserialization happens at ingress. Transport-wide, not
  tasks-specific. Suggested owner: a conformance/hardening plan, or whichever plan next touches
  `ClientRequest`'s deserialization.
- Pre-existing and untouched: D-114-A, D-114-B, D-114-C, D-114-E, D-113-U.

## Commits

| Hash | Message |
|---|---|
| `e1836884` | `refactor(114-09): thread the identity inputs into TaskDispatch at both sites` (Task 1, pre-existing) |
| `d0b12d84` | `feat(114-09): era-aware resolve_owner implementing the three-row identity table` (Task 2) |
| `d2594744` | `feat(114-09): the two refusals, in the required order` (Task 3) |

## Self-Check: PASSED

All 8 claimed files exist on disk; all 3 claimed commit hashes resolve in `git log`; the four
load-bearing symbols/records (`OwnerBinding`, `missing_tasks_declaration_refusal`, the
SPEC-RECHECK TASK-05/D-07 row, `D-114-F`) are present.
