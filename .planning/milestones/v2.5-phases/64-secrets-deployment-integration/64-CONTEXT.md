# Phase 64: secrets-deployment-integration - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire `cargo pmcp secret` into deployment targets so secrets are injected as environment variables at deploy time. Five workstreams: (1) AWS Lambda env var injection, (2) pmcp.run secret requirement reporting, (3) SDK `pmcp::secrets` thin reader, (4) local dev `.env` loading, (5) documentation updates.

</domain>

<decisions>
## Implementation Decisions

### Secret resolution strategy
- **D-01:** Secrets are resolved from **local environment variables and `.env` files only** — no direct Secret Manager integration at deploy time. Secret Manager access is a premium pmcp.run service feature.
- **D-02:** The deploy command reads `SecretRequirement` definitions from the server's configuration file (`.pmcp/config.toml` or server metadata), then searches for matching values in `std::env` and `.env` files in the project root.
- **D-03:** The deploy command reports which secrets were found and which are missing before proceeding.

### AWS Lambda target behavior
- **D-04:** For `cargo pmcp deploy --target aws-lambda`, found secrets are injected as Lambda environment variables in the CDK context. Missing secrets produce a warning but do not block deployment.
- **D-05:** Secret values come from the developer's local env vars or `.env` file — they are baked into the Lambda configuration at deploy time.

### pmcp.run target behavior
- **D-06:** For `cargo pmcp deploy --target pmcp-run`, secrets are **never** sent from the local machine to pmcp.run. The CLI only performs a diagnostic check.
- **D-07:** For missing secrets, the CLI shows the exact `cargo pmcp secret set --server <id> <SECRET_NAME> --target pmcp --prompt` command the user should run to store each secret in pmcp.run's managed Secrets Manager.
- **D-08:** The actual env var injection for pmcp.run happens server-side — pmcp.run reads from its org-level Secrets Manager and sets env vars on the Lambda/container.

### SDK `pmcp::secrets` module
- **D-09:** Start with a thin env-var reader: `pmcp::secrets::get(name) -> Option<String>` and `pmcp::secrets::require(name) -> Result<String, SecretError>`.
- **D-10:** `require()` error message includes actionable guidance: `"Missing secret FOO. Set with: cargo pmcp secret set <server>/FOO --prompt"`.
- **D-11:** No compile-time macro or startup validation in v1 — gather developer feedback first before adding complexity.

### Local dev secret injection
- **D-12:** `cargo pmcp dev` loads `.env` file from the project root and sets values as env vars for the child server process. Standard `.env` format (KEY=VALUE, # comments, no export prefix).
- **D-13:** If both `.env` file and shell env var define the same key, shell env var wins (existing environment takes precedence).

### Documentation
- **D-14:** Update cargo-pmcp README with secret + deploy integration workflow.
- **D-15:** Update secret command help text with deployment-aware examples.
- **D-16:** Add SDK-level rustdoc with examples for `pmcp::secrets` module.
- **D-17:** Fold the pending todo "Create README docs for cargo-pmcp CLI" into this phase's documentation workstream.

### Claude's Discretion
- `.env` parsing implementation (hand-rolled vs `dotenvy` crate)
- Exact warning/info message formatting for missing vs found secrets
- Whether to add `--dry-run` flag to show what secrets would be injected without deploying
- Test strategy for secret resolution logic

### Folded Todos
- **Create README docs for cargo-pmcp CLI** (area: docs, score: 0.9) — folded into D-14/D-15 documentation workstream

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Secret management
- `cargo-pmcp/src/secrets/mod.rs` — Module root, naming conventions, SecretTarget enum
- `cargo-pmcp/src/secrets/provider.rs` — SecretProvider trait definition
- `cargo-pmcp/src/secrets/providers/local.rs` — Local filesystem provider (`.pmcp/secrets/` storage)
- `cargo-pmcp/src/secrets/providers/pmcp_run.rs` — pmcp.run GraphQL provider with serverId mutations
- `cargo-pmcp/src/secrets/config.rs` — SecretsConfig, target auto-detection
- `cargo-pmcp/src/secrets/value.rs` — SecretValue wrapper with zeroization
- `cargo-pmcp/src/commands/secret/mod.rs` — CLI command handler

### Deployment integration
- `cargo-pmcp/src/deployment/trait.rs` — DeploymentTarget trait with `secrets()` method, SecretsAction enum
- `cargo-pmcp/src/deployment/config.rs` — DeployConfig with `secrets` and `environment` HashMaps
- `cargo-pmcp/src/deployment/metadata.rs` — SecretRequirement struct, ResourceRequirements extraction
- `cargo-pmcp/src/deployment/targets/aws_lambda/mod.rs` — AWS Lambda target (secrets stubbed)
- `cargo-pmcp/src/deployment/targets/pmcp_run/mod.rs` — pmcp.run target (secrets returns "Phase 2!")
- `cargo-pmcp/src/deployment/builder.rs` — Build pipeline with env var injection for cross-compilation

### SDK
- `src/lib.rs` — Main pmcp crate module exports (no secrets module exists yet)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `SecretRequirement` struct in `deployment/metadata.rs` — already captures name, description, required, env_var, obtain_url
- `DeployConfig.secrets` HashMap — exists but unused, ready for secret injection
- `DeployConfig.environment` HashMap — existing pattern for env var injection into deploys
- `SecretProvider` trait — existing trait for multi-backend secret resolution
- `SecretsAction` enum — existing action types for secret CRUD

### Established Patterns
- `DeploymentTarget::secrets()` trait method — async, takes `DeployConfig` + `SecretsAction`, returns `Result<()>`
- Local provider stores secrets at `.pmcp/secrets/{server-id}/{SECRET_NAME}` with 0600 permissions
- pmcp.run GraphQL mutations use `serverId` parameter for namespacing
- GlobalFlags `should_output()` guard pattern for CLI output

### Integration Points
- `cargo pmcp deploy` pipeline in `deployment/mod.rs` — where secret resolution hooks in before target-specific deploy
- `cargo pmcp dev` command — where `.env` loading hooks in before spawning child process
- `src/lib.rs` — where `pub mod secrets` would be added to the SDK
- CDK context injection in `deployment/builder.rs` — where Lambda env vars are set

</code_context>

<specifics>
## Specific Ideas

- Secret resolution is a pre-deploy step: scan config → search local env + `.env` → report found/missing → proceed (or guide user to `cargo pmcp secret set`)
- pmcp.run is a "show, don't send" model — CLI never transmits secret values to pmcp.run during deploy, only diagnostics
- SDK helper is intentionally thin — `std::env::var()` with better error messages, not a framework

</specifics>

<deferred>
## Deferred Ideas

- Secret Manager integration at deploy time (AWS SM ARN references, Lambda Secrets Extension) — premium pmcp.run feature, not CLI-level
- Compile-time `declare!` macro for secret registration and startup validation — pending developer feedback
- Secret rotation / versioning in CLI — pmcp.run backend feature
- `.env.production` / `.env.staging` environment-specific files — future enhancement based on usage patterns

</deferred>

---

*Phase: 64-secrets-deployment-integration*
*Context gathered: 2026-03-29*
