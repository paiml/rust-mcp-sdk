---
phase: 77
plan: 02
subsystem: cargo-pmcp/cli
tags: [cli, clap, refactor, deploy, target-flag]
dependency_graph:
  requires:
    - "77-01 (CHANGELOG entry documenting target-flag deprecation grace period)"
  provides:
    - "Cli.target field readable by Plan 06 resolver"
    - "DeployCommand.target_type field name stable for Plan 06+ references"
    - "Legacy `cargo pmcp deploy --target <type>` invocations continue to work via clap alias for one release cycle"
  affects:
    - "cargo-pmcp/src/commands/deploy/mod.rs"
    - "cargo-pmcp/src/main.rs"
tech_stack:
  added: []
  patterns:
    - "clap `#[arg(long = \"x\", alias = \"y\")]` for deprecation grace on flag rename"
    - "`#[arg(long)]` (non-global) when a global flag would collide with a same-named local alias on a subcommand"
key_files:
  created: []
  modified:
    - cargo-pmcp/src/commands/deploy/mod.rs
    - cargo-pmcp/src/main.rs
decisions:
  - "Dropped `global = true` on new top-level Cli.target flag — clap requires unique long-option names across the global+local namespace; the deploy subcommand owns `--target` as the deprecated alias for `--target-type`. Top-level resolution is sufficient since main.rs reads cli.target directly before dispatching (Plan 06 resolver)."
  - "Field naming: top-level Cli flag is `pub target` (named-target selector); deploy-scoped flag is `target_type` with `alias = \"target\"` (deployment backend selector). Both can coexist because they live in disjoint clap arg-id namespaces (global Cli vs. DeployCommand)."
  - "Replaced test fixture `status` subcommand → `outputs` (status requires <OPERATION_ID> positional, outputs has no required positionals — keeps tests focused on flag-parse behavior, not on fully-formed subcommand invocations)."
metrics:
  duration: "~25 minutes wall-clock"
  completed_date: "2026-04-26"
  tasks_completed: 2
  commits: 3  # one refactor + one test (RED) + one feat (GREEN)
  files_modified: 2
---

# Phase 77 Plan 02: Rename `--target` flag and add named-target flag — Summary

## One-liner

Renamed `DeployCommand.target` → `target_type` with `alias = "target"` deprecation grace, and added a new `pub target: Option<String>` field on top-level `Cli` to hold the Phase 77 named-target selector — the `--target` flag now means "named target from `~/.pmcp/config.toml`" at the top level while still meaning "deployment backend type" inside `cargo pmcp deploy ...` for one release cycle.

## What Shipped

### Task 1 — `refactor(77-02): rename DeployCommand.target → target_type with alias` (commit 83fbf633)

- Renamed the field declaration in `cargo-pmcp/src/commands/deploy/mod.rs` (line 92-96):
  - Before: `#[arg(long, global = true)] target: Option<String>`
  - After: `#[arg(long = "target-type", alias = "target", global = true)] target_type: Option<String>`
- Updated the doc comment to disambiguate "deployment target TYPE" from the new Phase 77 named-target meaning.
- Updated the single consumer site `if let Some(target) = &self.target` at line 744 → `&self.target_type`.
- Updated the `// Priority: --target flag > config file > default` comment → `--target-type`.

**Number of `self.target` references rewritten:** 1 (the plan estimated 8 based on RESEARCH Code Recon §3, but those were references to a local `target_id` variable, not `self.target` — only the field-load site at line 744 actually used `self.target`).

### Task 2 — `test/feat(77-02): add global --target flag` (commits e0f0710d + 894db1d5)

TDD cycle:
- **RED** (e0f0710d): added `cli_target_flag_tests` module with 7 tests; build fails E0609 "no field `target` on type `Cli`".
- **GREEN** (894db1d5): added `pub target: Option<String>` field to `Cli` with `#[arg(long)]`. All 7 tests pass.

## Verification Results

```
cargo build -p cargo-pmcp                        : exit 0 (11 pre-existing warnings, no errors)
cargo test -p cargo-pmcp --lib                   : 355/355 passed
cargo test -p cargo-pmcp --bin cargo-pmcp        : 407/407 passed (including 7 new cli_target_flag_tests)
cargo run -p cargo-pmcp -- --target dev auth status : exit 0, renders auth status table, no clap errors
```

Acceptance criteria:

| Criterion | Result |
|---|---|
| `grep -E 'self\.target([^_a-zA-Z]\|$)' deploy/mod.rs \| wc -l` == 0 | PASS (0) |
| `grep -c 'target_type: Option<String>' deploy/mod.rs` == 1 | PASS (1) |
| `grep -c 'alias = "target"' deploy/mod.rs` == 1 | PASS (1) |
| `grep -c 'long = "target-type"' deploy/mod.rs` == 1 | PASS (1) |
| `cargo build -p cargo-pmcp` exit 0 | PASS |
| `cargo test -p cargo-pmcp --lib` exit 0 | PASS |
| `grep -c 'pub target: Option<String>' main.rs` == 1 | PASS (1) |
| `grep -q "Named target from ~/.pmcp/config.toml" main.rs` | PASS |
| `grep -c "fn med3_" main.rs` == 4 | PASS (4) |
| MED-3 matrix tests all pass | PASS (4/4) |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking issue] Dropped `global = true` on the new Cli.target flag**

- **Found during:** Task 2, GREEN phase test run.
- **Issue:** Adding `pub target: Option<String>` with `#[arg(long, global = true)]` on `Cli` triggers clap debug_assert at parse time: `long option names must be unique, but '--target' is in use by both 'target_type' and 'target'`. The deploy subcommand's `target_type` field has `alias = "target"`, which clap registers as a long-option name in the deploy scope; setting `global = true` on the top-level `target` field promotes its `--target` long-name to the same shared namespace, producing the conflict.
- **Fix:** Plan 02's `<action>` block already anticipated this exact failure mode and prescribed the resolution: "If the third test reveals a clap conflict between the global `--target` and the deploy-scoped `target` alias, resolve by removing `global = true` from the new flag." Applied verbatim. The flag remains useful at top level only (parses BEFORE the subcommand), which is exactly what the Plan 06 resolver will need (it reads `cli.target` directly in main.rs before dispatching).
- **Files modified:** `cargo-pmcp/src/main.rs` (1 line: `#[arg(long, global = true)]` → `#[arg(long)]` + a multi-line doc comment explaining the rationale for future maintainers).
- **Commit:** 894db1d5 (rolled into the GREEN commit since the conflict was discovered during the GREEN test run, not separately).

**2. [Rule 1 — Bug] Test fixture used `status` subcommand which requires <OPERATION_ID>**

- **Found during:** Task 2, first GREEN test run (after the clap conflict above was resolved).
- **Issue:** The plan-supplied test bodies invoked `cargo pmcp deploy --target aws-lambda status` and `cargo pmcp --target dev deploy --target-type aws-lambda status`. The deploy subcommand's `status` action requires a positional `<OPERATION_ID>` arg (per `cargo-pmcp/src/commands/deploy/mod.rs` DeployAction::Status); without it clap exits 2.
- **Fix:** Replaced `"status"` → `"outputs"` in 2 test bodies. The `outputs` subcommand has no required positional, so the test parses cleanly and isolates the flag-parse behavior under test.
- **Files modified:** `cargo-pmcp/src/main.rs` (2 line edits in test fixtures).
- **Commit:** 894db1d5 (rolled into GREEN).

## Auth Gates

None — this plan is purely a clap-level refactor; no network or auth interactions.

## Threat Surface

The plan's `<threat_model>` flagged 3 threats (T-77-12, T-77-12-A, T-77-12-B). All mitigated as designed:

| Threat | Mitigation Result |
|---|---|
| T-77-12 (semantic confusion `--target` vs `--target-type`) | Help text on both fields explicitly disambiguates ("named target from `~/.pmcp/config.toml`" vs "deployment target TYPE"). |
| T-77-12-A (legacy script breakage) | `alias = "target"` keeps `cargo pmcp deploy --target aws-lambda` working — verified by `legacy_deploy_target_alias_still_works` test. |
| T-77-12-B (mass `self.target` regex misfire) | Acceptance regex `self\.target([^_a-zA-Z]\|$)` returns 0 matches; manual diff review pre-commit. |

No new threat surface introduced.

## Self-Check: PASSED

Verified all artifacts exist and commits are reachable:

```
$ [ -f cargo-pmcp/src/commands/deploy/mod.rs ] && echo FOUND : FOUND
$ [ -f cargo-pmcp/src/main.rs ] && echo FOUND : FOUND
$ git log --oneline | grep -q 83fbf633 && echo FOUND : FOUND
$ git log --oneline | grep -q e0f0710d && echo FOUND : FOUND
$ git log --oneline | grep -q 894db1d5 && echo FOUND : FOUND
```

## TDD Gate Compliance

- RED gate: commit `e0f0710d test(77-02): add failing tests for global --target named-target flag` ✓
- GREEN gate: commit `894db1d5 feat(77-02): add global --target named-target flag on Cli` ✓
- REFACTOR gate: not needed — implementation was already minimal (single field declaration + doc comment).
