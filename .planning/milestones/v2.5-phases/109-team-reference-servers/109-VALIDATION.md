---
phase: 109
slug: team-reference-servers
status: ready
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-18
updated: 2026-07-18
---

# Phase 109 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Updated 2026-07-18 (reviews replan): adds the prerequisite pmcp-core enablement plan 109-00
> (extensible `_meta` + `_meta`-forwarding client APIs) and re-maps every task to the revised set.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + proptest for property tests + cargo-fuzz targets |
| **Config file** | `crates/pmcp-team-servers/Cargo.toml` (Wave 1 / 109-01 creates crate); pmcp core (109-00) |
| **Quick run command** | `cargo test -p pmcp-team-servers --all-features` (+ `cargo test -p pmcp <filter>` for 109-00) |
| **Full suite command** | `make quality-gate` (now includes `make comply` graceful; CI adds `make comply-ci`) |
| **Estimated runtime** | ~60 seconds (quick) / ~10 minutes (full gate) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp-team-servers --all-features` (or `cargo test -p pmcp <filter>` for 109-00 pmcp-core tasks)
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

Every task across the 9 plans carries a `<verify><automated>` command (no MISSING references → Nyquist-compliant). Cargo test commands use a SINGLE positional filter per invocation (the review-flagged multi-filter commands are corrected).

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|--------|
| 109-00-T1 | 109-00 | 1 | TEAM-05/06 | T-109-00-02 | Extensible RequestMeta preserves namespaced keys; additive, unchanged existing serialization | tdd | `cargo test -p pmcp request_meta` | ⬜ pending |
| 109-00-T2 | 109-00 | 1 | TEAM-05/06 | T-109-00-01 | Client forwards custom _meta (task + non-task); observed server-side | tdd | `cargo test -p pmcp call_tool_with_meta` | ⬜ pending |
| 109-01-T1 | 109-01 | 1 | TEAM-01 | T-109-01-SC | Wasm-clean manifest, deliberate default features, stdio-buildable bins | auto | `cargo build -p pmcp-team-servers --no-default-features --features team-fs` | ⬜ pending |
| 109-01-T2 | 109-01 | 1 | TEAM-01 | T-109-01-SC | Empty documented modules (zero-SATD); PackageResolver + MemberId seams; central fuzz manifest | auto | `cargo test -p pmcp-team-servers team::identity` | ⬜ pending |
| 109-01-T3 | 109-01 | 1 | TEAM-01 | T-109-01-01 | derive_attachment total/pure (atomic impl) over adversarial pkg + opt-in dedup | tdd/proptest | `cargo test -p pmcp-team-servers --test derive_props` | ⬜ pending |
| 109-01-T4 | 109-01 | 1 | TEAM-01 | — | Additive contract rev w/ CORRECT related-task key; contract-first binding skeleton | auto | `cargo test --test team_contracts_conformance` | ⬜ pending |
| 109-02-T1 | 109-02 | 2 | TEAM-02 | T-109-02-01/03 | Pure lexical containment before I/O; symlink reject; percent-encoded file://; fuzz | tdd+fuzz | `cargo test -p pmcp-team-servers fs::` | ⬜ pending |
| 109-02-T2 | 109-02 | 2 | TEAM-02 | T-109-02-01 | Exact 11-tool surface; server-owned complete_task; HTTP via SDK (feature-gated) | auto | `cargo build -p pmcp-team-servers --features "team-fs http" --bin team-fs` | ⬜ pending |
| 109-03-T1 | 109-03 | 2 | TEAM-04 | T-109-03-01 | Zero-dep BM25 (L_avg==0 short-circuit, IDF floor); deterministic ids; bounded limits | tdd | `cargo test -p pmcp-team-servers mem` | ⬜ pending |
| 109-03-T2 | 109-03 | 2 | TEAM-04 | T-109-03-02 | Safe scorer invariants (non-neg, determinism, stable tie-break); exact 6-tool surface | auto/proptest | `cargo test -p pmcp-team-servers --test mem_props` | ⬜ pending |
| 109-04-T1 | 109-04 | 2 | TEAM-03 | T-109-04-01/02 | Console offline; webhook bounded-timeout non-blocking; secret non-leak; mock test | auto | `cargo test -p pmcp-team-servers approval::channels --features "approval-mcp webhook"` | ⬜ pending |
| 109-04-T2 | 109-04 | 2 | TEAM-03 | T-109-04-03/05 | ApprovalRepository domain state; service-owner; atomic option-validated resolve; subject ref | auto | `cargo test -p pmcp-team-servers approval::server` | ⬜ pending |
| 109-05-T1 | 109-05 | 2 | TEAM-05 | T-109-05-01/02/04 | Strict depth (absent=0, garbage=Error) from request_meta; MemberId guards; fuzz | tdd+fuzz | `cargo test -p pmcp-team-servers --test team_props` | ⬜ pending |
| 109-05-T2 | 109-05 | 2 | TEAM-05 | T-109-05-03/05 | Task+_meta hop; explicit Task/Result forwarding contract; related-task under RELATED_TASK_META_KEY; injected override | tdd | `cargo test -p pmcp-team-servers team::` | ⬜ pending |
| 109-05-T3 | 109-05 | 2 | TEAM-05 | T-109-05-05 | PackageResolver-driven members; SlotResolver LLM; depth header→_meta edge | auto | `cargo build -p pmcp-team-servers --features "team-mcp http" --bin team-mcp` | ⬜ pending |
| 109-06-T1 | 109-06 | 3 | TEAM-01 | T-109-06-02/03 | TeamRuntimeBuilder seams; cfg-gated + fail-closed attachment; transactional startup | auto | `cargo build -p pmcp-team-servers --no-default-features --features team-fs` | ⬜ pending |
| 109-06-T2 | 109-06 | 3 | TEAM-01 | T-109-06-01 | Small-team + team-of-one + fail-closed + clean shutdown on injected FixedSource | auto | `cargo test -p pmcp-team-servers --test small_team --all-features` | ⬜ pending |
| 109-07-T1 | 109-07 | 3 | TEAM-06 | T-109-07-01/03 | Fixture schema v2; ConformanceTarget (in-mem+HTTP); _meta send; semantic related-task; negative harness | auto | `cargo build -p pmcp-team-servers --features conformance` | ⬜ pending |
| 109-07-T2 | 109-07 | 3 | TEAM-06 | T-109-07-02 | Every-tool/every-guard v2 fixtures, deterministic ids, fresh servers | auto | `cargo test -p pmcp-team-servers --test conformance --all-features` | ⬜ pending |
| 109-08-T1 | 109-08 | 4 | TEAM-06 | T-109-08-01 | binding.yaml finalized (implemented, real signatures) + broken-binding fixture | auto | `grep -q "status: implemented" contracts/team-servers/binding.yaml` | ⬜ pending |
| 109-08-T2 | 109-08 | 4 | TEAM-06 | T-109-08-02 | Correct `pmat comply check --path .`; fail-closed comply-ci in CI; graceful comply; negative reject | auto | `make comply` | ⬜ pending |
| 109-08-T3 | 109-08 | 4 | TEAM-06 | — | Full four-server E2E on injected FixedSource, offline | auto | `cargo run -p pmcp-team-servers --example doc_review_team --all-features` | ⬜ pending |
| 109-08-T4 | 109-08 | 4 | TEAM-01 | T-109-08-03 | ALL FOUR dev binaries via SDK stdio client; bounded, no leak | auto | `cargo test -p pmcp-team-servers --test dev_binary_smoke --all-features` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

The Roadmap "Wave 0" spikes were resolved during planning; the CRITICAL `_meta` blocker surfaced by the cross-AI review is now a concrete prerequisite plan (109-00), not a deferred spike:

- [x] `crates/pmcp-team-servers/` — new workspace crate + seams (PackageResolver, MemberId) → **109-01**
- [x] **pmcp-core `_meta` enablement** (extensible `RequestMeta` + `RequestHandlerExtra.request_meta` + `call_tool_with_task_and_meta`/`call_tool_with_meta`) — the shared review-confirmed CRITICAL finding → **109-00 (Wave 1 prerequisite; 109-05/07 depend on it)**
- [x] Task-augmented + `_meta`-forwarding member hop with explicit Task/Result forwarding contract → **109-05 Task 2 (via 109-00)**
- [x] Conformance fixture schema v2 (kind/scenario/seed/capture/expected-schemas) + ConformanceTarget → **109-07 Task 1**
- [x] `pmat comply check --path .` correct invocation probed (109-01 Task 4) + wired fail-closed (109-08 Task 2)

No task carries an `<automated>MISSING` marker → no Wave-0 test-scaffold work is outstanding.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real-LLM (Ollama/Anthropic) member run per configured slot | TEAM-05 / D-15 | Requires a live LLM endpoint; CI path uses an injected FixedSource | Configure a member `AgentPackage` llm `ConfigSlot` (OLLAMA_BASE_URL / ANTHROPIC_API_KEY env), run `team-mcp --package <pkg>`, dispatch a `team_mcp__<member>` call, confirm a live completion |

> The "launch each dev binary" manual row is now AUTOMATED across ALL FOUR binaries: 109-08 Task 4 (`dev_binary_smoke` real subprocess via the SDK stdio client).

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies (23/23 tasks carry `<automated>`; single-filter cargo commands)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers the review-confirmed CRITICAL `_meta` blocker (now plan 109-00, not a deferred spike)
- [x] No watch-mode flags
- [x] Feedback latency < 120s (quick run ~60s)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved (plan-time validation contract updated for the reviews replan; per-task Status flips to ✅ during execution)
