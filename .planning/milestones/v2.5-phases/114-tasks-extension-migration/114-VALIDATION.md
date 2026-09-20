---
phase: 114
slug: tasks-extension-migration
status: complete
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-27
updated: 2026-07-28
---

# Phase 114 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo nextest` (Rust built-in harness + nextest runner) + `proptest` + `cargo fuzz` |
| **Config file** | none dedicated; driven by `Makefile` targets and the root `Cargo.toml` |
| **Quick run command** | `cargo nextest run --features full -E 'test(/v2_tasks/)'` (scoped to the touched surface) |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~30s scoped / ~10min full gate |

⚠ **`make lint`, never a bare `cargo clippy -- -D warnings`.** `make lint` applies pedantic + nursery +
cargo groups; five consecutive Phase-113 plans broke on lints a bare clippy invocation does not catch.

---

## Sampling Rate

- **After every task commit:** `cargo nextest run --features full -E 'test(/v2_tasks/)'` plus
  `cargo nextest run -p pmcp-tasks` when the task touched `crates/pmcp-tasks/`. Target < 30s.
- **After every plan wave:** `make quality-gate` + `cargo semver-checks check-release` +
  `make test-feature-flags` + `make wasm-build`.
- **Before `/gsd:verify-work`:** full suite green, all negative controls run and recorded RED-before /
  RED-under-control.
- **Max feedback latency:** 600 seconds.

---

## Per-Task Verification Map

Task IDs are `{plan}.T{n}`, matching the `<task>` order in each `114-NN-PLAN.md`. Threat refs are the
`T-114-NN` ids in each plan's `<threat_model>`.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01.T1 | 114-01 | 1 | TASK-01..04 | T-114-01/02/03 | Vendored schema is attributable and unmodified | integration | `cargo nextest run --features full -E 'test(/vendored_schema/)'` | ❌ created here | ⬜ pending |
| 01.T2 | 114-01 | 1 | TASK-01..04 | T-114-02 | Hold obligation is written, not implied | doc gate | `grep -q "Third Outcome Policy" .planning/phases/114-tasks-extension-migration/114-SPEC-RECHECK.md` | ❌ created here | ⬜ pending |
| 01.T3 | 114-01 | 1 | TASK-01..04 | T-114-01 | An unrecorded edit to a vendored file fails | integration | `cargo nextest run --features full -E 'test(/vendored_schema/)'` | ❌ created here | ⬜ pending |
| 02.T1 | 114-02 | 1 | TASK-06 | T-114-04 | v1 `tasks/*` bytes pinned PRE-reshape (D-14 item 2), raw-string compare after fixed-width substitution, on BOTH store- and router-backed paths | golden | `cargo nextest run --features full -E 'test(/v1_tasks_golden/)'` | ❌ created here (F19: none existed) | ⬜ pending |
| 02.T2 | 114-02 | 1 | TASK-05 | T-114-05/06 | Auth posture must be chosen explicitly; no accidental no-auth default | integration | `cargo nextest run --features full -E 'test(/v2_/)'` | ✅ extends `tests/common/v2.rs` | ⬜ pending |
| 03.T1 | 114-03 | 1 | TASK-01 | T-114-07/08 | Declaration is typed; capability value is `{}` | unit | `cargo nextest run --features full -E 'test(/capabilities/)'` | ✅ extends `src/types/capabilities.rs` | ⬜ pending |
| 03.T2 | 114-03 | 1 | TASK-01 | T-114-09 | Default serializes with NO extensions key (absence, not null) | unit | `cargo nextest run --features full -E 'test(client_extensions) or test(tasks_extension)'` | ✅ | ⬜ pending |
| 04.T1 | 114-04 | 1 | TASK-02, TASK-06 | T-114-11/15 | Additive defaulted seams (delivery + owner-scoped snapshot + input-request recorder + error persistence); pre-114 implementors compile | unit + doctest | `cargo nextest run --features full -E 'test(/task_store/)'` · `cargo test --features full --doc task_store` | ✅ extends `src/server/task_store.rs` | ⬜ pending |
| 04.T2 | 114-04 | 1 | TASK-02 | T-114-10/12/13/14 | In-crate delivery: complete-set transition, ignore semantics, NotFound-not-owner; snapshot/recorder/error accessors all owner-scoped | unit | `cargo nextest run --features full -E 'test(deliver_inputs) or test(snapshot) or test(record_input_requests) or test(_error)'` | ✅ | ⬜ pending |
| 04.T3 | 114-04 | 1 | TASK-02 | T-114-11 | Router seam fails explicitly, never silently | build + unit | `cargo build --features full` · `cargo check -p pmcp-tasks` | ✅ | ⬜ pending |
| 05.T1 | 114-05 | 2 | TASK-01 | T-114-17/18 | Endpoint-backed advertisement, additive, no capability lie | unit | `cargo nextest run --features full -E 'test(/tasks_capability/)'` | ✅ | ⬜ pending |
| 05.T2 | 114-05 | 2 | TASK-01 | T-114-16/19 | Era projection never mutates stored capabilities | unit | `cargo nextest run --features full -E 'test(/discover/)'` | ✅ | ⬜ pending |
| 05.T3 | 114-05 | 2 | TASK-01 | T-114-16/17/18 | v2 shows the entry, hides v1 keys; v1 initialize byte-identical | integration (live socket) | `cargo nextest run --features full -E 'test(/v2_tasks_negotiation/)'` | ❌ created here | ⬜ pending |
| 06.T1 | 114-06 | 2 | TASK-01 | T-114-22 | Per-request declaration emitted; absent when undeclared | unit | `cargo nextest run --features full -E 'test(/client_extension/)'` | ✅ extends `src/client/mod.rs` | ⬜ pending |
| 06.T2 | 114-06 | 2 | TASK-01 | T-114-23 | Un-negotiated tasks call fails fast with ZERO transport sends | unit | `cargo nextest run --features full -E 'test(/assert_capability/)'` | ✅ | ⬜ pending |
| 06.T3 | 114-06 | 2 | TASK-02 | T-114-20/21/24 | `Mcp-Name` = taskId; `tasks/update` NOT MRTR-eligible | unit | `cargo nextest run --features full -E 'test(/mrtr/) or test(name_bearing)'` | ✅ | ⬜ pending |
| 07.T1 | 114-07 | 2 | TASK-02, TASK-06 | T-114-25/26/27/30 | One CAS; pre-114 record deserializes absent-means-empty | integration | `cargo nextest run -p pmcp-tasks` | ✅ | ⬜ pending |
| 07.T2 | 114-07 | 2 | TASK-02, TASK-06 | T-114-31 | Owner passed in, never re-derived from params | build | `cargo check -p pmcp-tasks` · `cargo check -p pmcp-tasks --no-default-features` | ✅ | ⬜ pending |
| 07.T3 | 114-07 | 2 | TASK-02, TASK-06 | T-114-25/26/28/30 | CAS conflict, partial set, delegation completeness (F12) | integration | `cargo nextest run -p pmcp-tasks -E 'test(/input_delivery/)'` · `make test-feature-flags` | ❌ created here | ⬜ pending |
| 08.T1 | 114-08 | 3 | TASK-03 | T-114-33/34/35/36 | Two distinct truthful `-32601` messages; v1 frozen | unit | `cargo nextest run --features full -E 'test(/gate_tests/)'` | ✅ | ⬜ pending |
| 08.T2 | 114-08 | 3 | TASK-03 | T-114-32/34/35 | v2 list/result gated (no enumeration, no `-32002`); v1 both serve | integration (live socket) | `cargo nextest run --features full -E 'test(/v2_tasks_era_gates/)'` | ❌ created here | ⬜ pending |
| 09.T1 | 114-09 | 4 | TASK-05 | T-114-38 | Identity inputs threaded at BOTH sites; no second `_meta` read | build + unit | `cargo build --features full` · `cargo nextest run --features full -E 'test(/task_dispatch/)'` | ✅ | ⬜ pending |
| 09.T2 | 114-09 | 4 | TASK-05 | T-114-37/38/39/42 | Three-row table reused; no session-id / `client_id` row; v1 `"local"` frozen | unit | `cargo nextest run --features full -E 'test(/resolve_owner/)'` | ✅ | ⬜ pending |
| 09.T3 | 114-09 | 4 | TASK-05 | T-114-37/40/41/43 | Refusals ordered: `-32601` → `-32021` → `-32003` → params parse | integration (live socket) | `cargo nextest run --features full -E 'test(/v2_tasks_owner_binding/)'` | ❌ created here | ⬜ pending |
| 10.T1 | 114-10 | 5 | TASK-04 | T-114-46 | The silent strip is REPRODUCED at runtime before it is fixed | integration | `cargo nextest run --features full -E 'test(/v2_reserved_fields_tasks/)'` | ❌ created here | ⬜ pending |
| 10.T2 | 114-10 | 5 | TASK-04 | T-114-44/45/47/48 | Two legitimate minters; `requestState` stays MRTR-only | unit | `cargo nextest run --features full -E 'test(/reserved/)'` | ✅ | ⬜ pending |
| 10.T3 | 114-10 | 5 | TASK-04 | T-114-44/45/46/49 | Six properties, three orthogonal controls | integration | `cargo nextest run --features full -E 'test(/v2_reserved_fields_tasks/) or test(/v2_mrtr/)'` | ❌ created here | ⬜ pending |
| 11.T1 | 114-11 | 6 | TASK-04 | T-114-52 | Additive projection types; v1 structs untouched | unit | `cargo nextest run --features full -E 'test(/projection/)'` | ✅ extends `src/types/tasks.rs` | ⬜ pending |
| 11.T2 | 114-11 | 6 | TASK-04 | T-114-50/51/52/56 | Flat shapes + empty acks; `NotFound` → `-32602` with no oracle | integration | `cargo nextest run --features full -E 'test(/v2_tasks_shapes/) or test(/v1_tasks_golden/)'` | ❌ created here | ⬜ pending |
| 11.T3 | 114-11 | 6 | TASK-04 | T-114-53/54/55 | Status set-equality vs vendored schema; both terminal-status directions; `resultType:"task"` confined to the tool-call create path and absent from `tasks/get`/`cancel`/`update` | integration + unit tripwire | `cargo nextest run --features full -E 'test(/v2_tasks_shapes/)'` | ❌ created here | ⬜ pending |
| 12.T1 | 114-12 | 7 | TASK-01, TASK-04 | T-114-57/59 | Era-aware trigger; each era ignores the other's signal | unit | `cargo nextest run --features full -E 'test(/gate_tests/)'` | ✅ | ⬜ pending |
| 12.T2 | 114-12 | 7 | TASK-04 | T-114-58 | Trigger condition exists exactly ONCE; both sites reach it; handler-declared inputRequests recorded against the STORE-minted id; `ResponseDisposition::Task` dead-code allow removed HERE (first production constructor) | build + unit | `cargo build --features full` · `make lint` · `cargo nextest run --features full -E 'test(/tool_call/)'` | ✅ | ⬜ pending |
| 12.T3 | 114-12 | 7 | TASK-01, TASK-04 | T-114-60/61 | Real v2 `tools/call` yields a pollable handle; a handler-declared input request is visible via `tasks/get` (create→pause loop); non-declaring client sees no `taskId` | integration (live socket) | `cargo nextest run --features full -E 'test(/v2_tasks_create/)'` | ❌ created here | ⬜ pending |
| 13.T1 | 114-13 | 8 | TASK-02 | T-114-63/64/65 | Raw params in the classifier; zero public-API change | build + semver | `cargo semver-checks check-release` · `cargo public-api diff` | ✅ | ⬜ pending |
| 13.T2 | 114-13 | 8 | TASK-02 | T-114-66 | Routed at both sites, all gates in order | build + integration | `cargo nextest run --features full -E 'test(/v2_tasks_update_routing/)'` | ❌ created here | ⬜ pending |
| 13.T3 | 114-13 | 8 | TASK-02 | T-114-62/63/65 | Three replacement guards for the lost MRTR compile tripwire | integration + source tripwire | `cargo nextest run --features full -E 'test(/v2_tasks_update_routing/)'` | ❌ created here | ⬜ pending |
| 14.T1 | 114-14 | 9 | TASK-02 | T-114-67/68/69/74/76 | Raw `BTreeMap` boundary (untagged decoder never runs at ingress), FOUR input-response bounds FIRST, then kind-directed decode from `task_input_snapshot` | integration | `cargo nextest run --features full -E 'test(/v2_tasks_update/)'` | ❌ created here | ⬜ pending |
| 14.T2 | 114-14 | 9 | TASK-02 | T-114-67/68/70/71/72/73 | Partial-vs-complete, ignore semantics, CAS, empty ack + property test | integration + property | `cargo nextest run --features full -E 'test(/v2_tasks_update/) or test(/task_dispatch/)'` | ❌ created here | ⬜ pending |
| 14.T3 | 114-14 | 9 | TASK-02 | T-114-68/74/75 | Raw-params boundary fuzzed; campaign proven falsifiable | fuzz | `cargo fuzz run fuzz_tasks_update -- -runs=20000` | ❌ created here | ⬜ pending |
| 15.T1 | 114-15 | 10 | TASK-05 | T-114-77/78/79 | B gets `NotFound` on A's task for get/update/cancel; message indistinguishable | integration (live socket) | `cargo nextest run --features full -E 'test(/v2_tasks_security/)'` | ❌ created here | ⬜ pending |
| 15.T2 | 114-15 | 10 | TASK-05 | T-114-80/81/82 | v1/v2 buckets disjoint; shared-bucket caveat asserted not implied; per-method controls each fail exactly one test; any production defect found is recorded as BLOCKING for 114-18 | integration (live socket) | `cargo nextest run --features full -E 'test(/v2_tasks_security/)'` | ❌ created here | ⬜ pending |
| 16.T1 | 114-16 | 10 | TASK-03 | T-114-83/87 | Every tasks route names an era guard; three rot conditions fail | source tripwire | `cargo nextest run --features full -E 'test(/v2_tasks_tripwires/)'` | ❌ created here | ⬜ pending |
| 16.T2 | 114-16 | 10 | TASK-04 | T-114-84/85/86 | No v2 `-32603` not-found; status set-equality; per-value provenance | source tripwire | `cargo nextest run --features full -E 'test(/v2_tasks_tripwires/)'` | ❌ created here | ⬜ pending |
| 19.T1 | 114-19 | 10 | TASK-04 | T-114-99/100/105 | Era-aware decoding of flat v2 create/get and empty cancel ack; `resultType` drives the branch, never shape-sniffing | unit | `cargo nextest run --features full -E 'test(/v2_tasks_client_era/) or test(/call_tool_with_task/)'` | ✅ extends `src/client/mod.rs` | ⬜ pending |
| 19.T2 | 114-19 | 10 | TASK-02, TASK-03 | T-114-101/102/103/104 | `tasks_update` asserts capability first; v2 terminal read is inline from `tasks/get`, never the retired `tasks/result`; input rounds bounded | unit | `cargo nextest run --features full -E 'test(/tasks_update/) or test(/wait_for_task/)'` | ✅ | ⬜ pending |
| 19.T3 | 114-19 | 10 | TASK-02, TASK-03, TASK-04 | T-114-99/100/103 | Raw-frame decoding of every v2 shape; retired methods fail fast with ZERO sends; v1 controls green | integration (raw frames) | `cargo nextest run --features full -E 'test(/v2_tasks_client_era/)'` | ❌ created here | ⬜ pending |
| 17.T1 | 114-17 | 11 | TASK-01, TASK-04 | T-114-88 | Runnable server half; caveats stated in source; tool declares inputRequests so the create→pause loop fires | example build | `cargo build --features full --example s50_v2_tasks_server` | ❌ created here | ⬜ pending |
| 17.T2 | 114-17 | 11 | TASK-02, TASK-04 | T-114-89/90/91/92 | Autonomous poll loop over production methods; exits non-zero on divergence; touches no file outside `examples/` | example (end-to-end) | `cargo build --features full --example s51_v2_tasks_agent` + recorded paired run | ❌ created here | ⬜ pending |
| 20.T1 | 114-20 | 1 | TASK-01..06 | T-114-106/107/108 | The contract-first dependency is measured, not inferred; both options stated at equal weight | doc gate | `test -s .../114-CONTRACT-DECISION.md && grep -c "Option A\|Option B\|## Decision" .../114-CONTRACT-DECISION.md` | ❌ created here | ⬜ pending |
| 20.T2 | 114-20 | 1 | TASK-01..06 | T-114-106/107 | A repository-level mandatory rule is settled by the OWNER before implementation, with a gated follow-up | **manual** (`checkpoint:decision`, gate `blocking`) | `grep -qE '^Chosen: option-(a\|b)' .../114-CONTRACT-DECISION.md` | ❌ created here | ⬜ pending |
| 18.T1 | 114-18 | 12 | TASK-01..06 | T-114-95 | No doc describes pre-114 behaviour as current | doc gate | `make doc-check` | ✅ | ⬜ pending |
| 18.T2 | 114-18 | 12 | TASK-01..06 | T-114-96/97/98 | Whole-tree gate; zero dependency change; no 4th pmat violation | build gate | `make quality-gate` · `cargo semver-checks check-release` · `make test-feature-flags` · `make wasm-build` · `make comply` | ✅ | ⬜ pending |
| 18.T3 | 114-18 | 12 | TASK-01..06 | T-114-93/94 | Six requirements booked `[~]`; every wire value walkable to source | doc gate | `grep -c "pending final schema" .planning/REQUIREMENTS.md` | ✅ | ⬜ pending |
| 18.T4 | 114-18 | 12 | TASK-01..06 | — | Delivered outcomes match the approved decisions | **manual** (see below) | `grep -c "quality-gate" .../114-18-SUMMARY.md` + human check | n/a | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

**Sampling continuity:** no three consecutive tasks lack an `<automated>` verify — every task in all 20
plans carries one, including 18.T4 and 20.T2, whose automated halves check that the decision/SUMMARY was
actually written while the human gate covers the judgement.

---

## Wave 0 Requirements

Wave 0 work is folded into **wave 1** (plans 114-01 through 114-04), which has no dependencies. Both
original Wave 0 items are addressed:

- [x] **`ClientRequest` semver additivity (RESEARCH Q5) — ANSWERED BEFORE PLANNING, not deferred to
      execution.** `114-PATTERNS.md` Fact 1 measured `src/types/protocol/mod.rs:479-483`: `ClientRequest`
      is **NOT** `#[non_exhaustive]`, so a variant would be semver-MAJOR. The design changed accordingly
      (DQ5 → `114-13-PLAN.md` routes via `InternalClientRequest`). `cargo semver-checks check-release`
      no-update-required is an acceptance criterion of 114-13 Task 1, so the answer is re-proven at
      execution rather than trusted.
- [ ] **Vendored ext-tasks schema pinned at a commit** → `114-01-PLAN.md` Task 1, with a SHA256 provenance
      tripwire (Task 3) so downstream plans review against a fixed offline artifact.

**Added by the cross-AI reviews replan (2026-07-28), all VERIFIED against the source before acting:**

- [ ] **The v2 CLIENT half** → `114-19-PLAN.md` (wave 10). Measured at `src/client/mod.rs`: the client
      still decodes NESTED v1 `CreateTaskResult` (`:1069`, `:1140`), `GetTaskResult` (`:1233`) and
      `CancelTaskResult` (`:1435`), and `wait_for_task` ends by calling the v2-retired `tasks/result`
      (`:1385`). Without this plan CONTEXT.md's locked dual-surface steer (D-04/D-05) would be false at
      phase end and 114-17's paired example could not run. All `src/client/mod.rs` work moved here from
      114-17, which is now examples-only.
- [ ] **The owner-scoped input snapshot** → `114-04-PLAN.md` Task 1 items 5-7. Measured:
      `TaskStore::get` returns only `Task` (`src/server/task_store.rs:249`), `TaskRecord` is private
      (`:404`) and carries no `error` field — so 114-11's `inputRequests`/`error` inlining and 114-14's
      kind-directed decode had no reachable data source. 114-11 now declares `depends_on: [114-04, 114-10]`.
- [ ] **The create → pause loop** → `114-12-PLAN.md` Task 2. Measured:
      `build_task_created_response` mints the canonical id via `store.create` AFTER the handler returns
      (`src/server/task_dispatch.rs:484-494`), discarding the tool's fabricated id — so a handler could
      not associate input requests with the id the client polls.
- [ ] **The `dead_code` ordering hazard** → moved from `114-10` (wave 5) to `114-12` (wave 7). Measured:
      `make lint` runs `cargo clippy --features "full" --lib --tests` under `RUSTFLAGS = -D warnings`
      (Makefile:11, :150-152) and the `--lib` half is a NON-test build, so removing the allow on
      `ResponseDisposition::Task` (`src/server/core.rs:1150`) before a production constructor exists
      would break the gate for two waves.
- [ ] **The contract-first owner decision** → `114-20-PLAN.md` (wave 1, blocking checkpoint), replacing
      114-18's self-granted exemption.

Additional wave-1 prerequisites this phase discovered, all owned:

- [ ] **v1 golden byte fixtures** → `114-02-PLAN.md` Task 1. F19 measured that NONE exist; they must be
      captured PRE-reshape or they pin the wrong bytes.
- [ ] **Two-principal / optional-bearer live-socket harness** → `114-02-PLAN.md` Task 2. `BearerSubjects`
      returns `Err` for a missing token, so the transport answers 401 before dispatch and TASK-05's
      fail-closed branch is otherwise **unreachable from a test**.
- [ ] **`ClientCapabilities.extensions` field** → `114-03-PLAN.md` Task 1. F6: the field does not exist, so
      a client declaration is silently dropped by serde today.
- [ ] **Backend input-delivery seams** → `114-04-PLAN.md`, then `114-07-PLAN.md` for the three production
      backends.

**Framework install: none needed.** `cargo nextest`, `cargo-semver-checks` (0.49.0), `cargo-public-api`
(0.52.0), `pmat` (3.15.0) and `cargo fuzz` are all present. `make test-feature-flags` already implements
D-14 item 4 (F14) and is reused rather than reinvented.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Delivered DQ1/DQ4 outcomes match what the owner approved, and DQ2/DQ3/DQ5/DQ6/DQ7 are confirmed | TASK-01..06 | A scope-fidelity judgement, not a behavioural property: no test can assert that shipped behaviour is what the owner meant. Scope itself is NOT re-litigated here — DQ1 and DQ4 were explicitly approved pre-execution on 2026-07-27 | `114-18-PLAN.md` Task 4 (`checkpoint:human-verify`, gate `blocking`): run `cargo run --features full --example s50_v2_tasks_server` and `s51_v2_tasks_agent`; confirm `s51` exits 0 showing create → input_required → update → terminal; check the recorded gate numbers in `114-18-SUMMARY.md`; confirm TASK-01..06 are `[~]` and the recheck Verdict is `PENDING` |

Everything else in this phase has automated verification. Two categories are worth naming because they are
easy to mistake for manual work:

- **Live DynamoDB / Redis** — not manual and not skipped: `GenericTaskStore<InMemoryBackend>` shares 100%
  of the domain logic (that is the type's stated purpose) and `make test-feature-flags` compiles all four
  feature rows. Per the project's no-Docker-in-tests rule, testcontainers are not added.
- **Negative controls** — executed and recorded per plan by the executor, then verified from the SUMMARY.
  They are procedure, not manual verification.

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies — all 54 tasks across 20 plans carry one
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (folded into wave 1; Q5 answered pre-planning by measurement)
- [x] No watch-mode flags
- [x] Feedback latency < 600s (scoped runs ~30s; full gate ~10min, run per wave not per task)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-07-27; per-task map re-synchronised 2026-07-28 after the `--reviews` replan (18 → 20 plans, 11 → 12 waves)
