---
phase: 64-secrets-deployment-integration
plan: 01
subsystem: deployment
tags: [dotenvy, secrets, env-vars, cdk, aws-lambda, pmcp-run]

# Dependency graph
requires: []
provides:
  - "SecretResolution struct and resolve_secrets/load_dotenv/print_secret_report functions"
  - "DeployExecutor.extra_env transient env var passthrough to CDK process"
  - "Pre-deploy secret resolution wired into None => branch of deploy command"
affects: [64-02, 64-03]

# Tech tracking
tech-stack:
  added: [dotenvy 0.15]
  patterns: [transient-env-passthrough, pure-resolution-function, target-specific-guidance]

key-files:
  created:
    - cargo-pmcp/src/secrets/resolve.rs
  modified:
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/src/secrets/mod.rs
    - cargo-pmcp/src/commands/deploy/deploy.rs
    - cargo-pmcp/src/commands/deploy/mod.rs
    - cargo-pmcp/src/deployment/targets/aws_lambda/deploy.rs
    - cargo-pmcp/src/deployment/targets/aws_lambda/mod.rs

key-decisions:
  - "Used dotenvy::from_path_iter for .env parsing without process env mutation"
  - "Transient extra_env HashMap on DeployExecutor for CDK secret injection (never persisted to deploy.toml)"
  - "config.secrets used as passthrough vehicle from mod.rs -> AwsLambdaTarget -> deploy_aws_lambda -> DeployExecutor"

patterns-established:
  - "Pure resolution pattern: resolve_secrets takes data in, returns SecretResolution out, no side effects"
  - "Transient env passthrough: DeployExecutor.with_extra_env() -> cmd.env() per entry in run_cdk_deploy"
  - "Target-specific report: print_secret_report branches on target string for aws-lambda vs pmcp-run guidance"

requirements-completed: []

# Metrics
duration: 6min
completed: 2026-03-30
---

# Phase 64 Plan 01: Secret Resolution and Deploy Pipeline Integration Summary

**dotenvy-based secret resolution with transient CDK env var passthrough for AWS Lambda and diagnostic-only guidance for pmcp-run**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-30T00:41:52Z
- **Completed:** 2026-03-30T00:48:00Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Pure secret resolution function that takes Vec<SecretRequirement> + dotenv HashMap and returns found/missing maps
- dotenvy parses .env without mutating process environment (from_path_iter)
- AWS Lambda deploy injects resolved secrets as transient CDK process env vars via DeployExecutor.extra_env (never written to deploy.toml per D-05/D-06)
- pmcp-run deploy shows diagnostic-only guidance with exact `cargo pmcp secret set` commands per D-07
- Missing secrets produce warnings, not deployment-blocking errors per D-04
- 14 unit tests covering all behaviors (11 resolution + 3 deploy executor)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add dotenvy dep and create resolve.rs with secret resolution logic** - `223de5c3` (feat)
2. **Task 2: Wire secret resolution into deploy pipeline with transient CDK env passthrough** - `3ba6a06b` (feat)

## Files Created/Modified
- `cargo-pmcp/Cargo.toml` - Added dotenvy 0.15 dependency
- `cargo-pmcp/src/secrets/resolve.rs` - SecretResolution struct, resolve_secrets(), load_dotenv(), print_secret_report()
- `cargo-pmcp/src/secrets/mod.rs` - Added pub mod resolve and re-exports
- `cargo-pmcp/src/commands/deploy/deploy.rs` - Added extra_env field, with_extra_env() builder, cmd.env() forwarding, unit tests
- `cargo-pmcp/src/commands/deploy/mod.rs` - Wired resolve_secrets/load_dotenv/print_secret_report into None => branch, injects secrets into config.secrets for aws-lambda
- `cargo-pmcp/src/deployment/targets/aws_lambda/deploy.rs` - Updated deploy_aws_lambda to accept extra_env parameter
- `cargo-pmcp/src/deployment/targets/aws_lambda/mod.rs` - AwsLambdaTarget::deploy forwards config.secrets.clone() as extra_env

## Decisions Made
- Used dotenvy::from_path_iter() for .env parsing to avoid mutating process environment (Pitfall 1 from RESEARCH.md)
- Transient extra_env HashMap on DeployExecutor rather than persisting via config.save() (D-05/D-06 compliance)
- config.secrets HashMap used as the passthrough vehicle from mod.rs through AwsLambdaTarget to DeployExecutor (existing but unused field)
- Shell env var precedence over .env (D-13) implemented via checking std::env::var() before dotenv_vars
- env_var field on SecretRequirement used as lookup key when present, falling back to name (Pitfall 4)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all functionality is fully wired.

## Next Phase Readiness
- Secret resolution infrastructure ready for Plan 02 (SDK pmcp::secrets module and cargo pmcp dev .env loading)
- Plan 03 can build documentation on the established patterns

## Self-Check: PASSED

All files exist, all commits verified, all 19 acceptance criteria pass.

---
*Phase: 64-secrets-deployment-integration*
*Completed: 2026-03-30*
