# Phase 72 — PoC Proposal (RMCP-EVAL-03, reviews-mode revised)

**Baseline pin:** rmcp 1.5.0 / pmcp current main.
**Scope rule:** every slice ≤500 LOC touched; at least one slice executable in ≤3 days.
**Execution posture:** Slice 1 is EXECUTED in Plan 02 Task 1b as a throwaway time-boxed spike (per reviews-mode revision). Slices 2 and 3 remain PROPOSALS scoped to a future migration-scoping phase (likely v3.0).
**Data feeds:** slice outcomes (real for Slice 1 via 72-POC-RESULTS.md, hypothetical for Slices 2–3) are named data sources for 72-DECISION-RUBRIC.md threshold rows.

---

### PoC Slice 1: Types re-export feasibility (EXECUTED in Plan 02 Task 1b)

LOC: 100

Files:
- examples/spike_72_rmcp_types/main.rs (NEW, scratch; lives on `spike/72-poc-slice-1` branch ONLY)
- examples/spike_72_rmcp_types/Cargo.toml (NEW, scratch; separate crate so main pmcp Cargo.toml is NOT modified)

Hypothesis tested: pmcp internal code can tolerate rmcp types being referenced directly without losing spec conformance or serde round-trip identity; compile-error delta against the re-exported baseline is ≤50.

Pass:
- `cargo check -p spike_72_rmcp_types --message-format=json 2>&1 | grep -c '"level":"error"'` returns ≤50
- `serde_json::to_value` roundtrip on a canonical `JsonRpcRequest` produces bit-identical JSON between rmcp's `JsonRpcRequest` and pmcp's `JsonRpcRequest` (test written inline in main.rs)

Fail:
- `cargo check` returns >200 errors OR JSON round-trip diverges OR required trait impls are missing AND cannot be added with a blanket impl

Time-box: 4 hours (HARD — if unfinished, record partial data with `status: TIMED_OUT`, delete branch/dir regardless)

Maps to strategy-matrix option: disambiguates A (Full adopt) vs B (Hybrid) vs C1 (types-only borrow). Fails means A and C1 are off the table; B is the only adopt path.

Cites 72-INVENTORY.md rows: 1 (jsonrpc, EXACT), 2 (protocol envelopes, EXACT).

Execution: see Plan 02 Task 1b.
Output artifact: 72-POC-RESULTS.md (T3_compile_errors, T4_loc_delta).

---

### PoC Slice 2: One typed tool on rmcp service layer (PROPOSAL — executed in future phase)

LOC: 400

Files:
- examples/z01_rmcp_foundation_poc.rs (NEW, one-off — not committed to main)
- Cargo.toml dev-dependencies (rmcp service feature)
- In-file adapter module wrapping pmcp's `ToolHandler` in an rmcp `ServerHandler`

Hypothesis tested: pmcp's TypedTool and `#[mcp_tool]` macro-generated surface composes on top of rmcp's `ServerHandler`/`Service` without losing its ergonomics.

Pass:
- Example compiles and runs under stdio transport via `rmcp::service::serve_server`
- `mcp-tester conformance run --target-example examples/z01_rmcp_foundation_poc.rs` returns 0 failures
- Adapter LOC ≤ 400

Fail:
- Conformance suite fails OR adapter grows past 400 LOC

Time-box: 2 days

Maps to strategy-matrix option: validates Option B (Hybrid).

Cites 72-INVENTORY.md rows: 4 (tools, EXACT), 21 (protocol framing helpers, Partial).

---

### PoC Slice 3: Workflow engine on rmcp Peer (PROPOSAL — executed in future phase)

LOC: 500

Files:
- src/server/workflow/task_prompt_handler.rs (feature-gated `#[cfg(feature = "rmcp-foundation")]` alternate impl)
- Cargo.toml (new optional feature `rmcp-foundation`)
- examples/s31_workflow_minimal.rs (used as end-to-end test, unchanged)
- examples/s33_workflow_dsl_cookbook.rs (used as end-to-end test, unchanged)

Hypothesis tested: pmcp's workflow engine runs on rmcp's `Peer<RoleServer>` without DSL regressions and with identical observable semantics.

Pass:
- `cargo run --example s31_workflow_minimal --features rmcp-foundation` emits identical event sequence vs default build
- `cargo run --example s33_workflow_dsl_cookbook --features rmcp-foundation` passes all in-example assertions

Fail:
- DSL semantics drift OR any workflow-specific Peer method is absent upstream

Time-box: 3 days

Maps to strategy-matrix option: validates Option B preserves pmcp workflow engine.

Cites 72-INVENTORY.md rows: 14 (mcp_apps), 22 (middleware), 24 (cancellation/RequestHandlerExtra).

---

## Slice Selection Recommendation

For the current phase: Slice 1 is EXECUTED now (cheap signal; 4-hour time-box).
For the future migration-scoping phase: Slice 2 next, Slice 3 only if workflow migration is on the critical path.

## Workspace Ripple Caveat

Every future slice must be executed with `--workspace`, not just on the pmcp root crate. Phase 72 Pitfall 5 names workspace ripple (mcp-tester, cargo-pmcp, pmcp-code-mode, pmcp-tasks) as the dominant migration cost.

## Data Feeds Into

- 72-DECISION-RUBRIC.md: Slice 1 compile-error count → T4 data source via 72-POC-RESULTS.md. Slice 2 mcp-tester conformance → T3 data source. All time-boxes → T5 data source.
- 72-RECOMMENDATION.md: T3/T4 are RESOLVED (not UNRESOLVED) for Plan 03 via 72-POC-RESULTS.md; T3 remains UNRESOLVED only if Slice 2 is not run (which is expected — Slice 2 is proposal-only).
