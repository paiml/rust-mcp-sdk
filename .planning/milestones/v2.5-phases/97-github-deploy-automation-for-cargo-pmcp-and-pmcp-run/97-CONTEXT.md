# Phase 97: GitHub Deploy Automation for cargo-pmcp and pmcp.run - Context

**Gathered:** 2026-06-12
**Status:** Ready for implementation planning
**Source:** User design discussion + local cargo-pmcp code inspection

<domain>
## Phase Boundary

Phase 97 adds GitHub-native deployment automation to the Rust SDK while preserving the product boundary between the open-source CLI and the pmcp.run hosted service.

The SDK owns portable GitHub Actions scaffolding:
- `cargo pmcp github init --target-type <target>`
- workflow generation for `pmcp-run`, `aws-lambda`, `google-cloud-run`, `azure-container-apps`, and `cloudflare-workers`
- idempotent writes under `.github/workflows/`
- validation/dry-run behavior
- documentation for target-specific secrets/OIDC requirements

The pmcp.run service owns managed connected-repository hosting:
- GitHub App or equivalent repository connection
- repo/branch/path selection
- CodeBuild-backed build and deploy
- service-owned logs, deployment history, rollbacks, and preview deployments
- OIDC trust enforcement and short-lived deploy credential minting

This Rust SDK phase implements the SDK-side contract and client hooks only. pmcp.run backend implementation is a companion service phase in the pmcp.run repository.
</domain>

<decisions>
## Implementation Decisions

### Product Split
- `cargo-pmcp` remains the universal build/deploy tool and GitHub Actions workflow scaffolder.
- pmcp.run becomes the managed deployment platform for connected GitHub repositories.
- Do not choose one surface exclusively; both are required but have different responsibilities.

### Config Model
- Reuse `.pmcp/deploy.toml`; do not introduce a new `pmcp.toml` for deployment automation in this phase.
- Reuse existing `cargo pmcp deploy` behavior inside workflows.
- Keep target selection aligned with `--target-type` for deployment backend selection and the existing named-target resolver for environment defaults.

### Authentication
- For pmcp.run GitHub Actions, prefer GitHub OIDC with `permissions: id-token: write`.
- Avoid browser/device OAuth in CI.
- Support existing CI fallback credentials already present in `pmcp_run::auth`: `PMCP_CLIENT_ID` + `PMCP_CLIENT_SECRET` and `PMCP_ACCESS_TOKEN`.
- Add a future-proof SDK client seam for exchanging a GitHub OIDC token for a short-lived pmcp.run deploy credential.

### Non-pmcp.run Targets
- The generated GitHub Actions workflows for AWS/GCP/Azure/Cloudflare run in the user's GitHub Actions account and deploy directly to the user's cloud account.
- pmcp.run must not proxy or hold third-party cloud provider credentials for those portable workflows.

### pmcp.run Service
- Managed connected-repo deploys should be a service feature similar to AWS Amplify/Vercel: connect repo, pick branch/path, build on push.
- pmcp.run can use existing AWS CodeBuild and M2M Cognito patterns to build configuration-only MCP servers and Rust projects.
- Preview deployments are a pmcp.run service feature first, not a generic CLI feature.
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### CLI Dispatch
- `cargo-pmcp/src/main.rs` - top-level clap command registration and target-consuming dispatch
- `cargo-pmcp/src/commands/mod.rs` - command module exports and `GlobalFlags`
- `cargo-pmcp/src/commands/deploy/mod.rs` - deploy command shape, target-type flag, manifest-path flag, async dispatch

### Deployment Config and Targets
- `cargo-pmcp/src/deployment/config.rs` - `.pmcp/deploy.toml` schema and target/server config
- `cargo-pmcp/src/deployment/targets/mod.rs` - deployment target registry entry point
- `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` - existing pmcp.run local and CI credential resolution
- `cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs` - pmcp.run API client pattern
- `cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs` - existing pmcp.run deploy flow

### Existing Docs and Tests
- `cargo-pmcp/docs/commands/deploy.md` - current deploy command docs
- `cargo-pmcp/tests/cli_acceptance.rs` - CLI-level acceptance-test patterns
- `cargo-pmcp/tests/deploy_config_only.rs` - config-only deploy test patterns
- `cargo-pmcp/fuzz/fuzz_targets/pmcp_config_toml_parser.rs` - existing fuzz target style for config parsing
</canonical_refs>

<specifics>
## Specific Ideas

- Add a top-level `github` command group rather than nesting under `deploy`, because GitHub is the automation source, not the deployment target.
- Minimum command surface:
  - `cargo pmcp github init --target-type pmcp-run --branch main --environment production`
  - `cargo pmcp github init --target-type aws-lambda --branch main`
  - `cargo pmcp github init --target-type google-cloud-run --dry-run`
  - `cargo pmcp github workflow validate`
- Generated workflow should be stable, small, and comment-light:
  - checkout
  - install Rust stable
  - install `cargo-pmcp`
  - run `cargo pmcp deploy --target-type <target> --non-interactive`
- Add `--non-interactive` or equivalent deploy behavior if current deploy flows can prompt in CI.
- For pmcp.run OIDC, generated workflow should include:
  - `permissions: contents: read, id-token: write`
  - `PMCP_GITHUB_OIDC_AUDIENCE=pmcp.deploy`
  - no long-lived PMCP secret by default
- For fallback mode, docs may mention `PMCP_CLIENT_ID`/`PMCP_CLIENT_SECRET` and `PMCP_ACCESS_TOKEN`.
</specifics>

<deferred>
## Deferred Ideas

- pmcp.run GitHub App implementation and webhook handling live in the pmcp.run service repository.
- Cross-target preview deployments are deferred; pmcp.run previews are the first preview path.
- Publishing a separate `pmcp/deploy-action@v1` is planned as a follow-up once generated workflow semantics stabilize.
- GitLab/Bitbucket support is deferred until the GitHub model is proven.
</deferred>

---

*Phase: 97-github-deploy-automation-for-cargo-pmcp-and-pmcp-run*
*Context gathered: 2026-06-12*
