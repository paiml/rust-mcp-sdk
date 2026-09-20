---
phase: 114-tasks-extension-migration
plan: 06
subsystem: api
tags: [mcp, tasks-extension, client, negotiation, streamable-http, routing-headers, mrtr]

requires:
  - phase: 114-03
    provides: "ClientCapabilities.extensions, TASKS_EXTENSION_KEY, TasksExtensionCapability — the types a client declaration serializes from"
  - phase: 114-05
    provides: "the server half: apply_tasks_capability_rule advertises the extension and project_capabilities_for_v1 strips it from the v1 wire"
  - phase: 113
    provides: "the era-aware assert_capability escape hatch, frame_routing_pair / encode_header_value, MRTR_METHODS and splice_mrtr_params"
provides:
  - "ClientBuilder::with_tasks_extension() — the per-request v2 extension declaration"
  - "An era-split `tasks` arm in assert_capability: extensions[TASKS_EXTENSION_KEY] on v2, capabilities.tasks on v1"
  - "TASK_NAME_BEARING_METHODS + name_bearing_key — the SEPARATE tasks routing-name table"
  - "Mcp-Name = params.taskId on tasks/get|update|cancel from pmcp's client emitter (spec MUST, inventory row 34)"
  - "pmcp::testing::{routing_name_key, method_is_mrtr_eligible} — the pub(crate) table bridge for integration tests"
  - "tests/v2_tasks_client.rs — 10 client-half tests over a raw-TCP capture socket"
affects: [114-07, 114-09, 114-10, 114-12, 114-13, 114-14, 117, 118]

tech-stack:
  added: []
  patterns:
    - "Two method tables, one lookup: MRTR eligibility and routing-name location are separate properties and are read from separate tables, joined only by name_bearing_key"
    - "Raw-TCP capture socket as the client-emitter test fixture — measures the bytes and headers the client actually sends, with no server gate able to reject them first"
    - "pmcp::testing thin wrappers as the E0365 bridge for pub(crate) protocol tables"

key-files:
  created:
    - tests/v2_tasks_client.rs
  modified:
    - src/client/mod.rs
    - src/types/mrtr.rs
    - src/shared/streamable_http.rs
    - src/testing/mod.rs
    - .planning/phases/114-tasks-extension-migration/deferred-items.md

key-decisions:
  - "The v2 tasks capability check is PRESENCE (contains_key), not `{}`-equality — refusing a server that advertised support with an unexpected value would be the mirror image of the over-removal 114-05's v1 projection deliberately avoids"
  - "with_tasks_extension() is inert on v1: the declaration is never injected into `initialize`, so no existing caller's handshake bytes move (D-02)"
  - "frame_routing_pair (shared by client emitter AND server body reader) now resolves through name_bearing_key; the widening is inert server-side because cross_check_name short-circuits on is_name_bearing_method, which still reads logical_name_key"
  - "Server-side Mcp-Name enforcement for tasks/* stays OFF this phase and is recorded as D-114-C for Phase 118 — turning it on is a BREAKING change for clients still sending the empty value"
  - "No new error variant: the fail-fast refusal reuses Error::capability and names the extension key in its message (T-114-23)"

patterns-established:
  - "Negative control as a table trap demo: adding a tasks row to MRTR_METHODS fails exactly 3 tests (the Phase-113 lock, the decoupling test, the disjointness test) while 2 orthogonal tests stay green"
  - "splice_mrtr_params_would_delete_a_tasks_update_payload — the mechanical consequence of the trap is pinned as a permanent test next to the tables that decide who is subject to it"

requirements-completed: []

duration: 118min
completed: 2026-07-28
---

# Phase 114 Plan 06: v2 Tasks Client Negotiation Summary

**A pmcp v2 client can now declare the tasks extension on every request, refuses an un-negotiated `tasks/*` call locally with zero bytes on the wire, and sets the spec's `Mcp-Name: <taskId>` routing header — through a SECOND name-key table that keeps `tasks/update` out of the MRTR pipeline that would delete its payload.**

## Performance

- **Duration:** ~118 min
- **Tasks:** 3 of 3 (plus one Rule-1 auto-fix)
- **Files modified:** 4 source files + 1 new test file + 1 planning file
- **Diff:** `+1297 / -16` across `src/` and `tests/` (`542b8096~1..HEAD`); `Cargo.toml` / `Cargo.lock` **byte-unchanged**, zero packages installed (T-114-SC)

## Accomplishments

### The client half of D-04 is real, and the emission mechanism was MEASURED rather than assumed

Task 1's acceptance criterion asked, by measurement, whether the v2 `_meta` emission serializes the `ClientCapabilities` struct or hand-builds a `json!` object. **Measured: it serializes the struct** — `Client::v2_request_meta` does `serde_json::to_value(self.v2_client_capabilities())` and inserts the result under `io.modelcontextprotocol/clientCapabilities`. So 114-03's new `extensions` field was already on the wire path and **no rewrite was needed**; the work was providing the way to SET it. That is recorded as a fact, not a guess, and it is additionally pinned by a test (`the_emitted_capabilities_deserialize_back_into_client_capabilities`) that round-trips the emitted value back through `serde_json::from_value::<ClientCapabilities>` — so a future hand-built `json!` object, which would silently drop any newly added field, fails.

`ClientBuilder::with_tasks_extension()` inserts `TASKS_EXTENSION_KEY → {}` into a new `declared_extensions` map that `v2_client_capabilities` **merges** (not assigns) into the capabilities it builds. Merge rather than assign because `ClientCapabilities::default()` carries `extensions: None` *today* — an assignment would silently discard a future default that pre-seeded the map.

**The declaration is deliberately v1-inert.** v1 advertises the `ClientCapabilities` the caller passed to `initialize`, so injecting there would move the `initialize` bytes of every existing caller — the D-02 lock, and exactly the "more complete"-looking fix that 114-03 already had to refuse once for `ClientCapabilities::full()`. A test (`the_declaration_never_reaches_a_v1_initialize`) builds a `with_tasks_extension()` client with **no** protocol-version selection and asserts the serialized v1 capabilities contain no `extensions` key at all.

**Absence is asserted as key ABSENCE, in both the unit and the integration test, and this is a measurement inherited from 114-03's negative control**: the field carries `skip_serializing_if = "Option::is_none"`, so a regression emitting `"extensions": null` is exactly the falsy shape a value-based assertion would accept. Presence is asserted as **equality with `{}`**, not as `is_some()`, for the same reason 114-05 gave for the server side: a presence-only assertion passes on precisely the change that would break it.

### `assert_capability`'s `tasks` arm is era-SPLIT, and the non-vacuity guard is the interesting test

On v2 the capability is satisfied iff `server_capabilities.extensions` contains `TASKS_EXTENSION_KEY`; on v1 it still reads `capabilities.tasks`. **Reading `capabilities.tasks` on v2 would refuse every conformant v2 server**, because `core::project_capabilities_for_v2` strips that field from the `server/discover` projection — that is the whole point of 114-05's projection, and this arm is its client-side counterpart. The test that proves the arm is not a no-op is `v2_tasks_capability_ignores_the_v1_tasks_field`: a v2 client with `capabilities.tasks = Some(..)` and no extensions entry must **fail**. That is the fixture that passed under the old arm.

`v1_tasks_capability_still_gates_on_the_tasks_field` asserts the separation in **both** directions — v1 with no tasks field fails, v1 with the field passes, and v1 presented with the *v2* spelling (an extensions entry) still fails. The two eras' spellings do not leak into each other in either direction.

**The escape hatch was not narrowed.** A v2 client that never called `server_discover` still passes: it has no basis to refuse, and the server refuses authoritatively. The `_ =>` arm's `debug_assert!` tripwire is untouched — `grep -c 'debug_assert' src/client/mod.rs` is **1** at `HEAD` and **1** at the plan's start commit.

**"Fails fast" is measured, not asserted.** Two separate tests count transport sends: the unit test uses `ModeRecordingTransport` (typed sends **0**, raw bodies **empty**), and the integration test uses a `DiscoverTransport` whose counter is **zeroed after** a real `server_discover` round trip, so the assertion is `0` sends *for the refused call specifically* rather than `1` for the whole test. Its non-vacuity twin (`a_negotiated_tasks_call_passes_the_gate_and_reaches_the_transport`) drives the SAME call against a projection that DOES carry the extension and asserts the send count is **1** and that the error is not the capability refusal — without it, a `tasks_get` that failed for any other reason would satisfy the first test.

No new error variant was introduced (that would be public-API churn for no benefit). `Error::capability` is reused and the message names the key. The non-tasks message is byte-identical to before — the only change is `format!("... {} ...", capability, method)` → the inline-arg form, which produces the same string.

### The routing header, via a SECOND table — and the trap is demonstrated, not just described

`TASK_NAME_BEARING_METHODS: [(&str, &str); 3]` maps `tasks/get` / `tasks/update` / `tasks/cancel` to `taskId` (spelled once as `TASK_ID_KEY` so the three rows cannot disagree). `name_bearing_key` consults `logical_name_key` first, then that table. `frame_routing_pair` — the ONE shared reader the client emits headers from and the server reads bodies with — now resolves through `name_bearing_key`, so the header and the body still come from the same bytes (T-113-08 / T-114-20) and cannot desync.

**`mrtr_eligible` still reads `MRTR_METHODS` and ONLY `MRTR_METHODS`.** Both tables carry rustdoc saying why there are two, and `MRTR_METHODS`' doc ends with a literal **"Do not add a tasks row here."**

**The negative control (required by the plan's acceptance criteria) was run and reverted.** With a `tasks/update` row added to `MRTR_METHODS` (arity 3 → 4):

| Test | Result under the control |
|------|--------------------------|
| `mrtr_eligible_is_exactly_three_methods` (Phase 113 lock) | **FAIL** |
| `tasks_methods_are_name_bearing_but_not_mrtr_eligible` | **FAIL** |
| `the_two_name_key_tables_are_disjoint` | **FAIL** (`tasks/update must not also live in MRTR_METHODS`) |
| `logical_name_key_table` | PASS |
| `tasks_list_and_result_are_not_name_bearing` | PASS |

The two PASSes are the evidence that the control is **attributable** rather than indiscriminate. Reverted from a byte-for-byte backup; `shasum -a 256 -c` reports `src/types/mrtr.rs: OK`. **`git stash` was not used at any point.**

**A permanent test now pins the mechanical half of the trap.** `splice_mrtr_params_would_delete_a_tasks_update_payload` shows that `splice_mrtr_params` removes `inputResponses` **unconditionally** — the method gate is upstream, in `mrtr_eligible` — so the moment a tasks row entered `MRTR_METHODS`, `tasks/update`'s entire payload would be stripped in flight. Describing the trap in a comment leaves the consequence unverified; this test keeps it visible next to the tables that decide who is subject to it.

**The Phase-113 lock and the table body are byte-identical to `HEAD`,** verified block-by-block rather than by eyeballing the diff: an `awk`-extracted `MRTR_METHODS` block and `mrtr_eligible_is_exactly_three_methods` body from `git show HEAD:src/types/mrtr.rs` both `diff` clean against the working tree (`IDENTICAL`). The plan's stronger requirement (zero CHANGED lines inside them) holds: the only hunk near the table is `@@ -124,0 +125,18 @@` — a pure insertion of rustdoc **above** the const declaration.

`tasks/list` and `tasks/result` are deliberately absent from the table, asserted by name at unit level and — more usefully — behaviourally: `tasks_list_emits_an_empty_mcp_name` drives a real `tasks_list` through the real transport and asserts `Mcp-Name: ""`. That is the assertion that distinguishes this design from the plausible wrong one ("every `tasks/*` method is name-bearing").

### `tests/v2_tasks_client.rs`: a raw-TCP capture socket, and why not a real server

561 lines, 10 tests. Four of them point a **real `StreamableHttpTransport`** at a socket that records the request head and body verbatim and answers a JSON-RPC `-32601` echoing the request id.

The fixture choice is deliberate and is written into the file's module doc: `tasks/*` is **not routed on the v2 wire yet** (TASK-03 — a v2 `tasks/result` answers `-32601` by design since 113-29), so a real `pmcp` server would reject the request before the interesting bytes could be observed. The capture socket has no gate. Echoing the id (rather than answering `204`) matters for a measured reason: it lets the client's in-flight request resolve promptly, so the tests measure emitted bytes instead of a receive timeout.

The four HTTP tests assert what a conformance run will grade: `Mcp-Name: abc` for `tasks/get` **and** `params.taskId == "abc"` in the same captured frame (header/body derived from one source), `Mcp-Name: abc` for `tasks/cancel`, `Mcp-Name: ""` for `tasks/list`, the `{}` declaration on **two successive** requests of **different methods**, and key-absence for a non-declaring client with the `_meta` envelope itself asserted present so the absence check cannot pass for the wrong reason.

The three table tests reach the two `pub(crate)` tables through **new thin wrappers in `pmcp::testing`** — the established E0365 bridge in that module (`encode_mcp_name`'s rustdoc records the same reason, and that wrapper exists because a hand-copied mirror had already drifted).

## Task Commits

1. **Task 1: Per-request client extension declaration** — `542b8096` (feat)
2. **Task 2: Era-aware `assert_capability` tasks arm (D-04)** — `d27120dd` (feat)
3. **Task 3: Tasks routing header with the tables kept decoupled** — `00225315` (feat)
4. **Rule-1 auto-fix: PMAT cog-25 cap** — `c878591e` (refactor)

## Files Created/Modified

- `tests/v2_tasks_client.rs` **(created, 561 lines)** — the client-half suite: raw-TCP capture server, `DiscoverTransport`, 10 tests
- `src/client/mod.rs` — `declared_extensions` on `Client` and `ClientBuilder`; `with_tasks_extension()`; the `extensions` merge in `v2_client_capabilities`; the era-split `tasks` arm plus `tasks_capability_satisfied_by` and `unsupported_capability_message`; 10 new unit tests
- `src/types/mrtr.rs` — `TASKS_GET_METHOD` / `TASKS_UPDATE_METHOD` / `TASKS_CANCEL_METHOD` / `TASK_ID_KEY` / `TASK_NAME_BEARING_METHODS`; `name_bearing_key`; `logical_name_of` re-pointed at it; the two-table rustdoc at both ends; 6 new unit tests
- `src/shared/streamable_http.rs` — `v2_routing_headers` rustdoc updated to the combined lookup; 2 new emitter tests
- `src/testing/mod.rs` — `routing_name_key`, `method_is_mrtr_eligible`
- `.planning/phases/114-tasks-extension-migration/deferred-items.md` — **D-114-C** added

## Decisions Made

1. **Presence, not `{}`-equality, for the v2 capability check.** `contains_key` is what the negotiation rule tests. An operator may configure a richer value, and refusing to CALL a server that advertised support with an unexpected value would be the mirror image of the over-removal 114-05's v1 projection deliberately avoids. The reason is in the helper's rustdoc so it is not "tightened" later.
2. **`frame_routing_pair` widened rather than forked.** The alternative — a second routing-pair reader for the client — would recreate exactly the two-halves-drift that `frame_routing_pair` exists to prevent (T-113-08). The widening was verified inert on the server: `method_and_name_of` has exactly one consumer (`classify_v2_request`), whose `body_name` feeds `cross_check_name`, which returns `Ok(())` for a non-`logical_name_key` method before comparing anything. The outbound echo uses the *header* value from `require_three_headers`, not the body name, so it is unaffected.
3. **Server-side enforcement stays off (D-114-C).** Named as a tradeoff in `TASK_NAME_BEARING_METHODS`' rustdoc rather than left implicit: a pmcp server accepts BOTH a conformant `Mcp-Name: <taskId>` and a legacy empty value, and does not detect a header disagreeing with the body. Turning it on is one predicate (`is_name_bearing_method` → `name_bearing_key`) and a BREAKING change for non-conformant clients.
4. **`TASK_ID_KEY` spelled once.** Three rows repeating the literal `"taskId"` is three chances to disagree, and inventory row 34's key is exactly the kind of value the D-18 hold may move.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Quality gate regression] `capture_server` exceeded PMAT's cognitive-complexity cap**

- **Found during:** Task 3 verification (`pmat analyze complexity --max-cognitive 25`)
- **Issue:** The new capture server carried the accept loop AND the `Content-Length` framing loop in one function: **cognitive 28 vs. the cap of 25**. `CLAUDE.md` makes this a **PR-blocking** CI check ("PRs are blocked if PMAT detects new cognitive-complexity violations"), and the violation was unambiguously introduced by this plan — the tree went from the inherited 4 violations to 5.
- **Fix:** Split into `capture_server` (accept loop), `serve_one` (record + reply) and `read_one_request` (framing), with the reason written at `serve_one`.
- **Files modified:** `tests/v2_tasks_client.rs`
- **Verification:** `pmat analyze complexity --max-cognitive 25` back to **4 violations, all pre-existing in `crates/**/tests/`, 0 in `src/`, 0 in this plan's files**. All 10 tests still pass.
- **Committed in:** `c878591e`
- **Note on the query shape:** `violations` lives under `summary`, **not** at top level. A naive top-level query returns "0 violations" vacuously — this plan's first query used the correct path and is the reason the violation was seen at all.

**2. [Rule 3 — Blocking issue] One file beyond the plan's declared `files_modified`: `src/testing/mod.rs`**

- **Found during:** Task 3 (writing `tests/v2_tasks_client.rs`)
- **Issue:** The plan's `must_haves` require `tests/v2_tasks_client.rs` to provide the **decoupling** property. Both method tables and `mrtr_eligible` / `name_bearing_key` are `pub(crate)` (Phase-113 D-10), and a `pub use` of a `pub(crate)` item does not compile (E0365) — so the property was unstatable from an integration test.
- **Fix:** Two thin `pub fn` wrappers in `pmcp::testing`, following the precedent already established in that file by `encode_mcp_name` (whose rustdoc records that a hand-copied mirror of a `pub(crate)` codec had already drifted).
- **Files modified:** `src/testing/mod.rs`
- **Verification:** `cargo public-api --features full diff` shows them as **Added**, with **Removed: (none)** and **Changed: (none)**; `cargo semver-checks --baseline-rev 27364eb1` **223/223 no update required**.
- **Committed in:** `00225315`

**3. [Rule 3 — Plan-text correction] The plan's own type name was wrong**

- **Found during:** Task 2 (test authoring)
- **Issue:** The tasks capability type on `ServerCapabilities` is `ServerTasksCapability`, not `TaskCapabilities`. `ServerCapabilities` is also `#[non_exhaustive]`, so the integration test cannot build one with a struct expression (E0639) and uses field assignment instead — recorded in the fixture's own rustdoc so it is not "cleaned up" back into a struct literal.
- **Files modified:** `src/client/mod.rs`, `tests/v2_tasks_client.rs`
- **Committed in:** `d27120dd`, `00225315`

---

**Total deviations:** 3 auto-fixed (1 × Rule 1, 2 × Rule 3). **Impact:** none on scope. The Rule-1 fix is required to keep the PR-blocking gate green; the two Rule-3 fixes are the minimum needed to satisfy the plan's own `must_haves`.

## Verification — verbatim, only what was actually run

| Check | Command | Result |
|-------|---------|--------|
| Aggregate gate | `/usr/bin/make quality-gate` (detached + awaited) | **exit 0** — **264** `test result:` lines, **4649 passed, 0 failed**, **0** truncation markers, **0** `FAILED` |
| Full test sweep | `/usr/bin/make test-all` | **exit 0** — 172 lines, **2806 passed, 0 failed**, 0 truncation markers |
| ALWAYS requirements | `/usr/bin/make validate-always` | **exit 0** — 87 lines, **1713 passed, 0 failed**, 0 truncation markers |
| Lint (CI-equivalent) | `/usr/bin/make lint` | **exit 0**, four times (after each task and after the auto-fix) |
| Format | `cargo fmt --all -- --check` | **exit 0** |
| Semver | `cargo semver-checks check-release --baseline-rev 27364eb1` | **223 checks: 223 pass, 30 skip — no semver update required** |
| Public API | `cargo public-api --features full diff HEAD~4..HEAD` | Removed **(none)**, Changed **(none)**, Added: `ClientBuilder::with_tasks_extension` (×2 re-export paths), `pmcp::testing::routing_name_key`, `pmcp::testing::method_is_mrtr_eligible` |
| Complexity | `pmat analyze complexity --max-cognitive 25` (queried at `summary.violations`) | **4** violations, **0 in `src/`**, **0 in this plan's files** — the inherited set |
| wasm | `/usr/bin/make wasm-build` | **exit 0**, 93 warning lines; exactly **6 distinct** warnings name symbols introduced here, all the pre-existing D-14 dead-code-on-wasm32 class |
| Phase-113 MRTR | `cargo nextest run --features full -E 'binary(/mrtr/)'` | **46/46 passed** — no Phase-113 regression |
| This plan's suites | `cargo nextest run --features full --test v2_tasks_client` | **10/10 passed** |
| Cargo manifests | `git diff --stat -- Cargo.toml Cargo.lock` | **empty** (T-114-SC: zero packages installed) |
| Requirements lock | `git status --short -- .planning/REQUIREMENTS.md` | **empty** — untouched |

### The test-count arithmetic reconciles EXACTLY against 114-05's recorded baselines

114-05 measured `test-all` at **2777** and `validate-always` at **1695**. This plan adds **18** lib tests (4 + 6 in `src/client/mod.rs`, 6 in `src/types/mrtr.rs`, 2 in `src/shared/streamable_http.rs`), **10** integration tests and **1** doctest (`with_tasks_extension`):

- `validate-always`: 1695 + 18 = **1713** ✓ (independently confirmed — `cargo nextest list --lib` reports 1713)
- `test-all`: 2777 + 18 + 10 + 1 = **2806** ✓ (measured composition: the lib binary runs once with tests and once filtered to 0; doctests 486 in one pass, 0 in the other; `v2_tasks_client` 10 in one pass, 0 in the other)
- The `+1` / `+2` result-LINE deltas are the new `v2_tasks_client` binary appearing in each sweep.

No reconciliation is claimed for the aggregate gate's **264 / 4649**: 114-05's own gate log was truncated by RTK so its count is unavailable, and 114-04's 258 / 4576 predates 114-05's own additions.

### Counting caveats honoured (inherited from 114-03/04/05)

- The aggregate gate was **killed by the environment** when run in the foreground and completed only when launched **detached with an exit-code marker file and polled**. It ran to completion this way.
- Every count above was taken with **`/usr/bin/make`, `/usr/bin/git`, `/usr/bin/grep`** — absolute paths — because RTK's proxy has been measured to truncate logs and to make `git diff | grep -c '^-[^-]'` report 0 for a diff with 28 deletions. This plan's log had **0 truncation markers**, checked explicitly.
- All **16** deletion lines across `src/` and `tests/` were enumerated individually: 5 are the `assert_capability` arm and its error constructor, 11 are rustdoc lines replaced with wider text plus the one-line `logical_name_of` resolution change. Nothing was lost.
- The wasm baseline of "86 warnings" comes from 114-03/114-04 and was **not re-measured at 114-05's HEAD**, so no strict delta is claimed. What IS measured: exactly 6 distinct warnings name symbols this plan introduced (`TASKS_GET_METHOD`, `TASKS_UPDATE_METHOD`, `TASKS_CANCEL_METHOD`, `TASK_ID_KEY`, `TASK_NAME_BEARING_METHODS`, `name_bearing_key`), and every one is the same dead-code-on-wasm32 class that `MRTR_METHODS`, `mrtr_row` and `logical_name_key` already warn with — the whole MRTR module is unused on wasm32.

## Issues Encountered

- **The plan's `MRTR_METHODS`-row trap did not need overriding.** The plan text already prescribed the separate-table shape, so the phase-context warning ("if the plan appears to ask for the `MRTR_METHODS` row, treat that as a plan defect") was not triggered. No `tasks/*` row exists in `MRTR_METHODS` at `HEAD`.
- **The integration tests initially took ~6.4 s each** because the capture server answered `204 No Content` and the client then blocked on a receive that never resolved. Fixed by echoing the request id in a JSON-RPC `-32601`; a single test now runs in **1.15 s**.
- **`ServerCapabilities` is `#[non_exhaustive]`**, so the integration fixture cannot use a struct literal (E0639). Handled with field assignment and a rustdoc note.

## Constraints Honoured

- **`.planning/REQUIREMENTS.md` is untouched (0-byte diff).** TASK-01 and TASK-02 are now IMPLEMENTED on the client side but stay `[~]`, and `requirements mark-complete` was deliberately **NOT** run: `114-SPEC-RECHECK.md` flips TASK-01..06 as a **GROUP** and only on a `PUBLISHED-CONFIRMED` landing, and `## Verdict` is still `PENDING`.
- **No contract YAML** and no edit under `contracts/` — 114-20's option-b waiver.
- **Row 23 was not designed around.** Nothing here depends on a v2 `tasks/get` result shape; `own_reserved_result_fields`' silent deletion of `inputRequests` remains **114-10's**.
- **`git stash` was never used.** The one negative control was reverted from a byte-for-byte file copy, verified with `shasum -a 256 -c`.

## Next Phase Readiness

**Ready.** 114-07 (`GenericTaskStore<B>` input delivery), 114-09, 114-12 and 114-13 are unblocked on the client seam. Notes for whoever comes next:

- **114-12 (DQ1, the v2 create trigger)** now has its precondition: the client's declaration arrives at the server in `params._meta["io.modelcontextprotocol/clientCapabilities"].extensions`, which `ProtocolContext::client_capabilities` already deserializes. Read it from there — do not add a second path.
- **114-13 (`tasks/update` routing)** must route via `InternalClientRequest` (`ClientRequest` is not `#[non_exhaustive]`, PATTERNS Fact 1) and must **not** add a `MRTR_METHODS` row. `TASK_NAME_BEARING_METHODS` already supplies its routing name; nothing further is needed for the header.
- **114-14 (`tasks/update` decode)** inherits `Mcp-Name = taskId` on the wire but **must not** rely on the server cross-checking it — see D-114-C.
- **Phase 118 (conformance)** owns D-114-C: turning on server-side `Mcp-Name` enforcement for `tasks/*` is a one-predicate change (`is_name_bearing_method` → `name_bearing_key`) and a BREAKING change for clients still sending the empty value.
- **Phase 117 (agent task polling)** gets the clean precondition D-04 was for: an agent that calls `tasks_get` against a server that never advertised the extension now gets a typed local refusal naming the key, instead of an opaque round trip.
- **Still open and unowned:** **D-113-U** (`write_canonical` cognitive 26 vs. the PR-blocking cap of 25) — this plan did not touch that file, and `pmat` did not report it in `src/` this run (consistent with 114-05's observation, still uninvestigated). **D-114-A** and **D-114-B** remain as recorded.

---
*Phase: 114-tasks-extension-migration*
*Completed: 2026-07-28*

## Self-Check: PASSED

All claimed artifacts exist on disk (`tests/v2_tasks_client.rs`, `114-06-SUMMARY.md`, and the
four modified source files) and all five commits (`542b8096`, `d27120dd`, `00225315`,
`c878591e`, `18bfa6c2`) resolve in `git log --oneline --all`. Symbol greps confirm the
must-have `contains` claims: `with_tasks_extension` (9 hits in `src/client/mod.rs`),
`TASKS_EXTENSION_KEY` present in `src/client/mod.rs`, `TASK_NAME_BEARING_METHODS` (10) and
`taskId` (10) in `src/types/mrtr.rs`.
