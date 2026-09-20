---
phase: 109
reviewers: [codex, gemini]
reviewed_at: 2026-07-18T20:54:17Z
plans_reviewed: [109-01-PLAN.md, 109-02-PLAN.md, 109-03-PLAN.md, 109-04-PLAN.md, 109-05-PLAN.md, 109-06-PLAN.md, 109-07-PLAN.md, 109-08-PLAN.md]
---

# Cross-AI Plan Review — Phase 109

## Codex Review

# Cross-AI Plan Review — Phase 109

## Overall assessment

The phase is thoughtfully decomposed around the four server surfaces, shared transport, composition policy, runtime wiring, and conformance. The wave ordering and file ownership are mostly strong.

However, the plans are not currently executable as written. Repository inspection found several load-bearing assumptions that conflict with the actual APIs:

- `CallToolRequest::_meta` cannot carry the proposed arbitrary depth/caller/ancestor fields, and `RequestHandlerExtra` does not receive them.
- `Client::call_tool_with_task` cannot send `_meta`, and a required task call normally returns `ToolCallResponse::Task`, not a synchronous result with `related_task`.
- The actual related-task key is `io.modelcontextprotocol/related-task`, not `related_task`.
- `AgentPackage.llm` is mandatory, so the repeated "no LLM slot → FixedSource fallback" branch cannot occur.
- A `TeamPackage` contains `ComponentRef`s, not resolved `AgentPackage`s; no package resolver is supplied to the binaries or `TeamRuntime`.
- The existing fixtures are illustrative and contain hard-coded generated IDs/state. They cannot be replayed directly against live servers.
- `pmat comply check` accepts a project path, not a binding-file positional argument, and is not itself a direct `binding.yaml` validator.

These are architectural blockers concentrated in Plans 05–08, but they also affect the contract and scaffold in Plan 01. Overall phase risk is **HIGH** until a short prerequisite/API-enablement plan resolves them.

---

# 109-01 — Crate scaffold, transport, derivation, contract revision

## Summary

The plan establishes sensible ownership boundaries and correctly puts the pure attachment policy early. Centralizing the fuzz manifest and productionizing the duplex transport are good dependency-management decisions. The contract revision, however, freezes incorrect assumptions before their supporting SDK surfaces exist, while placeholder implementations and "filled later" comments conflict with the repository's zero-SATD, zero-defect policy.

## Strengths

- Clear Wave 1 ownership prevents Wave 2 plans from co-editing the fuzz manifest.
- `derive_attachment` is isolated as a pure, exported policy function.
- Property coverage captures the important N/M composition matrix.
- Promoting a reusable in-memory transport supports runtime and conformance testing.
- The additive contract revision preserves existing tool names and prefixes.
- Workspace and publish-order changes are explicitly included.

## Concerns

- **HIGH:** The contract documents arbitrary guard state in request `_meta`, but `RequestMeta` currently contains only `progress_token` and `_task_id`. Unknown fields deserialize away, and `RequestHandlerExtra` does not expose arbitrary request metadata.
- **HIGH:** The contract uses `_meta[related_task]`, while the SDK constant is `io.modelcontextprotocol/related-task`. Freezing the wrong key into v1.1.0 will create immediate conformance drift.
- **HIGH:** Placeholder behavior in `derive_attachment` is temporarily exported and buildable. A plausible but incorrect implementation is more dangerous than an unavailable module.
- **MEDIUM:** Comments such as `// filled by 109-02` are likely SATD under the repository's zero-SATD policy.
- **MEDIUM:** Four binaries require both their server feature and `http`, so the stdio fallback cannot be built without the HTTP stack.
- **MEDIUM:** `clap` and `tracing-subscriber` are described as binary-only but are unconditional Cargo dependencies. Cargo has no automatic binary-only dependency class.
- **MEDIUM:** Default features enable all four server modules plus conformance, undermining the stated per-server isolation and increasing the default dependency surface.
- **LOW:** `AttachmentSet.opt_ins` preserves arbitrary built-in server references rather than validating/deduplicating the supported `team-fs` and `mem-mcp` extras.

## Suggestions

- Add a prerequisite SDK task to extend `RequestMeta` with a namespaced extensible map and propagate it into `RequestHandlerExtra`.
- Use `RELATED_TASK_META_KEY` everywhere and update the contract/fixtures to `io.modelcontextprotocol/related-task`.
- Implement `derive_attachment` atomically when it is first exported; avoid placeholder behavior.
- Replace "filled later" comments with empty documented modules or omit modules until their implementing plans.
- Decide deliberately whether default features are empty, minimal, or "all reference servers"; test every supported feature combination.
- Add explicit behavior for unknown, duplicate, and wrongly typed `built_in_servers`.
- Include `make quality-gate` and PMAT proxy validation before any task-level commit, per repository instructions.

## Risk assessment

**HIGH.** The scaffold itself is conventional, but it would enshrine two incorrect wire contracts and knowingly introduce placeholder/SATD artifacts.

---

# 109-02 — team-fs

## Summary

The backend-trait design and real review-directory workflow fit the phase well. Security receives appropriate attention, including fuzzing. The proposed path-resolution algorithm is insufficient for safe creation of new nested paths and remains vulnerable to symlink/TOCTOU issues, so the plan overstates the strength of its containment proof.

## Strengths

- Strong separation between server surface and storage backend.
- Exact eleven-tool surface is explicit and testable.
- Real `workspace/` ↔ `review/` synchronization makes the dev backend useful.
- Path traversal is treated as a blocking security boundary.
- Fuzzing targets the most security-sensitive pure helper.
- Tool-list exactness and `fs__list` annotations are included.
- The server builder remains CLI-free.

## Concerns

- **HIGH:** "Canonicalize the parent" fails for writes where a nested parent does not yet exist. This conflicts with normal write/create-directory behavior.
- **HIGH:** `canonicalize` followed by later I/O is subject to symlink replacement/TOCTOU. The fuzz assertion `starts_with(root)` does not prove the later operation remains contained.
- **MEDIUM:** Rejecting only components equal to `..` and absolute paths needs platform-specific treatment for prefixes, symlinks, empty paths, NULs, and Windows-style paths.
- **MEDIUM:** `format!("file://{}", path)` does not produce a valid URI for spaces, `#`, `%`, non-ASCII text, or platform-specific paths.
- **MEDIUM:** `sync_*` behavior is unspecified for directories, symlinks, missing destinations, overwrites, and partial-copy failures.
- **MEDIUM:** `complete_task` is placed on the filesystem backend even though task completion is protocol/server behavior, not storage behavior.
- **MEDIUM:** The binary is said to load per-server settings from `TeamPackage`, but `TeamPackage` has no filesystem-server settings. Only the override source is described.
- **LOW:** The stated test command supplies two Cargo test filters and is invalid; Cargo accepts one positional test filter.

## Suggestions

- Define a single safe path abstraction with lexical normalization, nearest-existing-ancestor validation, and no-follow/open-at semantics where practical.
- Explicitly reject symlinks for the local reference backend if race-resistant traversal is out of scope.
- Use a URI/file-URL library or a tested conversion routine with percent encoding.
- Specify overwrite, recursive-copy, atomicity, and partial-failure semantics for review synchronization.
- Keep `complete_task` in the server layer unless the contract genuinely requires backends to implement it.
- Add tests for symlink escape, nonexistent nested parents, Unicode filenames, directory sync, destination collision, and partial failure.
- Correct verification commands and run the full crate gate after implementation.

## Risk assessment

**HIGH.** The functional scope is reasonable, but the proposed containment implementation does not support all intended operations safely.

---

# 109-03 — mem-mcp

## Summary

This is one of the better bounded plans. A small deterministic in-memory scorer is appropriate for a dev reference implementation and avoids the prohibited embedder dependency. The main weaknesses are imprecise BM25 properties, omitted test-file ownership, concurrency details, and fixture determinism.

## Strengths

- Correctly avoids embedder/vector dependencies.
- Clear object-safe backend seam.
- Exact six-tool surface is explicit.
- Deterministic in-memory behavior suits CI.
- Unit and property tests cover ranking and CRUD behavior.
- The scorer's constants and formula are intended to be documented.
- The binary reuses the same HTTP/stdio shape as other servers.

## Concerns

- **MEDIUM:** The proposed monotonicity property is not generally valid under BM25 length normalization. Adding a term can change document length and comparative scores.
- **MEDIUM:** "Adding an unrelated document does not change relative order beyond documented IDF effects" is too vague to be falsifiable.
- **MEDIUM:** `parking_lot` is suggested but is not included in the manifest dependencies.
- **MEDIUM:** Rebuilding the complete index for every search is acceptable for dev scale, but no limits or complexity expectations are stated.
- **MEDIUM:** Generated UUIDs conflict with existing fixtures expecting deterministic IDs such as `mem-001`.
- **LOW:** `tests/mem_props.rs` may be created but is absent from `files_modified`.
- **LOW:** The stated Cargo test command includes multiple positional filters and will fail.
- **LOW:** Stable tie-breaking is unspecified; equal scores could yield nondeterministic order.

## Suggestions

- Test safer invariants: non-negativity, zero for no overlap, determinism, finite scores, stable tie-breaking, and increased term frequency under fixed document length.
- Add a deterministic ID/clock seam for conformance and examples.
- Specify stable ordering as score descending, then creation ordinal or ID.
- Add configurable dev limits for item count, text length, query length, and result limit.
- Add all created test files to `files_modified`.
- Choose and declare the synchronization primitive explicitly.

## Risk assessment

**MEDIUM.** The core design is sound; the remaining issues are mostly testability and precision rather than architecture.

---

# 109-04 — approval-mcp

## Summary

The notify-only channel model is clean and correctly keeps resolution in MCP tools. Reusing `InMemoryTaskStore` is directionally right, but the plan does not explain how approval-specific state lives in a store that only contains task status/result records. Owner scoping, atomic resolution, and notification failures need explicit design.

## Strengths

- Correctly preserves the two unnamespaced legacy tools.
- Dynamic ask tools are derived once from human roles.
- Console and webhook are clearly notification-only.
- No stdin/TTY behavior keeps CI deterministic.
- Webhook secret logging is treated as a blocking concern.
- Optional subject linkage is carried through ask/get/resolve.
- Exact tool-surface tests are included.

## Concerns

- **HIGH:** `InMemoryTaskStore` has no native approval-record fields for question, options, target role, verdict, or subject reference. The plan simultaneously forbids a bespoke store but requires this state without defining where it lives.
- **HIGH:** `TaskStore` is owner-scoped, while D-10 says resolution may occur from any connected client. The owner model is unresolved.
- **HIGH:** Tasks are created in `Working`, not "pending"; the intended approval lifecycle and legal transitions are unspecified.
- **HIGH:** Ask→notify failure semantics are missing. The task could be created while notification fails, leaving an unreachable approval.
- **MEDIUM:** Concurrent/double resolution needs an atomic first-writer policy and idempotency definition.
- **MEDIUM:** The decision must be validated against the original option set.
- **MEDIUM:** Unknown dynamic tool names naturally produce generic "tool not found," not necessarily the contract's `"unknown member"` error.
- **MEDIUM:** Webhook tests only compile the feature; no mock HTTP server verifies payload, header behavior, timeout, or non-resolution.
- **LOW:** The non-webhook binary needs cfg-safe behavior when `--webhook-url` is supplied without the feature.

## Suggestions

- Define an `ApprovalRepository` or store approval data in task terminal results/variables through an explicit adapter. Do not pretend `TaskStore` alone stores approval domain state.
- Lock the owner policy: service owner, creator owner, shared capability, or authenticated human role.
- Specify lifecycle states, terminal result shape, TTL, cancellation, idempotency, and double-resolution behavior.
- Decide whether notification occurs before creation, after creation with rollback, or after creation with a retryable notification state.
- Add webhook integration tests with a local mock endpoint and bounded timeout.
- Use a deterministic ID generator for conformance fixtures.

## Risk assessment

**HIGH.** The plan has the right external behavior but lacks the internal state and authorization model required to implement it correctly.

---

# 109-05 — team-mcp

## Summary

This plan targets the most important migration path, but its central execution story is incompatible with the current SDK. Request guard metadata is not available to handlers, task-augmented calls cannot carry it, the client may return a task rather than a result, the related-task key is wrong, and the package model has neither optional LLM slots nor distinct member ID/display-name fields.

## Strengths

- Correctly identifies the raw-JSON-RPC bypass as the migration target.
- Uses a real MCP client/server hop rather than directly invoking handlers.
- Calls out `ToolOutput::Result` middleware bypass and redaction ownership.
- Treats depth, self-call, ancestor cycle, and unknown member as explicit guards.
- Includes property and fuzz testing for parser/guard behavior.
- Separates reusable member wiring from the server builder and binary.
- Tries to centralize member completion-source construction.

## Concerns

- **HIGH:** `RequestMeta` cannot represent arbitrary depth, caller, or ancestor fields. `RequestHandlerExtra` currently receives task/auth/progress information, not the proposed namespaced metadata.
- **HIGH:** `Client::call_tool_with_task(name, arguments)` always sets `_meta: None`; it cannot forward guard state.
- **HIGH:** For task-required member tools, `call_tool_with_task` returns `ToolCallResponse::Task`. The plan only meaningfully handles `Result` and expects a synchronous `CallToolResult`.
- **HIGH:** Existing SDK tests explicitly note that `call_tool_with_task` discards the create-envelope `_meta`. The acceptance test requiring client-observed `related_task` is therefore impossible.
- **HIGH:** The wire key is `io.modelcontextprotocol/related-task`, not `related_task`.
- **HIGH:** `AgentPackage.llm` is mandatory. "Fallback only when there is no llm ConfigSlot" is an unreachable branch.
- **HIGH:** `TeamPackage` contains `ComponentRef`s, but the binary has no package repository/resolver with which to load each `AgentPackage`.
- **HIGH:** `TeamMember` has no separate stable ID and display name. Its identity is effectively the referenced component name, so the proposed same-name/different-ID test cannot be constructed.
- **MEDIUM:** `resolve_slot` returns a model/provider string; it does not itself construct an OpenAI/Anthropic source or resolve API credentials/endpoints. `resolve_member_factory` is significantly underspecified.
- **MEDIUM:** `StreamableHttpServer` has no described per-binary hook that rewrites an HTTP header into `CallToolRequest::_meta`.
- **MEDIUM:** Sanitizing all member `_meta` except related-task contradicts "re-emits the result verbatim."
- **MEDIUM:** Missing-depth behavior is unspecified.
- **LOW:** Several Cargo test commands use invalid multiple filters.

## Suggestions

Create a prerequisite "109-00 SDK enablement" plan that:

1. Extends `RequestMeta` with an extensible namespaced map.
2. Propagates request metadata into `RequestHandlerExtra`.
3. Adds `Client::call_tool_with_task_and_meta` or a general request API.
4. Adds a transport-agnostic metadata injection hook for HTTP.
5. Defines whether team-mcp returns a `CreateTaskResult`, polls the member task, or synthesizes a synchronous result linked to the member task.
6. Uses the SDK's `RELATED_TASK_META_KEY`.
7. Defines a real `MemberId` source available in `TeamPackage`.
8. Supplies a package resolver and a provider/source factory registry.

Also replace the nonexistent "no LLM slot" branch with an explicit injected factory override used by tests and examples.

## Risk assessment

**HIGH.** The plan's primary success criterion cannot be reached through the current client, request metadata, task, or package APIs.

---

# 109-06 — TeamRuntime

## Summary

The idea of making Phase 110 a thin CLI over a library runtime is excellent. The plan, however, assumes it can resolve member packages and construct complete `AgentServer`s from a `TeamPackage` plus one completion factory. It lacks package-resolution, agent-runtime seams, filesystem configuration, feature-gating behavior, and a workable LLM fallback model.

## Strengths

- Correctly centralizes attachment decisions through `derive_attachment`.
- In-memory transports make tests deterministic and sandbox-safe.
- Runtime ownership and shutdown are considered.
- Team-of-one degeneration is explicitly tested.
- Runtime accessors make examples and future CLI integration practical.
- FixedSource is the correct CI strategy.

## Concerns

- **HIGH:** `TeamRuntime::start` cannot turn `ComponentRef`s into `AgentPackage`s without a package repository/resolver argument.
- **HIGH:** Constructing `AgentServer` also requires configuration, tool invoker, and conversation store seams; the proposed signature does not supply them.
- **HIGH:** The "no LLM slot" fallback is impossible because `AgentPackage.llm` is mandatory.
- **HIGH:** `compose::wiring` is always compiled while it references feature-gated server modules. Selective feature builds will fail unless the implementation is heavily cfg-gated or given an aggregate feature.
- **MEDIUM:** No data-directory/configuration argument exists for `LocalDirBackend`.
- **MEDIUM:** Unsupported or unknown opt-ins have no fail/ignore policy.
- **MEDIUM:** Startup failure after some tasks are spawned needs rollback and cleanup.
- **MEDIUM:** Runtime ownership of background tasks and clients needs explicit `Drop` versus async `shutdown` semantics.
- **LOW:** The large all-features integration test does not prove all supported reduced feature combinations.

## Suggestions

- Introduce an explicit `TeamRuntimeBuilder` with: `PackageResolver`, `SlotResolver`, completion-source registry or override, agent runtime seams, data root, approval channel, enabled-server policy.
- Add an aggregate `runtime` feature depending on the four server features, or cfg-gate each attachment branch.
- Fail closed on requested-but-uncompiled or unknown built-ins.
- Make startup transactional: abort already spawned tasks on any later failure.
- Test startup failure cleanup and selective feature configurations.

## Risk assessment

**HIGH.** The API omits multiple inputs essential to building the runtime and depends on unresolved Plan 05 assumptions.

---

# 109-07 — Conformance harness

## Summary

An exportable wire-level conformance runner is a high-value deliverable, and exact surface matching is the right standard. The current fixture schema and runner API cannot support the proposed live execution: fixtures contain generated IDs and assumed state, do not contain expected input schemas, and the client cannot send arbitrary `_meta`. The API also does not actually support HTTP endpoints.

## Strengths

- Tests behavior through a real protocol client rather than direct handlers.
- Exact tool-name equality is stronger than subset checks.
- Every-tool/every-guard coverage is clearly required.
- The runner is intended for reuse by platform implementations.
- Structured reports are preferable to fail-fast assertions.
- Existing Phase 107 structural checks are preserved.

## Concerns

- **HIGH:** Existing fixtures contain hard-coded generated IDs (`appr-42`, `mem-001`, `task-abc123`) and assumed prior state. They are not independently replayable.
- **HIGH:** Stateful sequences such as add→get, write→read, and ask→resolve need setup, capture/substitution, ordering, and reset semantics, none of which exist in fixture schema v1.
- **HIGH:** Fixtures contain no expected `tools/list` schemas, so the runner cannot assert per-tool input-schema equality from the current data.
- **HIGH:** `Client::call_tool` and `call_tool_with_task` cannot send the proposed guard `_meta`; arbitrary fixture `_meta` will be lost.
- **HIGH:** `run_fixtures(server: Server, ...)` cannot point at an HTTP endpoint, contrary to D-17/D-19 claims.
- **HIGH:** The actual related-task metadata key differs from the fixture key.
- **MEDIUM:** "Subset match" needs precise recursive semantics for arrays, generated fields, numbers, and ignored notes.
- **MEDIUM:** Error handling through `Client` may expose a typed error rather than the original raw JSON response; numeric code preservation must be verified.
- **MEDIUM:** File iteration order is not deterministic, making implicit stateful sequencing unsafe.
- **MEDIUM:** The runner signature consumes a concrete `Server`, not "any server implementation."

## Suggestions

- Version the fixture format to include: `kind: tools_list | tool_call`, setup/teardown or scenario grouping, deterministic seed/clock/ID injection, capture and substitution variables, wildcard/predicate assertions, expected tool schemas, explicit ordering.
- Abstract the runner over a `ConformanceTarget`/client connector so it can drive in-memory and HTTP targets.
- Add a low-level client request API capable of sending task and `_meta`.
- Use semantic related-task assertions through `CallToolResult::related_task()`.
- Execute independent cases against fresh server instances; use explicit scenarios only when state sharing is required.
- Add negative tests proving the harness fails on extra tools, schema drift, missing guards, and malformed fixtures.

## Risk assessment

**HIGH.** The testing objective is excellent, but neither the existing fixture schema nor the proposed runner interface can express or execute it.

---

# 109-08 — Binding, compliance, example, subprocess smoke

## Summary

Closing with compliance, a narrative example, and an out-of-process smoke test is good phase hygiene. The compliance command is misunderstood, the contract-first ordering is reversed, and the example depends on unresolved runtime/team dispatch behavior. A single binary smoke test is also weaker than TEAM-01's "all four runnable binaries" claim.

## Strengths

- Makes compliance operational rather than leaving passive metadata.
- Includes a deterministic end-to-end example.
- Uses a real subprocess rather than only in-process tests.
- Bounds the child process and requires cleanup.
- Keeps the smoke path offline.
- The example tells a coherent business-facing story.

## Concerns

- **HIGH:** `pmat comply check contracts/team-servers/binding.yaml` is not valid. `comply check` accepts a project path (`--path`), not a binding-file positional argument.
- **HIGH:** `pmat comply check` is project compliance checking, not necessarily direct validation of this binding file. `refresh-bindings` and/or `pv lint` appears closer to the intended contract-binding workflow.
- **HIGH:** Contract compliance is deferred until after implementation, violating the repository's contract-first requirement to check before and after code changes.
- **HIGH:** The proposed `command -v pmat` warning makes a supposedly mandatory gate pass vacuously. That contradicts the claim that drift becomes gate-blocking.
- **HIGH:** The doc-review example depends on the unresolved TeamRuntime package resolution, `_meta`, task, and FixedSource issues.
- **MEDIUM:** Binding builder functions to whole equations may not prove individual contract obligations unless PMAT understands the binding schema and source signatures.
- **MEDIUM:** Only team-fs is launched. TEAM-01 states all four binaries are runnable.
- **MEDIUM:** Raw stdio framing is assumed to be newline-delimited without first confirming the SDK stdio transport framing.
- **MEDIUM:** `env!("CARGO_BIN_EXE_team-fs")` must remain behind compile-time feature cfg or default-feature quality-gate builds may fail.
- **LOW:** Adding a new mandatory step to the large root quality gate may materially increase iteration time; its scope and caching should be measured.

## Suggestions

- Probe and document the actual contract workflow before implementation (`pmat comply refresh-bindings`, `pmat comply check --strict --path .`, relevant `pv lint`).
- Make contract tooling fail closed in CI. If local PMAT is optional, separate the local convenience target from the mandatory CI target.
- Move initial binding generation/validation into Wave 1, then rerun it in Wave 4.
- Parameterize subprocess smoke coverage across all four binaries, using deterministic injected packages and sources.
- Reuse the SDK's actual stdio client/transport rather than hand-writing assumed JSON framing.
- Add a negative compliance test proving a deliberately broken binding is rejected.

## Risk assessment

**HIGH.** The compliance mechanism is currently incorrect, and the example/smoke work depends on unresolved upstream plans.

---

# Codex cross-plan recommendations

Before executing the eight plans, add a short prerequisite plan with explicit proof spikes:

1. Add extensible `tools/call` request metadata and propagate it to handlers.
2. Add a client API capable of sending both task augmentation and `_meta`.
3. Decide the member-task forwarding contract: return task, poll to result, or produce a synchronous related-task result.
4. Correct all related-task keys to `io.modelcontextprotocol/related-task`.
5. Define member identity using fields that actually exist in `TeamPackage`.
6. Introduce a package resolver for `ComponentRef → AgentPackage`.
7. Replace "missing LLM slot" fallback with an explicit test/runtime override.
8. Design approval state storage and owner semantics.
9. Version the conformance fixture schema for state, captures, deterministic IDs, and tool schemas.
10. Confirm the PMAT/provable-contract command chain and run it before implementation.
11. Add valid verification commands, `make quality-gate`, PMAT quality-proxy writes, fuzz, and feature-matrix checks to every committing plan.

With those changes, the four-server decomposition and overall phase architecture should be viable. Without them, Plans 05–08 are likely to stall or force substantial mid-phase redesign.

---

## Gemini Review

# Phase 109 Plan Review: Team Reference Servers

This review provides structured feedback on the quality, completeness, and risks of the implementation plans (`109-01-PLAN.md` through `109-08-PLAN.md`) for **Phase 109: Team Reference Servers**.

---

## 1. Quality Assessment

The implementation plans are structured around a 4-wave rollout that logically builds from basic crate scaffolding to server implementations, composition wiring, and conformance testing.

### Key Strengths
- **Clean Feature-Gated Modular Layout**: Features (`team-fs`, `mem-mcp`, `approval-mcp`, `team-mcp`, `conformance`, `webhook`, `http`) isolate server-specific dependencies, ensuring that the crate remains WASM-clean and does not compile heavy HTTP server code (`streamable-http`) by default.
- **TDD & Invariant Testing Discipline**: Standardizing on `proptest` for `derive_attachment` rules and guard state validates pure behavior under adversarial conditions.
- **Exhaustive Automated Fuzzing**: Incorporating `cargo-fuzz` targets for `fs_resolve` and `team_guards` early protects file paths and recursion boundaries against crashes.

### Areas for Improvement
- **Task Ordering & Dependencies**: The plans do not explicitly specify which task commits should trigger CI vs. local gates. Since `make comply` is introduced in Wave 4, intermediate commits in Waves 1–3 might drift without a check. Recommend running `cargo test -p pmcp-team-servers` after every task.

---

## 2. Completeness Verification

Mapping of Phase 109 requirements (**TEAM-01** to **TEAM-06**) and locked decisions (**D-01** to **D-20**) to the plans:

| Requirement / Decision | Plan Reference | Status / Completeness |
| :--- | :--- | :--- |
| **TEAM-01**: Crate & binaries | `109-01`, `109-08` | Complete. Crate scaffolded; 4 dev binaries created. |
| **TEAM-02**: team-fs server | `109-02` | Complete. Trait seams, `LocalDirBackend`, 11 `fs__*` tools. |
| **TEAM-03**: approval-mcp | `109-04` | Complete. `InMemoryTaskStore`, console/webhook channels. |
| **TEAM-04**: mem-mcp server | `109-03` | Complete. Trait seams, zero-dep BM25 keyword index. |
| **TEAM-05**: team-mcp server | `109-05` | Complete. In-process agent clients, verbatim re-emit. |
| **TEAM-06**: Conformance tests | `109-07`, `109-08` | Complete. Fixture schema validation, `pmat comply` target. |
| **D-01 to D-04**: Wiring & binary flags | `109-01`, `109-06`, `109-08` | Complete. `--package` flag, DuplexTransport socketless runs. |
| **D-05 to D-07**: Derived attachment | `109-01` | Complete. `derive_attachment` pure fn + property tests. |
| **D-08 to D-12**: Dev-backend fidelity | `109-02`, `109-04` | Complete. `file://` download URIs, console resolver tool. |
| **D-13 to D-16**: Member agent loop | `109-05` | Complete. SlotResolver LLM, depth headers mapped. |
| **D-17 to D-20**: Conformance | `109-07`, `109-08` | Complete. `binding.yaml` validation, complete guard fixtures. |

---

## 3. Critical Execution Risks & Mitigations

### 🚨 Risk 1: Namespaced `_meta` Serialization Blocker (Critical)
* **Threat Reference**: `T-109-05-05` (Guard bypass / Cycle escalation)
* **Problem**:
  1. Decision `D-14` states that depth and ancestor-chain guard state travels in the namespaced `_meta` on `tools/call`.
  2. The SDK's client call methods (`call_tool` in `src/client/mod.rs:577` and `call_tool_with_task` in `src/client/mod.rs:624`) both construct requests with `_meta: None` hardcoded. There is no client API parameter to supply custom metadata.
  3. The `CallToolRequest` struct (in `src/types/tools.rs:454`) defines `_meta` as `Option<RequestMeta>`.
  4. `RequestMeta` (in `src/types/protocol/mod.rs:315`) is a strongly-typed struct with only `progress_token` and `_task_id` fields, and has *no* `#[serde(flatten)]` map.
  5. **Result**: Any custom keys under `_meta` (like `x-pmcp-team-depth` or `caller_member_id`) are silently discarded during JSON deserialization. Even if team-mcp bypassed `Client` to write custom JSON to the transport, the member server would deserialize it into `CallToolRequest` and lose the guard values, rendering depth and cycle checks non-functional.
* **Mitigation (Recommended)**: Modify `pmcp` core in Wave 1:
  - Add `#[serde(flatten)] pub other: serde_json::Map<String, serde_json::Value>` to `RequestMeta` to preserve extra metadata keys.
  - Implement a `call_tool_with_meta_and_task` method in `Client` (or update `call_tool_with_task` to take an optional metadata map).
* **Mitigation B**: Change the transport mechanism to pass the guard parameters as special keys inside the `arguments` map instead of `_meta`.

### ⚠️ Risk 2: `LocalDirBackend` Path Containment Parent Non-Existence
* **Threat Reference**: `T-109-02-01` (Path Traversal / Escape)
* **Problem**: `109-02` states it "joins against the base root, canonicalizes the PARENT, and asserts the result starts_with the canonical base root". If a write targets a path whose parent directories do not exist yet (e.g. `fs__write` to `nested/dir/file.txt`), `std::fs::canonicalize(parent)` returns `NotFound` and fails — preventing new nested subdirectories, or forcing side-effects (creating the directory) before validating containment, which violates the security model.
* **Mitigation**: Implement a pure lexical path cleaner in `src/fs/local.rs` that resolves `.` and `..` in-memory without contacting disk; join against the canonical root and verify containment *before* any filesystem operation.

### ⚠️ Risk 3: Zero-Document and Negative IDF Edge Cases in BM25 Scorer
* **Threat Reference**: `T-109-03-01` (Scorer DoS / Division by Zero)
* **Problem**: BM25 uses average document length (L_avg) in the denominator. If the store or documents are empty, L_avg = 0 → division-by-zero crash. If a term appears in more than half the documents, standard IDF `ln((N - n + 0.5)/(n + 0.5))` returns a negative value, penalizing common-word matches.
* **Mitigation**: Add bounds check (if L_avg == 0, short-circuit to 0.0). Use smoothed IDF `ln(1.0 + (N - n + 0.5)/(n + 0.5))` or floor IDF at 0.0 (`idf.max(0.0)`).

### ⚠️ Risk 4: Webhook Channel Hanging & Error-Failures
* **Threat Reference**: `T-109-04-02` (Egress Hang / Denial of Service)
* **Problem**: `109-04` configures the Webhook channel to send a `reqwest` POST. If the receiver is offline/unresponsive, the request can hang indefinitely, blocking the server task; an HTTP error could block the agent flow.
* **Mitigation**: Configure `reqwest::Client` with a strict connection and response timeout (2–5s). Treat webhook failures as non-blocking: `tracing::warn!` and proceed, since resolution is managed out-of-band via `resolve_approval`.

---

## 4. Wave-by-Wave Execution Recommendations

- **Wave 1 (Scaffolding)**: Ensure `crates/pmcp-team-servers` is in the root `[workspace] members` list; exclude the inner `fuzz/` directory from the workspace so it is not built during standard workspace test sweeps.
- **Wave 2 (Server Implementations)**: Ensure all server handlers return detailed, contract-specified error messages on error states (invalid arguments, missing keys) to pass the conformance runners.
- **Wave 3 (Runtime Wiring)**: Validate that `DuplexTransport::pair()` socketless setup handles clean shutdown and does not leak spawned tasks; add a drop handler or task manager to clean up member servers.
- **Wave 4 (Integration & Comply)**: Ensure the compliance script runs gracefully when `pmat` is absent (`command -v pmat`); confirm the schema-property mapping before checking compliance.

---

## Consensus Summary

Both reviewers independently rate the overall phase **HIGH risk** and agree the architecture/decomposition is sound but several plans encode SDK assumptions that do not match the actual APIs. The strongest signal is a **shared, independently-derived critical finding**: the namespaced `_meta` guard-state mechanism at the heart of TEAM-05 does not work with the current `RequestMeta` type or client call APIs.

### Agreed Strengths
- Clean, feature-gated modular crate layout that keeps the default build WASM-clean and per-server isolated (both).
- Strong TDD/property + fuzz discipline (`derive_attachment` props, `fs_resolve` and `team_guards` fuzz targets) (both).
- Correct decomposition: four server surfaces + shared transport + composition + conformance, with sensible Wave-1 file ownership (both).
- Zero-dep in-memory BM25 scorer is the right call for a dev reference backend (both).

### Agreed Concerns (highest priority — raised by BOTH reviewers)
1. **[CRITICAL] Namespaced `_meta` guard state is not representable.** `RequestMeta` has only `progress_token` + `_task_id`, no `#[serde(flatten)]` catch-all, so custom depth/caller/ancestor keys are silently dropped on deserialization; and `Client::call_tool_with_task` hardcodes `_meta: None`, so it cannot send them. This breaks TEAM-05's depth/cycle guards and the `related_task` acceptance test. Both recommend a Wave-1/prerequisite SDK enablement change (extend `RequestMeta` + a client API that forwards `_meta`), with Gemini offering an alternative of smuggling guard state inside `arguments`.
2. **[HIGH] `LocalDirBackend` path containment is unsafe/insufficient.** "Canonicalize the parent" fails when nested parents don't exist yet, and canonicalize-then-IO is TOCTOU/symlink-vulnerable. Both recommend a pure lexical normalizer that verifies containment *before* any filesystem side-effect (Codex adds: reject symlinks / use open-at semantics).

### Additional HIGH concerns (Codex, deeper repo inspection — worth investigating)
- The related-task wire key is `io.modelcontextprotocol/related-task`, not `related_task` — freezing the wrong key into contract v1.1.0 causes immediate conformance drift.
- `call_tool_with_task` returns `ToolCallResponse::Task` (not a synchronous `CallToolResult`) for task-required tools — 109-05/07 only handle the `Result` path.
- `AgentPackage.llm` is mandatory → the "no LLM slot → FixedSource fallback" branch in 109-05/06 is unreachable; replace with an explicit injected factory override for tests/examples.
- `TeamPackage` holds `ComponentRef`s, not resolved `AgentPackage`s, and no package resolver is supplied to the binaries or `TeamRuntime` (109-05/06).
- `InMemoryTaskStore` has no approval-domain fields and is owner-scoped, conflicting with D-10 "resolve from any client" (109-04) — needs an explicit approval repository/adapter and owner policy.
- Existing Phase 107 fixtures contain hard-coded generated IDs and assumed state; fixture schema v1 has no setup/capture/deterministic-ID/expected-schema support, so it can't be replayed live (109-07).
- `pmat comply check` takes a project `--path`, not a binding-file argument, and is deferred post-implementation (violating contract-first); the `command -v pmat` guard makes a "mandatory" gate pass vacuously (109-08).

### Divergent Views
- **`_meta` mitigation:** Codex prescribes only the SDK-core route (extend `RequestMeta` + new client method, plus deciding the member-task forwarding contract). Gemini offers a lighter-weight alternative — carry guard params inside the `arguments` map — avoiding an SDK-core change at the cost of contract cleanliness.
- **Depth of the API mismatch:** Gemini rates the phase broadly executable with four targeted fixes and treats the completeness matrix as "Complete" per requirement. Codex, from deeper repository inspection, argues Plans 05–08 are *not executable as written* and require a prerequisite "109-00 SDK enablement" plan (package resolver, member identity, task-forwarding contract, fixture schema v2, PMAT command chain) before execution. Codex is the more conservative read; its API-level claims (`_meta: None` hardcoding, related-task key, mandatory `llm`, `ComponentRef` vs `AgentPackage`) are concrete and independently checkable against the cited source locations.

### Recommended next step
Verify the concrete API claims against the cited source (`src/client/mod.rs:577,624`, `src/types/protocol/mod.rs:315`, `src/types/tools.rs:454`, the `RELATED_TASK_META_KEY` constant, `AgentPackage.llm`, `TeamPackage`/`ComponentRef`). If confirmed, replan via `/gsd:plan-phase 109 --reviews` — most likely adding a prerequisite Wave-1 "SDK enablement" plan (extensible `_meta` + client forwarding + correct related-task key + package resolver + member identity) and correcting the `pmat comply` invocation to contract-first ordering.
