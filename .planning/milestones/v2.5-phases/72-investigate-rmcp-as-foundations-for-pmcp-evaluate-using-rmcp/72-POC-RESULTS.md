# Phase 72 — PoC Slice 1 Execution Results (reviews-mode addition)

**Status:** COMPLETED
**Execution date:** 2026-04-19
**Executor:** Plan 02 Task 1b spike
**Branch used:** `spike/72-poc-slice-1` (DELETED at end of task — see Cleanup Confirmation)
**Wall-clock time:** ~15 minutes (well under 4-hour hard time-box)

**Why this file exists (per 72-REVIEWS.md HIGH-2):** without this task, T3 (enterprise extension-point coverage) and T4 (SemVer break count) would remain UNRESOLVED at Plan 03 synthesis, and the decision-tree would trigger DEFER for reasons unrelated to the evidence gathered. This task produces real measured data so T4 subcount (a) — compile error count against rmcp types — is resolved.

---

## Experiment Setup

- **Spike crate:** `examples/spike_72_rmcp_types/` (standalone; NOT a workspace member — root `Cargo.toml` not modified).
- **Dependency pin:** `rmcp = "=1.5.0"`, `serde = "1"`, `serde_json = "1"`.
- **Imports attempted:** `use rmcp::model::{JsonRpcRequest as RmcpRequest, Request as RmcpRequestEnvelope, RequestId};`
- **Compiler:** default toolchain on local machine (rust-mcp-sdk `rust-version = "1.83.0"`).

Note on type shape: `rmcp::model::JsonRpcRequest` in 1.5.0 is generic (`JsonRpcRequest<R>`), which means the caller must instantiate it with a concrete request envelope type (`rmcp::model::Request`). This is already a divergence from pmcp's simpler `JSONRPCRequest` surface — any "direct re-export" under Option A would still require at minimum a small facade to fix the type parameter.

---

## Measurements

### T4 — Compile errors (`cargo check --message-format=json`)

T4_compile_errors: 0

- Threshold from 72-DECISION-RUBRIC.md T4: ≤50 → Option A feasible; >200 → Option A disqualified.
- Outcome: Under threshold — A/B/C1 remain compile-feasible at the types layer.
- Warnings: 0.
- Build time: ~10s cold (dependency compilation dominated); ~0.03s incremental.
- Diagnostic summary: compilation succeeded on the first attempted import shape. No missing trait impls; no feature-flag gating issues; no name collisions.

### T4 — LOC delta

T4_loc_delta: 537  (= pmcp/src/types/jsonrpc.rs LOC 615 − spike total LOC 78)
T3_spike_loc: 78  (main.rs 65 + Cargo.toml 13)
T3_pmcp_jsonrpc_loc: 615

**Interpretation:** a minimum-viable re-export experiment for the JSON-RPC envelope costs 78 LOC of new code; the pmcp side currently pays 615 LOC for the same surface. Net reduction at this layer alone is ~537 LOC under a full adopt (A) direction. This is in the right order of magnitude for the 72-INVENTORY.md Totals row 1 estimate (~615 LOC deletable under A for row 1) — data consistency check PASSES.

### Serde roundtrip (canonical JSON-RPC 2.0 request shapes)

Running the spike binary with three canonical shapes produced:

| Attempt | Shape | Parse result |
|---|---|---|
| 1 | `{jsonrpc, id, method, params: null}` | FAIL — `invalid type: null, expected struct WithMeta` |
| 2 | `{jsonrpc, id, method}` (no `params` field) | FAIL — `missing field 'params'` |
| 3 | `{jsonrpc, id, method, params: {}}` | PASS |

Overall verdict: PARTIAL PASS — rmcp's `JsonRpcRequest<Request>` requires `params` to be a present object (not null, not omitted). This is a real serde-shape divergence from the JSON-RPC 2.0 wire spec, which treats `params` as OPTIONAL (may be absent or any value).

**Implication for Option A / B:** a direct re-export is NOT wire-compatible with off-the-shelf JSON-RPC 2.0 clients that omit `params` on method calls that take no arguments (common in `ping`, `initialized`, etc.). Either (a) rmcp generates `params: {}` on the client side uniformly, or (b) a pmcp facade must normalize incoming null/missing params before deserializing through rmcp's type. This is a concrete serde-compat risk that 72-INVENTORY.md row 1 rated "EXACT (pending Slice 1 spike confirmation)" — the spike now DOWNGRADES that row to "compatible-via-adapter" (requires a trivial params-normalizer).

---

## Implications for Decision Rubric

- **T4 subcount (a) — compile errors — is RESOLVED** at 0. Plan 03 consumes this value from this file directly.
- **T4 subcounts (b) and (c) remain UNRESOLVED** (broken examples and broken downstream workspace crates). A full-migration spike would be required to resolve them.
- **T3 remains UNRESOLVED** until Slice 2 is executed in a follow-on phase (Slice 2 was NOT executed — intentionally, per scope rule of Phase 72).
- Since `T4_compile_errors = 0 ≤ 50`: Option A is compile-feasible at the types layer. This is NOT sufficient evidence to recommend A (subcounts b and c are still unresolved, and the serde-shape divergence above means a facade layer is required), but it does disqualify the "A is infeasible" hypothesis.
- **New finding:** 72-INVENTORY.md row 1 serde-compat rating should be downgraded from "EXACT" to "compatible-via-adapter" (requires params-normalizer). Plan 03 should surface this finding in the "Strongest Counterargument" section.

---

## Cleanup Confirmation

- [x] Branch `spike/72-poc-slice-1` DELETED (`git branch -D spike/72-poc-slice-1`)
- [x] Directory `examples/spike_72_rmcp_types/` DOES NOT EXIST in the `main` working tree
- [x] Starting branch (`main`) is checked out
- [x] No rmcp dependency in root `Cargo.toml` or any published crate's `Cargo.toml`
- [x] `git status --porcelain` returns only this new `72-POC-RESULTS.md` file (plus pre-existing untracked `.claude/scheduled_tasks.lock` and `crates/pmcp-code-mode/IMPROVEMENTS.md` that are out of scope for this plan)

---

## Closes (partial)

T4 subcount (a) (from 72-DECISION-RUBRIC.md) — RESOLVED with real data: 0 compile errors.
T4 subcounts (b) and (c) — still unresolved (require a full-migration spike, deferred to future phase).
T3 — still unresolved (Slice 2 is proposal-only in Phase 72).

**Also produced as a side-effect:** a downgrade finding for 72-INVENTORY.md row 1 serde-compat (EXACT → compatible-via-adapter), which Plan 03 should surface.
