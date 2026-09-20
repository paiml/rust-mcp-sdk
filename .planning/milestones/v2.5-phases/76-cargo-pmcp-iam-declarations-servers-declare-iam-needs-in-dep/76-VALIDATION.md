---
phase: 76
slug: cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-22
revised: 2026-04-22 (B-01 reconciliation — task IDs now match final PLAN.md structure)
---

# Phase 76 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Observable correctness signals are enumerated in `76-RESEARCH.md` → *Validation Architecture*.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (unit + integration) + `proptest` (property) + `cargo-fuzz` with `libfuzzer-sys` (fuzz) |
| **Config file** | `cargo-pmcp/Cargo.toml` + `cargo-pmcp/fuzz/Cargo.toml` (both exist today) |
| **Quick run command** | `cargo test -p cargo-pmcp iam` |
| **Full suite command** | `make quality-gate` (CI parity — fmt --all, clippy pedantic+nursery, build, test, audit, doctests) |
| **Estimated runtime** | ~30s quick; ~5-8 min for `make quality-gate` |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p cargo-pmcp iam` (the quick subset)
- **After every plan wave:** Run `cargo test -p cargo-pmcp` (full crate tests)
- **Before `/gsd-verify-work`:** `make quality-gate` must be green end-to-end
- **Max feedback latency:** 30 seconds for per-task; 8 minutes for full gate

---

## Per-Task Verification Map

> **Revised 2026-04-22 (B-01):** IDs below map 1:1 to actual PLAN.md task anchors.
> Format: `{phase}-{plan}-T{task_index}` — e.g., `76-01-T2` = Phase 76, Plan 01, Task 2.
> All 9 original signals preserved; rows rekeyed against final task structure.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 76-01-T2 | 01 (role ARN export) | 1 | PART-1 | — | Stable CFN export unblocks bolt-on stacks without name guessing | integration (in-crate — InitCommand private fields) | `cargo test -p cargo-pmcp wave1_stack_ts_tests` | ❌ W0 (Task 1 creates the render_stack_ts seam; Task 2 adds the tests) | ⬜ pending |
| 76-01-T3 | 01 (backward compat) | 1 | D-05 | — | stack.ts emitted from no-`[iam]` config is byte-identical to pre-phase output (modulo the additive McpRoleArn CfnOutput) | integration (golden file) | `cargo test -p cargo-pmcp --test backward_compat_stack_ts` | ❌ W0 (Task 3 creates goldens + harness) | ⬜ pending |
| 76-02-T1 | 02 (IamConfig schema) | 2 | PART-2 schema | — | Full IamConfig/TablePermission/BucketPermission/IamStatement struct hierarchy; Wave-1 tests still pass against refined is_empty | unit (in-crate — iam_wave1_tests) | `cargo test -p cargo-pmcp iam_wave1_tests` | ✅ (Wave 1 Task 1 creates; Wave 2 Task 1 refines) | ⬜ pending |
| 76-02-T2 | 02 (IamConfig schema) | 2 | PART-2 schema | — | TOML parses cleanly; cost-coach-shaped fixture roundtrips; empty config omits `[iam]` table (D-05) | integration (serde round-trip) | `cargo test -p cargo-pmcp --test iam_config` | ❌ W0 (Wave 2 Task 2 creates) | ⬜ pending |
| 76-03-T1 | 03 (translation rules — unit) | 3 | PART-2 tables + buckets | T-76-01 (no silent privilege expansion) | read/write/readwrite emit exactly the CR-specified action lists (D-02 lock); S3 object-level ARN only | unit (in-crate in deployment::iam) | `cargo test -p cargo-pmcp deployment::iam` | ❌ W0 (Wave 3 Task 1 creates iam.rs + 13 unit tests) | ⬜ pending |
| 76-03-T3 | 03 (translation rules — property) | 3 | PART-2 tables | T-76-01 | Property: any valid IamConfig → one addToRolePolicy per declaration; read/write flags emit full action sets; toml roundtrip | property (proptest) | `cargo test -p cargo-pmcp --test iam_translation_props` | ❌ W0 (Wave 3 Task 3 creates prop harness) | ⬜ pending |
| 76-04-T1 | 04 (validator) | 4 | PART-2 validation | T-76-02 (wildcard escalation) | Allow/`*:*`/`*` hard-rejected; 6 hard-error rules + 2 warning classes per CR | unit + integration | `cargo test -p cargo-pmcp validate_tests && cargo test -p cargo-pmcp --test iam_validate` | ❌ W0 (Wave 4 Task 1 creates validator + iam_validate integration file) | ⬜ pending |
| 76-04-T2 | 04 (validator CLI gate) | 4 | PART-2 validation | T-76-02 | `cargo pmcp validate deploy` and `cargo pmcp deploy` exit non-zero on invalid config before touching AWS | integration | `cargo test -p cargo-pmcp --test deploy_validate_gate` | ❌ W0 (Wave 4 Task 2 creates gate test) | ⬜ pending |
| 76-05-T1 | 05 (fuzz) | 5 | PART-2 robustness | T-76-03 (parser DoS / crash) | IamConfig parser does not panic on adversarial TOML | fuzz | `(a) test -f cargo-pmcp/fuzz/fuzz_targets/fuzz_iam_config.rs && test -d cargo-pmcp/fuzz/corpus/fuzz_iam_config` (acceptance gate); `(b) cargo +nightly fuzz build fuzz_iam_config` (best-effort) | ❌ W0 (Wave 5 Task 1 creates target + corpus) | ⬜ pending |
| 76-05-T2 | 05 (example) | 5 | PART-2 DX | — | cost-coach-shaped example parses, validates, renders; wildcard-Allow rejection demonstrated | integration / example | `cargo run -p cargo-pmcp --example deploy_with_iam` | ❌ W0 (Wave 5 Task 2 creates example + fixture) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

> **Signal coverage:** 10 rows cover the original 9 signals from the first draft (signal 1 "role ARN export" split across 76-01-T2 emission tests and 76-01-T3 golden; signal 9 "backward compat" lands in 76-01-T3 — same row). No signal dropped.

---

## Wave 0 Requirements

Tests and harnesses that do not exist today and MUST be created before/during the first wave that needs them:

- [ ] `cargo-pmcp/src/commands/deploy/init.rs` `mod wave1_stack_ts_tests` — in-crate tests renaming InitCommand via `for_golden_test` or direct struct literal; asserts McpRoleArn CfnOutput + aws-iam import (Wave 1 Task 2)
- [ ] `cargo-pmcp/tests/backward_compat_stack_ts.rs` — golden-file test pinning no-`[iam]` stack.ts output; UPDATE_GOLDEN=1 regeneration flow (Wave 1 Task 3)
- [ ] `cargo-pmcp/tests/iam_config.rs` — integration serde roundtrip + cost-coach fixture + D-05 empty-serialize invariant (Wave 2 Task 2)
- [ ] `cargo-pmcp/src/deployment/iam.rs` `#[cfg(test)] mod tests` — 13 unit tests pinning translation rules (Wave 3 Task 1)
- [ ] `cargo-pmcp/tests/iam_translation_props.rs` — proptest strategies + 9 property tests on translation invariants (Wave 3 Task 3)
- [ ] `cargo-pmcp/src/deployment/iam.rs` `#[cfg(test)] mod validate_tests` + `cargo-pmcp/tests/iam_validate.rs` — validator unit + integration tests (Wave 4 Task 1)
- [ ] `cargo-pmcp/tests/deploy_validate_gate.rs` — integration test that invokes `validate_deploy()` on synthetic `.pmcp/deploy.toml` fixtures, asserts Ok/Err behavior (Wave 4 Task 2)
- [ ] `cargo-pmcp/fuzz/fuzz_targets/fuzz_iam_config.rs` + corpus seeds — libfuzzer target for `toml::from_str::<DeployConfig>` + `iam::validate` (Wave 5 Task 1)
- [ ] `cargo-pmcp/examples/deploy_with_iam.rs` + `examples/fixtures/cost-coach.deploy.toml` — runnable cost-coach-shaped walkthrough (Wave 5 Task 2)

*Infrastructure already present (no setup needed):* `proptest` in dev-deps, `libfuzzer-sys` + existing fuzz targets in `cargo-pmcp/fuzz/`, `regex` in main deps (no new ARN parser dependency required).

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Round-trip: emitted CFN template imports into a real CDK bolt-on stack via `Fn::ImportValue` and `grantReadWriteData` succeeds against a real DynamoDB table | PART-1 end-to-end | Requires a live AWS account; not suitable for CI | Deploy a test server with `cargo pmcp deploy --target pmcp-run`, then deploy a trivial bolt-on CDK stack that imports `pmcp-${serverName}-McpRoleArn` and grants RW on a table; observe the bolt-on `grantReadWriteData` call succeeds and the Lambda can write to the table |
| Cost-coach migration: replace the env-var-based `MCP_FUNCTION_ROLE_NAME` lookup with `Fn.importValue` in cost-coach's bolt-on stack and confirm IAM grants persist across a platform-stack redeploy | PART-1 real-world unblock | Live integration with cost-coach prod; cannot CI-run | (Post-merge task for cost-coach team, tracked separately — not blocking this phase's verification) |
| 60-second fuzz smoke (`cargo +nightly fuzz run fuzz_iam_config -- -max_total_time=60`) | PART-2 robustness | Requires nightly toolchain; non-blocking supplement to the acceptance gate in 76-05-T1 | Documented in Wave 5 Task 1; executor runs if nightly is installed locally; CI can extend to longer runs |

---

## Threat Model References

| Threat ID | Description | Mitigation |
|-----------|-------------|------------|
| T-76-01 | Silent privilege expansion — a future change to the action-set lookup maps could grant more AWS actions than the CR specifies without authors noticing | Property tests lock the exact action-set output per `read`/`write`/`readwrite` input; translation tests are golden on the full action list |
| T-76-02 | Wildcard escalation footgun — operator accidentally writes `[[iam.statements]]` with `effect=Allow` + `actions=["*"]` + `resources=["*"]` and ships it | Hard-error validation at `cargo pmcp validate` AND at `cargo pmcp deploy` entry points; refuses to synth or deploy |
| T-76-03 | Parser DoS — adversarial `.pmcp/deploy.toml` crafted to panic or hang the CLI | libfuzzer target exercises `IamConfig::from_str` / full `DeployConfig::load` path; 60-second fuzz run in quality gate |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (9 items above)
- [ ] No watch-mode flags in any test command
- [ ] Feedback latency < 30s for per-task quick command
- [ ] `nyquist_compliant: true` set in frontmatter after planner fills the per-task map

**Approval:** pending (planner has reconciled per-task map 2026-04-22 per B-01; executor-time verification next; operator signs after full gate green)
