---
phase: 76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep
verified: 2026-04-22T00:00:00Z
status: passed
score: 13/13
overrides_applied: 0
---

# Phase 76: cargo-pmcp IAM Declarations — Verification Report

**Phase Goal:** Ship pmcp-run CR `CLI_IAM_CHANGE_REQUEST.md` in one phase — Part 1 adds a stable `McpRoleArn` CfnOutput to both generated CDK stack templates (pmcp-run + aws-lambda). Part 2 adds an optional `[iam]` section to `.pmcp/deploy.toml` with three repeated tables (tables/buckets/statements) that translate to `addToRolePolicy` calls, plus `cargo pmcp validate deploy` that hard-errors on IAM footguns. Backward compatible (D-05 byte-identity invariant). Target: cargo-pmcp 0.10.0.

**Verified:** 2026-04-22
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                          | Status     | Evidence                                                                                                                           |
|----|-----------------------------------------------------------------------------------------------|------------|------------------------------------------------------------------------------------------------------------------------------------|
| 1  | McpRoleArn CfnOutput present in pmcp-run branch with export `pmcp-${serverId}-McpRoleArn`     | VERIFIED   | `init.rs:677-680`; `tests/golden/pmcp-run-empty.ts:147,150`                                                                       |
| 2  | McpRoleArn CfnOutput present in aws-lambda branch with export `pmcp-${serverName}-McpRoleArn` | VERIFIED   | `init.rs:785-788`; `tests/golden/aws-lambda-empty.ts:96,99`                                                                       |
| 3  | aws-iam TS import present in aws-lambda branch (D-03)                                         | VERIFIED   | `tests/golden/aws-lambda-empty.ts:5` — `import * as iam from 'aws-cdk-lib/aws-iam';` is NEW addition                              |
| 4  | `render_stack_ts` pure-string accessor exists (`pub #[doc(hidden)]`)                          | VERIFIED   | `init.rs:513-514` — `#[doc(hidden)] pub fn render_stack_ts`                                                                       |
| 5  | `IamConfig` has `tables: Vec<TablePermission>`, `buckets: Vec<BucketPermission>`, `statements: Vec<IamStatement>` | VERIFIED | `config.rs:743-755` — three-vector schema, all sub-structs defined at 791, 816, 835                                   |
| 6  | `is_empty` checks all three vectors; `skip_serializing_if = "IamConfig::is_empty"` preserves D-05 | VERIFIED | `config.rs:773-775` — `self.tables.is_empty() && self.buckets.is_empty() && self.statements.is_empty()`; `config.rs:33` — `#[serde(default, skip_serializing_if = "IamConfig::is_empty")]` |
| 7  | `render_iam_block(&IamConfig) -> String` wired into `render_stack_ts` in BOTH template branches | VERIFIED | `init.rs:525` — `let iam_block = crate::deployment::render_iam_block(iam);`; placeholders at `init.rs:651` (pmcp-run) and `init.rs:767` (aws-lambda) |
| 8  | Empty IamConfig → empty `{iam_block}` (D-05 byte-identity)                                   | VERIFIED   | `deployment/iam.rs:337-338` — early return `String::new()` on `iam.is_empty()`; golden files pass byte-identical (Wave 1-5 SUMMARY) |
| 9  | `validate(&IamConfig) -> Result<Vec<Warning>>` with 6+ hard-error rules incl. wildcard-escalation | VERIFIED | `deployment/iam.rs:148` — `pub fn validate`; 11 `return Err(anyhow!(...))` paths; wildcard escalation at line 253-255             |
| 10 | `cargo pmcp validate deploy` CLI subcommand                                                   | VERIFIED   | `commands/validate.rs:39` — `ValidateCommand::Deploy { server }` variant; dispatch at line 52; handler `validate_deploy` at line 531 |
| 11 | `cargo pmcp deploy` fails closed after `DeployConfig::load`, before AWS call                  | VERIFIED   | `commands/deploy/deploy.rs:39-46` — IAM validate gate at lines 45-46, before `BinaryBuilder::build` at line 55                    |
| 12 | `cargo-pmcp/Cargo.toml` version = 0.10.0                                                      | VERIFIED   | `cargo-pmcp/Cargo.toml:3` — `version = "0.10.0"`                                                                                  |
| 13 | CLAUDE.md ALWAYS coverage: fuzz target + runnable example + property tests + unit tests        | VERIFIED   | Fuzz: `fuzz/fuzz_targets/fuzz_iam_config.rs` registered in `fuzz/Cargo.toml:34-35`; Example: `examples/deploy_with_iam.rs`; Property: 10 proptests in `deployment/iam.rs:1093-1380`; Unit: 14 unit tests in `deployment/iam.rs` + 29 validator tests in Wave 4 |

**Score:** 13/13 truths verified

### Required Artifacts

| Artifact                                                         | Expected                                    | Status    | Details                                                        |
|------------------------------------------------------------------|---------------------------------------------|-----------|----------------------------------------------------------------|
| `cargo-pmcp/src/commands/deploy/init.rs`                        | McpRoleArn in both branches + render_stack_ts + iam_block | VERIFIED | 2 CfnOutput emissions, 2 iam_block placeholders, pub render_stack_ts at line 514 |
| `cargo-pmcp/src/deployment/config.rs`                           | Full IamConfig 3-vector schema + is_empty + skip_serializing_if | VERIFIED | IamConfig at line 743, 3 sub-structs, is_empty at 773, serde wiring at 33 |
| `cargo-pmcp/src/deployment/iam.rs`                              | render_iam_block + validate + Warning + 6 hard-error rules | VERIFIED | render_iam_block at 336, validate at 148, Warning at 52, 11 Err paths |
| `cargo-pmcp/src/deployment/mod.rs`                              | `pub mod iam` + `pub use iam::render_iam_block` | VERIFIED | Line 3 and 14 respectively                                     |
| `cargo-pmcp/src/commands/validate.rs`                           | ValidateCommand::Deploy + validate_deploy    | VERIFIED  | Deploy variant at 39, handler at 531, 4 gate tests             |
| `cargo-pmcp/src/commands/deploy/deploy.rs`                      | Fail-closed IAM gate before AWS call         | VERIFIED  | Lines 45-46, before BinaryBuilder at line 55                   |
| `cargo-pmcp/tests/golden/pmcp-run-empty.ts`                     | D-05 byte-identity anchor (153 lines)        | VERIFIED  | 153 lines, McpRoleArn at 147, aws-iam import at 4              |
| `cargo-pmcp/tests/golden/aws-lambda-empty.ts`                   | D-05 byte-identity anchor (102 lines)        | VERIFIED  | 102 lines, McpRoleArn at 96, D-03 aws-iam import at 5          |
| `cargo-pmcp/fuzz/fuzz_targets/fuzz_iam_config.rs`               | libfuzzer target (T-76-03 mitigation)        | VERIFIED  | File exists, registered in fuzz/Cargo.toml:34-35               |
| `cargo-pmcp/fuzz/corpus/fuzz_iam_config/seed_*.toml`            | 3 corpus seeds                               | VERIFIED  | 3 seed files present                                           |
| `cargo-pmcp/examples/deploy_with_iam.rs`                        | Runnable example (cost-coach-shaped)         | VERIFIED  | File exists, SUMMARY confirms exit 0 with rendered output      |
| `cargo-pmcp/examples/fixtures/cost-coach.deploy.toml`           | Reference fixture                            | VERIFIED  | File exists                                                    |
| `cargo-pmcp/DEPLOYMENT.md`                                       | `## IAM Declarations` section                | VERIFIED  | Line 1194                                                      |
| `cargo-pmcp/CHANGELOG.md`                                        | `[0.10.0]` entry                             | VERIFIED  | Line 8                                                        |
| `cargo-pmcp/README.md`                                           | IAM pointer to DEPLOYMENT.md                 | VERIFIED  | Line 22                                                        |
| `cargo-pmcp/src/lib.rs`                                          | Narrow `#[path]` deployment exposure         | VERIFIED  | Lines 18-27 — `pub mod deployment { pub mod config; pub mod iam; }` |

### Key Link Verification

| From                            | To                                      | Via                              | Status   | Details                                                |
|---------------------------------|-----------------------------------------|----------------------------------|----------|--------------------------------------------------------|
| `render_stack_ts`               | `render_iam_block`                      | `crate::deployment::render_iam_block(iam)` | WIRED | `init.rs:525`                                        |
| `render_stack_ts` (pmcp-run branch) | `{iam_block}` placeholder          | named interpolation              | WIRED    | `init.rs:651`                                          |
| `render_stack_ts` (aws-lambda branch) | `{iam_block}` placeholder        | named interpolation              | WIRED    | `init.rs:767`                                          |
| `DeployExecutor::execute`       | `validate(&config.iam)`                 | `crate::deployment::iam::validate` | WIRED  | `deploy.rs:45-46`                                      |
| `ValidateCommand::Deploy`       | `validate_deploy`                       | dispatch arm                     | WIRED    | `validate.rs:52`                                       |
| `validate_deploy`               | `validate(&config.iam)`                 | `crate::deployment::config::DeployConfig::load` | WIRED | `validate.rs:547+`                            |
| `deployment::mod.rs`            | `render_iam_block`                      | `pub use iam::render_iam_block`  | WIRED    | `deployment/mod.rs:14`                                 |

### Data-Flow Trace (Level 4)

The primary data flow is code-generation (Rust → TypeScript string), not UI rendering. Tracing the critical path:

| Artifact           | Data Variable | Source                          | Produces Real Data | Status   |
|--------------------|---------------|---------------------------------|--------------------|----------|
| `render_stack_ts`  | `iam_block`   | `render_iam_block(iam)`         | Yes — translates `IamConfig` vectors to `addToRolePolicy` TS strings | FLOWING |
| `validate_deploy`  | `config.iam`  | `DeployConfig::load(&project_root)` | Yes — parses `.pmcp/deploy.toml` from disk | FLOWING |
| `DeployExecutor`   | `warnings`    | `validate(&config.iam)`         | Yes — real validation against parsed config | FLOWING |

### Behavioral Spot-Checks

| Behavior                                              | Verification method                                                  | Result   | Status |
|-------------------------------------------------------|----------------------------------------------------------------------|----------|--------|
| render_iam_block returns empty string for default IamConfig | `iam.rs:337-338` — `if iam.is_empty() { return String::new(); }` | Confirmed by code inspection | PASS |
| DeployExecutor gate runs before BinaryBuilder         | `deploy.rs:39-55` — validate call at 45-46 precedes `builder.build()` at 55 | Confirmed by line ordering | PASS |
| wildcard escalation hard-errors                        | `iam.rs:253-255` — explicit error message `wildcard escalation footgun` | Confirmed, also tested in 76-04 validate_tests | PASS |
| `cargo run --example deploy_with_iam` exits 0         | Wave 5 SUMMARY verification evidence block shows exit 0 + rendered output | SUMMARY evidence | PASS |
| fuzz target smoke (170K runs, zero panics)            | Wave 5 SUMMARY: `Done 170942 runs in 11 second(s) (zero panics)` | SUMMARY evidence | PASS |
| `make quality-gate` passes                            | Wave 5 SUMMARY: `ALL TOYOTA WAY QUALITY CHECKS PASSED` (exit 0); `make lint` uses `--lib --tests` not `--all-targets` so pre-existing pentest clippy errors in unmodified files do not block | SUMMARY evidence | PASS |

### Requirements Coverage

| Requirement ID | Source Plan   | Description                                          | Status    | Evidence                                                     |
|----------------|---------------|------------------------------------------------------|-----------|--------------------------------------------------------------|
| PART-1         | 76-01-PLAN.md | McpRoleArn CfnOutput in both template branches + render_stack_ts seam | SATISFIED | Truths 1-4 above; golden files pinned                        |
| PART-2         | 76-02 through 76-04-PLAN.md | Full IamConfig schema + render_iam_block + validator + CLI subcommand + deploy gate | SATISFIED | Truths 5-11 above; 779 tests passing (Wave 5 SUMMARY)        |

**Note on REQUIREMENTS.md traceability:** PART-1 and PART-2 are CR-local IDs defined in the ROADMAP.md and plan frontmatter. They are not present in the shared `.planning/REQUIREMENTS.md` traceability table (which tracks v2.1/v2.0 milestone requirements under their own ID namespaces: EXMP-, PROT-, MACR-, etc.). Phase 76 was added to the ROADMAP after the REQUIREMENTS.md traceability section was established. This is an informational gap — the shared requirements file was not updated to include Phase 76's PART-1/PART-2 — but it does not represent a functional deficiency. All ROADMAP success criteria for Phase 76 are satisfied.

### Anti-Patterns Found

| File                    | Line | Pattern                        | Severity | Impact                                                                 |
|-------------------------|------|--------------------------------|----------|------------------------------------------------------------------------|
| `deployment/iam.rs`     | 337-338 | `return String::new()` on empty IamConfig | Info | Intentional — required for D-05 byte-identity; not a stub                |
| Pre-existing files (pentest/, secrets/, deployment/metadata.rs) | Various | 20 pre-existing clippy errors (`cargo clippy --all-targets -D warnings`) | Warning | Documented in `deferred-items.md`; do NOT affect `make quality-gate` (which uses `--lib --tests` scope) or any Phase 76 new code |

No blockers. The 20 pre-existing clippy errors are confined to files not touched by Phase 76 (`pentest/`, `secrets/`, `deployment/metadata.rs`) and are excluded from the Makefile's `lint` target (`--lib --tests` scope, not `--all-targets`).

### Human Verification Required

None. All must-haves are verifiable programmatically via code inspection and test evidence.

## Gaps Summary

No gaps. All 13 must-haves verified against the actual codebase:

- PART-1: McpRoleArn CfnOutput with correct export names present in both template branches in `init.rs`, confirmed by D-05 golden files. D-03 aws-iam import added to aws-lambda branch. `render_stack_ts` pure-string seam is `pub #[doc(hidden)]`.
- PART-2: Full three-vector `IamConfig` schema with `is_empty` and `skip_serializing_if` wiring. `render_iam_block` wired into both template branches via `{iam_block}` named placeholder. `validate` with 11 hard-error return paths including wildcard-escalation. `ValidateCommand::Deploy` subcommand dispatches to `validate_deploy`. `DeployExecutor` fail-closed gate at lines 45-46 runs before any AWS call at line 55.
- Release: version 0.10.0, `make quality-gate` passed (Wave 5 SUMMARY).
- Coverage: fuzz target registered and smoke-tested at 170K runs, runnable example exits 0, 10 property tests, 14+ unit tests.

All phase commits verified present in git log (0d586740 through 3923834a across Waves 1-5).

---

_Verified: 2026-04-22_
_Verifier: Claude (gsd-verifier)_
