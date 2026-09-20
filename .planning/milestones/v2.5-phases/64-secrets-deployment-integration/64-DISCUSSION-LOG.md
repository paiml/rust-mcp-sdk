# Phase 64: secrets-deployment-integration - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-29
**Phase:** 64-secrets-deployment-integration
**Areas discussed:** Secret resolution strategy, pmcp.run handshake, SDK module scope, Local dev injection, Documentation scope

---

## Secret Resolution at Deploy Time

| Option | Description | Selected |
|--------|-------------|----------|
| Local env + .env only | Resolve from local env vars and .env files, no Secret Manager | ✓ |
| AWS Secrets Manager ARN | Reference SM ARNs, use Lambda Secrets Extension | |
| Full provider resolution | Resolve from any configured provider (local/pmcp.run/AWS SM) | |

**User's choice:** Local env + .env only
**Notes:** Secret Manager access is a premium pmcp.run service feature. Deploy reads SecretRequirement from config, searches local env and .env, reports found/missing.

---

## pmcp.run Secret-to-Deploy Handshake

| Option | Description | Selected |
|--------|-------------|----------|
| Show-don't-send diagnostic | CLI checks locally, shows `cargo pmcp secret set` commands for missing | ✓ |
| Send requirements to backend | CLI sends SecretRequirement list so backend discovers what to inject | |
| Full secret sync | CLI sends secret values to pmcp.run | |

**User's choice:** Show-don't-send diagnostic
**Notes:** CLI never transmits secret values to pmcp.run during deploy. For missing secrets, shows exact `cargo pmcp secret set --server <id> <NAME> --target pmcp --prompt` command. Actual injection happens server-side.

---

## SDK `pmcp::secrets` Module Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Thin env-var reader | get/require with helpful error messages | ✓ |
| Reader + declare macro | Thin reader plus compile-time secret registration | |
| Full secrets framework | Multi-backend resolution, caching, rotation | |

**User's choice:** Thin env-var reader
**Notes:** Start minimal, gather developer feedback before adding complexity.

---

## Local Dev Secret Injection

| Option | Description | Selected |
|--------|-------------|----------|
| .env file loading | Standard .env format, shell env takes precedence | ✓ |
| .pmcp/secrets/ filesystem | Read from local provider's secret storage | |
| Both with priority | .env first, then .pmcp/secrets/ fallback | |

**User's choice:** .env file loading
**Notes:** Standard developer workflow. Shell env vars take precedence over .env values.

---

## Documentation Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Full update + fold todo | Update README, help text, rustdoc, fold "Create README docs" todo | ✓ |
| Minimal inline docs | Just rustdoc on new code | |

**User's choice:** Full update + fold todo
**Notes:** Documentation is critical for developer success with the SDK.

---

## Claude's Discretion

- .env parsing implementation (hand-rolled vs dotenvy crate)
- Warning/info message formatting
- Whether to add --dry-run flag
- Test strategy

## Deferred Ideas

- Secret Manager integration at deploy time — premium feature
- Compile-time declare! macro — pending feedback
- Secret rotation/versioning — backend feature
- Environment-specific .env files — future enhancement
