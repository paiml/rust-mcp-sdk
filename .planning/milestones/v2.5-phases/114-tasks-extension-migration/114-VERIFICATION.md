---
phase: 114-tasks-extension-migration
verified: 2026-08-01T02:00:00Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
---

# Phase 114: Tasks Extension Migration Verification Report

**Phase Goal:** Tasks become a v2 extension — a wire-API reshape behind the proven
`serde_json::Value` `TaskRouter` boundary, not a storage rewrite — while v1 Tasks stay fully
functional, all backends survive unchanged, and stateless v2 owner-binding fails closed (the
critical no-session cross-caller-leak guard).
**Verified:** 2026-08-01T02:00:00Z
**Status:** passed
**Re-verification:** No — initial verification

## A note on the `[~]` booking (read before the tables below)

This report verifies the phase goal **on the merits** — actual codebase behavior — not the
REQUIREMENTS.md status marker. TASK-01..06 are deliberately booked `[~]` (*implemented; pending
final schema*) under the recorded **D-18** hold, and `114-SPEC-RECHECK.md`'s `## Verdict` is
`PENDING`. That hold is a **publication-trigger gate** (waiting on `modelcontextprotocol/ext-tasks`
to leave `draft/`), re-measured 2026-08-01 and still `STILL-ABSENT` on that one repository — it is
not an implementation gap, and this verifier does not treat it as one. Every truth below was
checked against running code and passing tests, independent of that marker.

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Tasks negotiated on v2 via extensions map (`io.modelcontextprotocol/tasks`) while v1 `experimental.tasks` continues (TASK-01) | VERIFIED | `src/types/capabilities.rs` defines `TASKS_EXTENSION_KEY`/`TasksExtensionCapability`; `tests/v2_tasks_negotiation.rs` 6/6 pass (`v2_tasks_extension_advertised`, `v1_initialize_stays_byte_identical`, `v2_discover_omits_the_v1_tasks_keys`); live-run `s51_v2_tasks_agent` step [1] printed `extensions: ["io.modelcontextprotocol/tasks"]` |
| 2 | `tasks/update` input delivery; v2 task-augmented results `resultType:"task"` with `CreateTaskResult{taskId,status,ttlMs,pollIntervalMs}`; v1 5-state machine maps deterministically to v2 status enum (TASK-02, TASK-04) | VERIFIED | `tests/v2_tasks_update.rs` 17/17 pass; `tests/v2_tasks_shapes.rs` 13/13 pass incl. `task_status_wire_strings_match_the_extension_schema` and `v2_create_task_result_is_flat_and_carries_all_required_fields`; `crates/pmcp-tasks/tests/input_delivery.rs` 29/29 pass; live-run agent step [2]/[3] created a task, was asked for input, delivered it via `tasks/update`, and read the terminal result |
| 3 | `tasks/list` and blocking `tasks/result` era-gated off on v2, fully functional on v1 (TASK-03) | VERIFIED | `tests/v2_tasks_era_gates.rs` 8/8 pass (`v2_tasks_list_is_gated`, `v2_tasks_result_is_gated`, `v1_tasks_list_still_serves`, `v1_tasks_result_still_serves_pending_minus_32002`); unit test `server::task_dispatch_tests::tasks_dispatch_shared::pending_tasks_result_preserves_minus_32002` passes; live-run agent step [5] got `-32601` from the wire for both methods on v2 |
| 4 | v2 owner binding requires OAuth `sub` / stable per-request identity and fails closed (no session-id fallback); security test proves no cross-caller visibility (TASK-05) | VERIFIED | `tests/v2_tasks_owner_binding.rs` 8/8 pass (ordered refusal chain, `-32021` vs `-32003`, v1 `"local"` freeze); `tests/v2_tasks_security.rs` 8/8 pass — live-socket two-principal matrix over `tasks/get`/`update`/`cancel`, indistinguishable-from-absent refusals, disjoint v1/v2 buckets |
| 5 | `TaskStore` trait/state machine/DynamoDB/Redis/in-memory backends unchanged — reshape not rewrite — verified by v1 storage/tasks suite staying green (TASK-06) | VERIFIED | `crates/pmcp-tasks` unit+integration suite (`--features dynamodb,redis`, no live infra) 288+ tests pass, 0 failed; root-crate `server::task_store` 57/57 and `server::task_dispatch` 58/58 unit tests pass; `tests/v1_tasks_golden.rs` 14/14 pass (byte-identity fixtures for all v1 tasks surfaces, store- and router-backed) |

**Score:** 5/5 truths verified

### Deferred Items

Items not currently gaps for Phase 114 because they are explicitly owned by a later milestone
phase.

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | `D-114-P`: a `TaskRouter`-backed (not store-backed) v2 server answers `-32603` instead of the extension's `-32602` MUST on `tasks/get` for a task the router cannot find (3 named fall-through legs, pinned by count in `tests/v2_tasks_tripwires.rs`) | Phase 118 | Phase 118 goal: "the official conformance suite... runs against whatever the dual-version binary actually does" (CONF-01/02); `deferred-items.md` names Phase 118 as suggested owner "alongside the conformance run that would grade `tasks/get` on a router-backed server" |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `schema/vendored/ext-tasks/schema.ts` | Pinned v2 tasks TS schema | ✓ VERIFIED | 374 lines; `PROVENANCE.md` names commit `2c1425d9a288b9b1f489430fe1e00bb392b47e48` |
| `schema/vendored/ext-tasks/schema.json` | Generated JSON Schema | ✓ VERIFIED | 1834 lines |
| `schema/vendored/ext-tasks/PROVENANCE.md` | Repo/commit/SHA256/re-verify obligation | ✓ VERIFIED | Contains SHA256 table, explicit "read-only reference artifact" statement |
| `tests/vendored_schema_provenance.rs` | Edit-tripwire on vendored files | ✓ VERIFIED, WIRED | 358 lines; 5/5 tests pass |
| `.planning/phases/.../114-SPEC-RECHECK.md` | D-18 hold record | ✓ VERIFIED | Contains "Third Outcome Policy"; `## Verdict` = PENDING, re-measured 2026-08-01 |
| `tests/v1_tasks_golden.rs` | v1 byte-identity fixtures | ✓ VERIFIED, WIRED | 1012 lines; 14/14 tests pass |
| `tests/common/v2.rs` | Shared tasks harness (`OptionalBearer` etc.) | ✓ VERIFIED, WIRED | 848 lines; consumed by 8+ test files |
| `src/types/capabilities.rs` | `ClientCapabilities.extensions` + `TASKS_EXTENSION_KEY` + `TasksExtensionCapability` | ✓ VERIFIED, WIRED | Const + struct present; serde-lock tests pass |
| `src/server/task_store.rs` | `supports_inputs`, snapshot/record accessors, `InMemoryTaskStore` impl | ✓ VERIFIED, WIRED | 2500 lines; 57/57 unit tests pass |
| `src/server/tasks.rs` | Defaulted `TaskRouter::handle_tasks_update` | ✓ VERIFIED, WIRED | Present at line 105 |
| `crates/pmcp-tasks/src/store/generic.rs` | ONE input-delivery domain impl via `put_if_version` | ✓ VERIFIED, WIRED | 1583 lines; exercised by 29 `input_delivery.rs` tests across memory/DynamoDB/Redis |
| `crates/pmcp-tasks/src/store/memory.rs` | Delegating wrapper (D-13 site 2) | ✓ VERIFIED, WIRED | `every_generic_store_method_is_delegated_by_the_memory_wrapper` passes |
| `tests/v2_tasks_negotiation.rs` | Era x backend negotiation matrix | ✓ VERIFIED | 6/6 pass |
| `src/client/mod.rs` | Per-request extension decl, era-aware decoding, `tasks_update()` | ✓ VERIFIED, WIRED | 7675 lines; exercised by `v2_tasks_client.rs` (10/10) and `v2_tasks_client_era.rs` (21/21) |
| `src/types/mrtr.rs` | Separate tasks name-key table | ✓ VERIFIED, WIRED | `tasks_get_sets_mcp_name_to_the_task_id` etc. pass |
| `tests/v2_tasks_era_gates.rs` | Per-method era matrix | ✓ VERIFIED | 8/8 pass |
| `tests/v2_tasks_owner_binding.rs` | Identity-table + refusal-order matrix | ✓ VERIFIED | 8/8 pass |
| `tests/v2_reserved_fields_tasks.rs` | `inputRequests` egress-strip fix | ✓ VERIFIED | 6/6 pass |
| `src/types/tasks.rs` | Additive v2 projection types | ✓ VERIFIED, WIRED | 1874 lines |
| `tests/v2_tasks_shapes.rs` | Per-shape wire assertions | ✓ VERIFIED | 13/13 pass |
| `tests/v2_tasks_create.rs` | End-to-end v2 create via real `tools/call` | ✓ VERIFIED | 7/7 pass |
| `src/types/protocol/mod.rs` | `InternalClientRequest::TasksUpdate` | ✓ VERIFIED, WIRED | `client_request_has_no_tasks_update_variant` passes (semver-safe routing) |
| `tests/v2_tasks_update_routing.rs` | Routing/ordering/semver/MRTR-non-eligibility | ✓ VERIFIED | 18/18 pass |
| `fuzz/fuzz_targets/fuzz_tasks_update.rs` | Fuzz target over raw update boundary | ✓ VERIFIED | 175 lines; `cargo check --bin fuzz_tasks_update` compiles clean (sanitizer build needs nightly toolchain, unavailable in this environment — not re-executed) |
| `tests/v2_tasks_update.rs` | Delivery semantics/bounds/CAS/ack | ✓ VERIFIED | 17/17 pass |
| `tests/v2_tasks_security.rs` | Cross-caller live-socket matrix | ✓ VERIFIED | 8/8 pass |
| `tests/v2_tasks_tripwires.rs` | Source tripwires (era guards, `-32603` census) | ✓ VERIFIED | 25/25 pass |
| `examples/s50_v2_tasks_server.rs` / `s51_v2_tasks_agent.rs` | Paired runnable demo | ✓ VERIFIED, WIRED, DATA FLOWING | Built and RUN live end-to-end (see Behavioral Spot-Checks); exit code 0 |
| `tests/v2_tasks_client_era.rs` | Raw-frame decode assertions, v1 controls | ✓ VERIFIED | 21/21 pass |
| `.planning/.../114-CONTRACT-DECISION.md` | Contract-first owner decision record | ✓ VERIFIED | Contains "provable-contracts" |
| `.planning/.../deferred-items.md` | Owners/records for every deferred item | ✓ VERIFIED | 1115 lines; D-114-M/N/O/P/Q/S/U/V/W all recorded with owner or rationale |
| `src/server/task_dispatch.rs` | Central v2 tasks dispatch (era gates, owner binding, create gate, update delivery) | ✓ VERIFIED, WIRED | 4270 lines; 58/58 unit tests pass; exercised by every integration suite above |
| `src/server/core.rs` | Era-projected capabilities, reserved-field registry | ✓ VERIFIED, WIRED | 7848 lines |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `tests/vendored_schema_provenance.rs` | `schema/vendored/ext-tasks/PROVENANCE.md` | Recomputed SHA256 | WIRED | Test passes: `vendored_schema_every_file_digest_is_recorded_in_provenance_md` |
| `tests/v1_tasks_golden.rs` | `src/server/task_dispatch.rs` | Live v1 requests vs inline golden literals | WIRED | 14/14 pass, store- and router-backed |
| `src/server/builder.rs` → `src/server/task_dispatch.rs` | shared `apply_tasks_capability_rule` | WIRED | `v2_tasks_extension_advertised` + `v2_no_backend_advertises_no_tasks_extension` prove single-rule behavior |
| `src/shared/streamable_http.rs` → `src/types/mrtr.rs` | `v2_routing_headers` name-key lookup | WIRED | `tasks_get_sets_mcp_name_to_the_task_id`, `tasks_cancel_sets_mcp_name_to_the_task_id` pass |
| `crates/pmcp-tasks/src/router.rs` → `crates/pmcp-tasks/src/store/generic.rs` | `handle_tasks_update` override across the `Value` seam | WIRED | `the_router_delivers_inputs_across_the_value_seam` passes |
| `src/server/task_dispatch.rs` → `src/server/core.rs` | `resolve_owner` reusing `resolve_mrtr_principal`'s identity table | WIRED | `an_authenticated_caller_binds_its_own_subject`, ordered-refusal tests pass |
| `src/server/task_dispatch.rs` → `src/types/mrtr.rs` | `decode_for(kind, value)` from the persisted record | WIRED | `tasks_update_kind_directed_accepts_...` / `_refuses_a_sampling_shape_...` pass |
| `src/client/mod.rs` → `src/types/tasks.rs` | v2 projection types as client decode target | WIRED | `v2_flat_create_result_decodes_to_a_task_handle`, `v2_tasks_get_flat_payload_maps_ttl_ms_onto_ttl` pass |
| `examples/s51_v2_tasks_agent.rs` → `src/client/mod.rs` | production `tasks_update`/poll methods | WIRED | Live run drove the real client API, not a re-implemented loop (confirmed by reading source + observing output) |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Full v2 create→pause→update→terminal round trip over a real socket | `s50_v2_tasks_server` + `s51_v2_tasks_agent` run live | Agent printed all 5 demonstrations succeeding; task created `input_required` → answered → `completed`; manual `tasks_get_detailed`/`tasks_update` path also succeeded; undeclared client got ordinary result + `-32021`; `tasks/list`/`tasks/result` both `-32601` on v2 | ✓ PASS (exit code 0) |
| `cargo semver-checks` (no public-enum variant added by 114-13) | `cargo semver-checks` | 222/223 pass; the 1 failure (`OptimizedSseTransport` `#[deprecated]`) predates Phase 114 — introduced in Phase 113.1 commit `9b33a00f`, unrelated to tasks | ✓ PASS (for phase-114 scope) |
| `cargo clippy --all-targets --features full -D warnings` | direct clippy run | 0 warnings, 0 errors | ✓ PASS |
| `pmcp-tasks` backend suite (memory/DynamoDB-unit/Redis-unit, no live infra) | `cargo test --features dynamodb,redis` in `crates/pmcp-tasks` | 288+ passed, 0 failed | ✓ PASS |
| `pmcp-tasks` live-infra integration tests (`dynamodb-tests`/`redis-tests`) | `cargo test --all-features` | 36 failed — all connection-refused (no local DynamoDB/Redis running) | ? SKIP (environment has no local Redis/DynamoDB; these features are explicitly opt-in live-integration gates, not part of the default suite) |
| `fuzz_tasks_update` fuzz target compiles | `cargo check --bin fuzz_tasks_update` | Compiles clean | ✓ PASS (sanitizer build needs nightly toolchain, not installed in this environment — not re-executed; 20k-run claim not independently reproduced but code and harness are sound) |

### Probe Execution

SKIPPED — no `scripts/*/tests/probe-*.sh` declared for this phase and none found in the repository.

### Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|-------------|-----------------|-------------|--------|----------|
| TASK-01 | 114-01, 114-03, 114-05, 114-06, 114-12, 114-17, 114-18, 114-20 | v2 extension negotiation, v1 `experimental.tasks` preserved | ✓ SATISFIED | `v2_tasks_negotiation.rs`, `v2_tasks_client.rs`, live example |
| TASK-02 | 114-01, 114-04, 114-06, 114-07, 114-13, 114-14, 114-17, 114-18, 114-19, 114-20 | `tasks/update` input delivery | ✓ SATISFIED | `v2_tasks_update.rs`, `v2_tasks_update_routing.rs`, `input_delivery.rs`, live example |
| TASK-03 | 114-01, 114-08, 114-16, 114-18, 114-19, 114-20 | `tasks/list`/`tasks/result` era-gated off v2, v1 intact | ✓ SATISFIED | `v2_tasks_era_gates.rs`, `pending_tasks_result_preserves_minus_32002` unit test |
| TASK-04 | 114-01, 114-10, 114-11, 114-12, 114-16, 114-17, 114-18, 114-19 | v2 flat wire shapes, status-enum mapping | ✓ SATISFIED | `v2_tasks_shapes.rs`, `v2_tasks_tripwires.rs` status-equality tests |
| TASK-05 | 114-09, 114-15, 114-18, 114-20 | v2 owner binding fails closed, no cross-caller leak | ✓ SATISFIED | `v2_tasks_owner_binding.rs`, `v2_tasks_security.rs` (live socket, 2 principals) |
| TASK-06 | 114-02, 114-04, 114-07, 114-18, 114-20 | `TaskStore`/backends/state machine unchanged | ✓ SATISFIED | `v1_tasks_golden.rs`, `pmcp-tasks` backend suite, `task_store`/`task_dispatch` unit tests |

No orphaned requirements: REQUIREMENTS.md's Phase 114 traceability row lists exactly TASK-01..06,
and every ID appears in at least one plan's `requirements:` frontmatter field (114-20 and 114-18
each declare all six as phase-gate closers).

### Anti-Patterns Found

None. Scanned every `src/` and `crates/pmcp-tasks/src/` file this phase's plans identify as a
primary artifact (`task_dispatch.rs`, `task_store.rs`, `tasks.rs`, `core.rs`, `capabilities.rs`,
`types/tasks.rs`, `types/mrtr.rs`, `types/protocol/mod.rs`, `client/mod.rs`,
`crates/pmcp-tasks/src/store/{generic,memory}.rs`, `crates/pmcp-tasks/src/router.rs`) for
`TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/`PLACEHOLDER` markers. Zero hits.

### Human Verification Required

None. The phase's one deferred human-verify checkpoint (114-18 Task 4, `checkpoint:human-verify
gate="blocking"`, the phase sign-off) was already answered — approved by Guy Ernest (owner) on
2026-08-01, recorded in commit `cb0d2ecc`. No new human-verification items were identified during
this pass.

### Recorded / Known Items (not gaps — carried forward from the phase's own ledger)

These are cited here for completeness, per the phase's own instruction that they be reported as
RECORDED rather than re-litigated as new findings:

- **D-114-P** — three `TaskRouter` fall-through legs answer `-32603` where the extension makes
  `-32602` a MUST for `tasks/get`. Router-only; every store-backed path (the only backends shipped
  in this repository) is correct. Owner: Phase 118 (see Deferred Items above).
- **D-114-U** — `make test-feature-flags` grew 49→62 `^error` lines; the +13 are this phase's, all
  dead-code lints under `cargo clippy -p pmcp-tasks --no-default-features -D warnings`. Zero errors
  inside `crates/pmcp-tasks/` under any of its five real feature rows (all five `cargo check`
  independently). Owner: unassigned, tracked in `deferred-items.md`.
- **D-114-V** — `make doc-check` has 26 errors, byte-identical at phase base and HEAD; not part of
  `make quality-gate`.
- A pre-existing, unrelated `cargo semver-checks` failure (`OptimizedSseTransport` deprecation,
  Phase 113.1) requires a minor version bump before the next release — a release-process concern,
  not a Phase 114 defect.
- `ttl_respected_from_task_params` (pre-existing lifecycle test, not a Phase 114 artifact) failed
  once under parallel load and passed twice in isolation — a timing flake consistent with the
  already-documented D-114-A pattern, not a regression.

### Gaps Summary

None. All five ROADMAP success criteria (TASK-01 through TASK-06) are independently verified
against running code: negotiation, `tasks/update` delivery, era-gating, fail-closed owner binding
with a live-socket cross-caller proof, and unchanged storage backends all pass their respective
test suites, and the paired example demonstrates the full v2 lifecycle end-to-end with exit code 0.
The phase's `[~]` requirement marker reflects an external-publication hold (`ext-tasks` still in
`draft/`), not an implementation deficiency, and is correctly left untouched by this verification.

---

_Verified: 2026-08-01T02:00:00Z_
_Verifier: Claude (gsd-verifier)_
