# Phase 97: GitHub Deploy Automation - Research

## Summary

The correct architecture is a two-surface design:

1. `cargo-pmcp` SDK surface: generate and validate GitHub Actions workflows that call the existing deploy engine for every supported target.
2. pmcp.run service surface: offer connected-repository hosting with CodeBuild, server-side trust enforcement, deployment history, logs, previews, and rollbacks.

This split avoids overloading the open-source CLI with hosted-platform responsibilities while still giving non-pmcp.run users a portable automation path.

## Existing SDK Fit

The SDK already has the right anchor points:

- `cargo-pmcp/src/main.rs` owns top-level clap command dispatch.
- `cargo-pmcp/src/commands/deploy/mod.rs` owns `--target-type`, `--manifest-path`, and deploy action dispatch.
- `cargo-pmcp/src/deployment/config.rs` owns `.pmcp/deploy.toml`.
- `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` already supports CI credentials via `PMCP_CLIENT_ID`/`PMCP_CLIENT_SECRET` and `PMCP_ACCESS_TOKEN`.

Therefore, Phase 97 should not add a new deployment config file. It should add a `github` command group and a workflow renderer that calls existing deploy paths.

## Target Matrix

| Target | Build runner | Credential owner | Generated workflow auth |
|--------|--------------|------------------|-------------------------|
| pmcp-run | GitHub Actions for portable path; pmcp.run CodeBuild for connected repo path | pmcp.run deploy trust or fallback PMCP secret | Prefer GitHub OIDC exchange; fallback to `PMCP_CLIENT_ID`/`PMCP_CLIENT_SECRET` or `PMCP_ACCESS_TOKEN` |
| aws-lambda | GitHub Actions | User AWS account | AWS GitHub OIDC role or AWS secrets |
| google-cloud-run | GitHub Actions | User GCP project | Workload Identity Federation or service-account secret |
| azure-container-apps | GitHub Actions | User Azure subscription | Azure federated credential or service-principal secret |
| cloudflare-workers | GitHub Actions | User Cloudflare account | Cloudflare API token secret |

## Security Notes

- Browser/device OAuth is not acceptable in CI.
- GitHub OIDC is preferred for pmcp.run because it avoids long-lived PMCP tokens in GitHub secrets.
- Trust must be enforced server-side. Repository config can express intent but cannot be the authority.
- The pmcp.run service should validate issuer, audience, repository, ref, and optional environment before minting a short-lived deploy credential.
- Non-pmcp.run cloud credentials should remain in the user's GitHub/cloud account and never flow through pmcp.run.

## Validation Architecture

Phase 97 can validate most SDK behavior offline:

- Golden tests for workflow YAML rendering per target.
- Property tests for branch/environment/name sanitization and stable idempotent rendering.
- CLI acceptance tests for `github init --dry-run`, `--output`, and invalid target errors.
- Unit tests for GitHub Actions environment detection and OIDC exchange request construction.
- Fuzz target for renderer inputs if the workflow model accepts user-provided strings that reach YAML.

Network behavior should be isolated behind a small client seam and tested with deterministic mock HTTP responses.

## Implementation Risks

- `deploy` may prompt or assume interactive behavior in some target flows. The generated workflow needs a non-interactive path that fails fast.
- Target naming has a known distinction: global `--target` selects named target config, while deploy `--target-type` selects backend. GitHub command help must not blur those.
- Adding pmcp.run OIDC client code before the service endpoint exists must fail explicitly with a useful message, not silently fall back to interactive auth.
- Workflow generation must be idempotent and avoid overwriting user edits unless `--yes` or `--force` is supplied.

## Recommendation

Implement four SDK plans:

1. Workflow model/renderer and target matrix.
2. CLI command wiring and idempotent file writes.
3. pmcp.run GitHub Actions OIDC credential exchange client seam.
4. Docs, examples, property/fuzz coverage, and deploy-action handoff notes.
