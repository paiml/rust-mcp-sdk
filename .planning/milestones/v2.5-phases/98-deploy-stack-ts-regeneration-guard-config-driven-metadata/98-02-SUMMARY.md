---
phase: 98-deploy-stack-ts-regeneration-guard-config-driven-metadata
plan: 02
subsystem: cargo-pmcp / deployment stack.ts regeneration guard
tags: [deploy, stack-ts, exists-guard, clap-flag, DSTK-01]
requires:
  - "cargo_pmcp::deployment::config::DeployConfig.regenerate_stack (#[serde(skip)] runtime carrier, Plan 98-01)"
  - "render_stack_ts_for_deploy (bin-only renderer, threads IAM today)"
provides:
  - "cargo_pmcp::deployment::config::write_stack_ts_guarded (pub(crate) exists-guard helper, returns did-write bool)"
  - "cargo_pmcp::deployment::config::STACK_TS_PRESERVED_NOTICE (pub(crate) const)"
  - "DeployExecutor::with_regenerate_stack (aws-lambda flag carrier across the execute() disk reload)"
  - "DeployCommand --regenerate-stack flag (alias --force) → config.regenerate_stack"
affects:
  - "Plan 98-03 (DSTK-02/03 metadata→render) — unrelated render plumbing; shares config.rs"
  - "Plan 98-04 (docs + CLI acceptance) — decides on a lib-public guard entry point to flip Test A live; documents --regenerate-stack/--force"
tech-stack:
  added: []
  patterns:
    - "exists-guard helper returns Ok(bool) did-write so callers own the preserved-notice (decouples write from validation)"
    - "runtime flag re-applied onto disk-reloaded config carrier (#[serde(skip)] survives the CLI→config→target hop, not a new trait param)"
    - "executor builder field (with_regenerate_stack) mirrors with_extra_env to carry the flag past execute()'s DeployConfig::load reload"
    - "#[allow(dead_code)] + // Why: for pub(crate) items that are bin-only consumers but also mounted into the lib via #[path]"
key-files:
  created: []
  modified:
    - cargo-pmcp/src/deployment/config.rs
    - cargo-pmcp/src/commands/deploy/mod.rs
    - cargo-pmcp/src/commands/deploy/deploy.rs
    - cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs
    - cargo-pmcp/src/deployment/targets/aws_lambda/deploy.rs
    - cargo-pmcp/tests/deploy_stack_ts_guard.rs
decisions:
  - "Factored a single pub(crate) write_stack_ts_guarded helper used by both deploy targets; it returns whether it wrote so the caller (not the helper) prints the identical STACK_TS_PRESERVED_NOTICE — keeps IAM validation fully decoupled from the write."
  - "aws-lambda required threading the flag onto DeployExecutor (with_regenerate_stack) because DeployExecutor::execute() RE-LOADS DeployConfig from disk, which drops the #[serde(skip)] config.regenerate_stack set in the CLI arm. Re-applied in execute() before the guarded write. (Rule 3 blocking-issue fix.)"
  - "Test A in the external tests/ crate stays #[ignore]: the guard entry points (write_stack_ts_guarded, validate_and_regenerate_stack_ts, regenerate_stack_ts) are all pub(crate) in the bin-only tree the lib does not re-export, so an integration test cannot reach them. DSTK-01 is instead proven by 6 in-crate unit tests on both targets + the helper. Handed to 98-04 to decide on a lib-public surface."
metrics:
  duration: ~25min
  completed: 2026-06-16
---

# Phase 98 Plan 02: stack.ts Regeneration Guard — Exists-Guard + `--regenerate-stack` Flag Summary

Closed DSTK-01: `cargo pmcp deploy` no longer silently overwrites an operator-curated `deploy/lib/stack.ts` on EITHER deploy target. A shared `Path::exists()` guard skips the write (preserving the file byte-for-byte) unless `--regenerate-stack`/`--force` is passed, prints a one-line preserved-notice, and keeps IAM validation running on the skip path. First-deploy scaffolding of a missing file still works flag-free.

## What Was Built

**Task 1 — exists-guard on both write sites (commit `d184e2cf`)**
- New `pub(crate) fn write_stack_ts_guarded(lib_dir, stack_ts, regenerate) -> Result<bool>` in `deployment/config.rs`: `create_dir_all` always; returns `Ok(false)` WITHOUT writing when `path.exists() && !regenerate`; otherwise writes and returns `Ok(true)`. The bool = "did we write" so callers own the notice.
- New `pub(crate) const STACK_TS_PRESERVED_NOTICE` = `"preserved existing deploy/lib/stack.ts (pass --regenerate-stack to overwrite)"` — both targets emit the identical string.
- `pmcp_run::deploy::validate_and_regenerate_stack_ts`: kept the existing `iam::validate + emit_warnings` calls untouched (validation BEFORE the guard); replaced the unconditional `fs::write` with the guarded helper; prints the notice on skip.
- `DeployExecutor::regenerate_stack_ts` (aws-lambda): same guarded-helper + notice swap; IAM validation already runs earlier in `execute()`.
- Threaded the flag through aws-lambda: added a `regenerate_stack` field + `with_regenerate_stack` builder to `DeployExecutor`; `execute()` re-applies it onto the disk-reloaded config; `deploy_aws_lambda` passes `config.regenerate_stack` into the executor.
- 4 in-crate unit tests for the helper: absent→write, exists+no-flag→preserve, exists+flag→overwrite, plus a notice-text assertion.

**Task 2 — clap flag wiring + per-target guard tests (commit `e38a753f`)**
- `DeployCommand`: added `#[arg(long = "regenerate-stack", alias = "force")] regenerate_stack: bool` with a doc comment explaining it overwrites an existing `deploy/lib/stack.ts`.
- No-subcommand deploy arm: assign `config.regenerate_stack = self.regenerate_stack;` right after `DeployConfig::load`, before `target.build`/`target.deploy` (mirrors the `config.secrets` injection — flag travels via the config carrier, not a new trait parameter).
- Per-target exists-guard unit tests: `pmcp_run_{preserves,overwrites}_existing_stack_ts_*` and `aws_lambda_{preserves,overwrites}_existing_stack_ts_*` (pre-write a curated sentinel, assert byte-identity on the no-flag path and overwrite on the flag path), plus `with_regenerate_stack_builder`.
- Updated Test A in `tests/deploy_stack_ts_guard.rs` documenting DSTK-01 as satisfied by the in-crate unit tests, with an updated `#[ignore]` reason and 98-04 handoff.

## Verification

- `cargo test -p cargo-pmcp stack_ts` — 23 passed, 2 ignored (Test A + Test C, by design).
- `cargo test -p cargo-pmcp preserves_existing_stack` / `overwrites_existing_stack` — both report 4 passed (helper + pmcp-run + aws-lambda each contribute a case).
- `cargo test -p cargo-pmcp --test deploy_stack_ts_guard` — 2 passed, 2 ignored.
- `cargo build -p cargo-pmcp` — 0 `error[` (the 15 bin warnings are all pre-existing, unrelated files).
- `cargo build -p cargo-pmcp --lib` — clean (the bin-only `write_stack_ts_guarded`/notice carry a `// Why:`-annotated `#[allow(dead_code)]` for the lib-mounted `config.rs` view).
- `cargo run --bin cargo-pmcp -- deploy --help` — shows `--regenerate-stack` with its doc; `deploy --force …` is accepted by clap (reaches manifest resolution, not an unknown-arg error).
- `cargo fmt --all -- --check` — clean. Pre-commit quality-gate hook ran on BOTH commits (no `--no-verify`).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — blocking issue] aws-lambda flag dropped by DeployExecutor's disk reload of DeployConfig.**
- **Found during:** Task 1.
- **Issue:** `DeployExecutor::execute()` calls `DeployConfig::load(&self.project_root)` to build a FRESH config, so the `config.regenerate_stack` set by the CLI arm (a `#[serde(skip)]` field) never reaches `regenerate_stack_ts` on the aws-lambda path — the flag would silently no-op there.
- **Fix:** Added a `regenerate_stack` field + `with_regenerate_stack` builder to `DeployExecutor` (mirrors `with_extra_env`); `deploy_aws_lambda` passes `config.regenerate_stack` in; `execute()` re-applies it onto the reloaded config before the guarded write.
- **Files modified:** `cargo-pmcp/src/commands/deploy/deploy.rs`, `cargo-pmcp/src/deployment/targets/aws_lambda/deploy.rs`.
- **Commit:** `d184e2cf`.

**2. [Rule 3 — blocking issue] `#[allow(dead_code)]` on the bin-only helper + const for the lib build.**
- **Found during:** Task 1.
- **Issue:** `config.rs` is mounted into the lib via `#[path]` (see `lib.rs`). The guard's only production callers (`commands::*`, `deployment::targets::*`) are bin-only, so the non-test lib build reports `write_stack_ts_guarded` and `STACK_TS_PRESERVED_NOTICE` as dead code — a warning under the zero-warning gate.
- **Fix:** Added `// Why:`-annotated `#[allow(dead_code)]` to both, documenting the lib-vs-bin boundary; both are exercised by the in-crate `stack_ts_guard_tests` and used by the bin.
- **Files modified:** `cargo-pmcp/src/deployment/config.rs`.
- **Commit:** `d184e2cf`.

**3. [Plan-sanctioned fallback] Test A left `#[ignore]` instead of un-ignored.**
- **Found during:** Task 2.
- **Issue:** The plan's Task 2 `<action>` allows un-ignoring Test A only "if it is now reachable+green; if it remains integration-only and not reachable, leave the in-crate unit tests as the DSTK-01 proof and note the handoff to 98-04." All guard entry points are `pub(crate)` in the bin-only tree the lib does not re-export, so the external `tests/` crate genuinely cannot reach them.
- **Fix:** Kept Test A `#[ignore]` with an updated reason documenting DSTK-01 as satisfied by 6 in-crate unit tests; explicit 98-04 handoff for a lib-public surface decision. This is the plan's stated fallback, not a gap.
- **Files modified:** `cargo-pmcp/tests/deploy_stack_ts_guard.rs`.
- **Commit:** `e38a753f`.

## Deferred Issues

- Pre-existing lib proptest failure `test_support_cache::proptests::normalize_round_trip_idempotent` (`commands/auth_cmd/cache.rs:419`) — reproduced on clean HEAD with this plan's changes set aside; unrelated to the deploy/stack.ts guard. Logged to `deferred-items.md`, left untouched per SCOPE BOUNDARY.

## Known Stubs

- `render_stack_ts_with_metadata` in `tests/deploy_stack_ts_guard.rs` remains a `String::new()` placeholder for the still-`#[ignore]`d Test C (Plan 98-03's responsibility) — unchanged by this plan, not a product-path stub.

## Threat Flags

None — no new network endpoints, auth paths, or schema changes. The change tightens an existing file-write trust boundary (T-98-03/T-98-04 in the plan's threat register: the unconditional `fs::write` now requires explicit `--regenerate-stack` opt-in, with an operator-visible preserved-notice). IAM validation (T-98-05) is kept outside the guard on both targets.

## Self-Check: PASSED

- FOUND: `cargo-pmcp/src/deployment/config.rs` (write_stack_ts_guarded + STACK_TS_PRESERVED_NOTICE)
- FOUND: `cargo-pmcp/src/commands/deploy/mod.rs` (--regenerate-stack flag + config assignment)
- FOUND: `cargo-pmcp/src/commands/deploy/deploy.rs` (with_regenerate_stack + guarded write + aws-lambda tests)
- FOUND: `cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs` (guarded write + pmcp-run tests)
- FOUND: `cargo-pmcp/src/deployment/targets/aws_lambda/deploy.rs` (flag threaded into executor)
- FOUND: `cargo-pmcp/tests/deploy_stack_ts_guard.rs` (Test A updated)
- FOUND commit `d184e2cf` (Task 1)
- FOUND commit `e38a753f` (Task 2)

## Handoff

- **Plan 98-03 (DSTK-02/DSTK-03):** thread `DeployConfig.metadata` through `render_stack_ts_for_deploy` → `McpMetadata` → `to_cdk_context` → template; un-ignore Test C. The guard from this plan is render-agnostic — it writes whatever the renderer produces, so a regeneration with `--regenerate-stack` will safely carry 98-03's config-driven metadata.
- **Plan 98-04 (docs + CLI acceptance):** document `--regenerate-stack`/`--force` and the `[metadata]` block in `cargo-pmcp/docs/commands/deploy.md`; decide whether to expose a lib-public guard entry point (e.g. re-export `write_stack_ts_guarded`) to flip the external Test A from `#[ignore]` to a live black-box assertion.
