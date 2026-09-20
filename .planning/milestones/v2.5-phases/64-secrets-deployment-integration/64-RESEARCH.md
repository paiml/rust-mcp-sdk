# Phase 64: secrets-deployment-integration - Research

**Researched:** 2026-03-29
**Domain:** CLI deployment pipeline, SDK runtime helpers, .env parsing
**Confidence:** HIGH

## Summary

This phase wires existing `cargo pmcp secret` infrastructure into the deployment pipeline and adds a thin SDK reader module. The codebase already has comprehensive building blocks: `SecretRequirement` in `deployment/metadata.rs`, `DeployConfig.secrets` and `DeployConfig.environment` HashMaps in `deployment/config.rs`, `SecretProvider` trait with local and pmcp.run implementations, and stub `secrets()` methods on both `AwsLambdaTarget` and `PmcpRunTarget` (currently printing "Phase 2!"). The `dev.rs` command spawns a child process via `Command` with `.env()` calls, providing the exact hook point for .env injection. The SDK's `src/assets/` module establishes the pattern for the new `src/secrets/` module: free functions backed by a global `OnceLock` loader.

The five workstreams decompose cleanly. (1) A pre-deploy "secret resolution" step that scans `SecretRequirement` definitions, searches `std::env` and a `.env` file, reports found/missing, and populates `DeployConfig.environment` for AWS Lambda CDK injection. (2) pmcp.run diagnostic-only flow that never sends secrets -- only shows guidance commands. (3) A thin `pmcp::secrets` SDK module (~60 lines) with `get`/`require` helpers. (4) `.env` loading for `cargo pmcp dev` before child process spawn. (5) Documentation updates across cargo-pmcp README, CLI help text, and SDK rustdoc.

**Primary recommendation:** Use `dotenvy` crate (v0.15.7) for `.env` parsing rather than hand-rolling -- it handles edge cases (quoted values, multiline, `export` prefix stripping, comment parsing) that a hand-rolled parser would miss. The resolution logic is a standalone function taking `Vec<SecretRequirement>` and returning found/missing maps, testable without filesystem or deployment target dependencies.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Secrets are resolved from **local environment variables and `.env` files only** -- no direct Secret Manager integration at deploy time. Secret Manager access is a premium pmcp.run service feature.
- **D-02:** The deploy command reads `SecretRequirement` definitions from the server's configuration file (`.pmcp/config.toml` or server metadata), then searches for matching values in `std::env` and `.env` files in the project root.
- **D-03:** The deploy command reports which secrets were found and which are missing before proceeding.
- **D-04:** For `cargo pmcp deploy --target aws-lambda`, found secrets are injected as Lambda environment variables in the CDK context. Missing secrets produce a warning but do not block deployment.
- **D-05:** Secret values come from the developer's local env vars or `.env` file -- they are baked into the Lambda configuration at deploy time.
- **D-06:** For `cargo pmcp deploy --target pmcp-run`, secrets are **never** sent from the local machine to pmcp.run. The CLI only performs a diagnostic check.
- **D-07:** For missing secrets, the CLI shows the exact `cargo pmcp secret set --server <id> <SECRET_NAME> --target pmcp --prompt` command the user should run to store each secret in pmcp.run's managed Secrets Manager.
- **D-08:** The actual env var injection for pmcp.run happens server-side -- pmcp.run reads from its org-level Secrets Manager and sets env vars on the Lambda/container.
- **D-09:** Start with a thin env-var reader: `pmcp::secrets::get(name) -> Option<String>` and `pmcp::secrets::require(name) -> Result<String, SecretError>`.
- **D-10:** `require()` error message includes actionable guidance: `"Missing secret FOO. Set with: cargo pmcp secret set <server>/FOO --prompt"`.
- **D-11:** No compile-time macro or startup validation in v1 -- gather developer feedback first before adding complexity.
- **D-12:** `cargo pmcp dev` loads `.env` file from the project root and sets values as env vars for the child server process. Standard `.env` format (KEY=VALUE, # comments, no export prefix).
- **D-13:** If both `.env` file and shell env var define the same key, shell env var wins (existing environment takes precedence).
- **D-14:** Update cargo-pmcp README with secret + deploy integration workflow.
- **D-15:** Update secret command help text with deployment-aware examples.
- **D-16:** Add SDK-level rustdoc with examples for `pmcp::secrets` module.
- **D-17:** Fold the pending todo "Create README docs for cargo-pmcp CLI" into this phase's documentation workstream.

### Claude's Discretion
- `.env` parsing implementation (hand-rolled vs `dotenvy` crate)
- Exact warning/info message formatting for missing vs found secrets
- Whether to add `--dry-run` flag to show what secrets would be injected without deploying
- Test strategy for secret resolution logic

### Deferred Ideas (OUT OF SCOPE)
- Secret Manager integration at deploy time (AWS SM ARN references, Lambda Secrets Extension) -- premium pmcp.run feature, not CLI-level
- Compile-time `declare!` macro for secret registration and startup validation -- pending developer feedback
- Secret rotation / versioning in CLI -- pmcp.run backend feature
- `.env.production` / `.env.staging` environment-specific files -- future enhancement based on usage patterns
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| dotenvy | 0.15.7 | `.env` file parsing | Well-maintained dotenv fork, recommended over unmaintained `dotenv`. Handles edge cases: quoted values, multiline, `export` prefix, `#` comments. Used by Axum, Rocket, many Rust web projects. |
| thiserror | 2 | SDK SecretError type | Already in pmcp crate deps, project convention for error types |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| (none new for cargo-pmcp) | - | - | All .env parsing and resolution logic can use dotenvy + existing deps |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| dotenvy | Hand-rolled parser | Saves ~10KB dependency but misses edge cases (multiline values, `\n` escapes, BOM handling, Windows line endings). Recommendation: use dotenvy. |
| dotenvy | dotenv (original) | Unmaintained since 2020, RUSTSEC-2021-0141 advisory. Do not use. |

**Installation:**

For cargo-pmcp (deployment CLI):
```bash
# In cargo-pmcp/Cargo.toml [dependencies]
dotenvy = "0.15"
```

For pmcp SDK (runtime helpers):
```bash
# No new dependencies -- pmcp::secrets just reads std::env::var()
```

**Version verification:** `dotenvy` 0.15.7 confirmed on crates.io as of 2026-03-29.

## Architecture Patterns

### Recommended Project Structure
```
cargo-pmcp/src/
├── secrets/
│   ├── mod.rs              # existing - add resolve module re-export
│   ├── resolve.rs          # NEW: SecretResolution logic (pre-deploy step)
│   ├── provider.rs         # existing
│   ├── providers/          # existing
│   ├── config.rs           # existing
│   └── ...
├── commands/
│   ├── deploy/
│   │   ├── mod.rs          # MODIFY: add secret resolution before deploy()
│   │   └── deploy.rs       # MODIFY: inject resolved secrets into CDK env
│   └── dev.rs              # MODIFY: add .env loading before child spawn
├── deployment/
│   ├── config.rs           # existing - DeployConfig.environment used for injection
│   └── metadata.rs         # existing - SecretRequirement used for scanning

src/
├── secrets/
│   ├── mod.rs              # NEW: pmcp::secrets module
│   └── reader.rs           # NEW: get/require helpers
└── lib.rs                  # MODIFY: add pub mod secrets
```

### Pattern 1: Secret Resolution as a Pre-Deploy Step
**What:** A pure function that takes `Vec<SecretRequirement>` + env vars + .env file and returns `SecretResolution { found: HashMap<String, String>, missing: Vec<SecretRequirement> }`.
**When to use:** Called once before any target-specific deploy logic. Both AWS Lambda and pmcp.run use the same resolution step, then diverge on what they do with the result.
**Example:**
```rust
// cargo-pmcp/src/secrets/resolve.rs
pub struct SecretResolution {
    /// Resolved secrets: env_var_name -> value
    pub found: HashMap<String, String>,
    /// Requirements that could not be resolved
    pub missing: Vec<SecretRequirement>,
}

/// Resolve secrets from environment variables and .env file.
///
/// Priority: std::env > .env file (per D-13).
/// Does NOT modify process environment -- returns data only.
pub fn resolve_secrets(
    requirements: &[SecretRequirement],
    dotenv_vars: &HashMap<String, String>,
) -> SecretResolution {
    let mut found = HashMap::new();
    let mut missing = Vec::new();

    for req in requirements {
        let env_name = req.env_var.as_deref()
            .unwrap_or(&req.name);

        // std::env takes precedence over .env file (D-13)
        if let Ok(value) = std::env::var(env_name) {
            found.insert(env_name.to_string(), value);
        } else if let Some(value) = dotenv_vars.get(env_name) {
            found.insert(env_name.to_string(), value.clone());
        } else {
            missing.push(req.clone());
        }
    }

    SecretResolution { found, missing }
}
```

### Pattern 2: .env Loading with dotenvy (No Process Mutation)
**What:** Use `dotenvy::EnvLoader` to parse `.env` without modifying the current process environment. Return a `HashMap` for explicit injection.
**When to use:** Both `cargo pmcp deploy` and `cargo pmcp dev` load .env, but use the data differently.
**Example:**
```rust
// Parse .env file into a HashMap without modifying std::env
pub fn load_dotenv_file(project_root: &Path) -> HashMap<String, String> {
    let env_path = project_root.join(".env");
    if !env_path.exists() {
        return HashMap::new();
    }

    // dotenvy::EnvLoader can read without modifying process env
    dotenvy::from_path_iter(&env_path)
        .map(|iter| {
            iter.filter_map(|item| item.ok())
                .collect()
        })
        .unwrap_or_default()
}
```

### Pattern 3: SDK Thin Reader (Mirrors assets Module)
**What:** Free functions in `pmcp::secrets` backed by `std::env::var()` with actionable error messages.
**When to use:** Server-side code at runtime to read secrets injected as env vars.
**Example:**
```rust
// src/secrets/mod.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("Missing required secret '{name}'. Set with: cargo pmcp secret set <server>/{name} --prompt")]
    Missing { name: String },
}

/// Get a secret value from environment variables.
///
/// Returns `None` if the secret is not set. For required secrets,
/// use [`require`] which returns an actionable error message.
pub fn get(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Get a required secret value from environment variables.
///
/// Returns an error with actionable guidance if the secret is not set.
pub fn require(name: &str) -> Result<String, SecretError> {
    std::env::var(name).map_err(|_| SecretError::Missing {
        name: name.to_string(),
    })
}
```

### Pattern 4: Target-Specific Secret Handling After Resolution
**What:** AWS Lambda injects found secrets into `DeployConfig.environment` which flows to CDK env vars. pmcp.run shows diagnostic guidance only.
**When to use:** In the deploy command's `execute()` method, after resolution but before target dispatch.
**Example:**
```rust
// In deploy command flow:
let resolution = resolve_secrets(&metadata.resources.secrets, &dotenv_vars);

// Report
print_secret_report(&resolution, global_flags);

// Target-specific behavior
match target_type {
    "aws-lambda" => {
        // Inject found secrets into environment HashMap
        for (key, value) in &resolution.found {
            config.environment.insert(key.clone(), value.clone());
        }
        // Missing secrets are warnings, not errors (D-04)
    },
    "pmcp-run" => {
        // Never send secrets -- show commands for each missing one (D-06, D-07)
        for req in &resolution.missing {
            let env_name = req.env_var.as_deref().unwrap_or(&req.name);
            println!("  Run: cargo pmcp secret set --server {} {} --target pmcp --prompt",
                server_id, env_name);
        }
    },
    _ => {}
}
```

### Anti-Patterns to Avoid
- **Mutating process env in resolve step:** The resolution function must be pure -- return data, let callers decide how to use it. `cargo pmcp dev` sets env on the child process, not on itself.
- **Sending secrets to pmcp.run:** Per D-06, the CLI never transmits secret values for pmcp-run target. Only diagnostics.
- **Blocking deploy on missing secrets:** Per D-04, missing secrets are warnings for AWS Lambda, not errors. The server may have other ways to obtain them at runtime.
- **Duplicating .env parsing logic:** Use dotenvy once, share the parsed HashMap between resolution and dev command.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| .env file parsing | Custom KEY=VALUE parser | `dotenvy` 0.15.7 | Edge cases: quoted values with spaces, multiline values with `\n`, Windows CRLF, BOM handling, `export` prefix, inline comments. dotenvy handles all of these. |
| Secret value sanitization | Custom redaction in logs | Existing `SecretValue` wrapper | Already uses `secrecy` crate with zeroization and `[REDACTED]` Debug/Display |
| Secret requirement extraction | Manual TOML parsing | Existing `McpMetadata::extract()` | Already parses all config formats and returns `Vec<SecretRequirement>` |

**Key insight:** The existing codebase already has 80% of the infrastructure. This phase is primarily about wiring existing pieces together and adding the thin SDK module.

## Common Pitfalls

### Pitfall 1: dotenvy Overwriting Process Environment
**What goes wrong:** Calling `dotenvy::dotenv()` modifies the current process's environment variables, which can interfere with the CLI tool's own operation.
**Why it happens:** The default dotenvy API is designed for server startup, not CLI tools that need to pass env vars to a child process.
**How to avoid:** Use `dotenvy::from_path_iter()` to get an iterator of key-value pairs WITHOUT modifying process env. Collect into a HashMap.
**Warning signs:** Unit tests start failing non-deterministically because env vars leak between tests.

### Pitfall 2: Secret Values in CDK Context Arguments
**What goes wrong:** Secret values containing single quotes, double quotes, spaces, or special shell characters break the CDK command line when injected via `-c 'key=value'`.
**Why it happens:** `McpMetadata::to_cdk_context()` uses shell quoting. Secret values need different handling.
**How to avoid:** Secrets should be injected as environment variables to the CDK process (`.env("KEY", value)`), not as CDK context args. The CDK stack TypeScript reads `process.env` to set Lambda environment variables. This is how `DeployConfig.environment` is already used.
**Warning signs:** Deployments fail when API keys contain `=` or `'` characters.

### Pitfall 3: .env File Precedence Mismatch with dotenvy Default
**What goes wrong:** dotenvy's default behavior is to NOT overwrite existing environment variables (existing env takes precedence). This matches D-13 when using `dotenv()`, but if using `EnvLoader::load_and_modify()` with override mode, existing env vars get overwritten.
**Why it happens:** Different dotenvy APIs have different override behavior.
**How to avoid:** Don't use `load_and_modify()` at all. Use `from_path_iter()` to get a HashMap, then check `std::env::var()` first in the resolution function. This makes precedence explicit.
**Warning signs:** Developer sets `API_KEY` in shell, .env file has a different value, and the .env value is used instead.

### Pitfall 4: SecretRequirement.env_var vs .name Confusion
**What goes wrong:** Some `SecretRequirement` entries have an `env_var` field that differs from the `name` field. Code that uses `name` to look up env vars will miss secrets where the env var name is different.
**Why it happens:** The `name` field is the human-readable secret name (e.g., "TFL App Key"), while `env_var` is the actual environment variable name (e.g., "TFL_APP_KEY").
**How to avoid:** Always use `req.env_var.as_deref().unwrap_or(&req.name)` to get the lookup key.
**Warning signs:** Secrets show as "missing" even though the env var is set under a different name.

### Pitfall 5: CDK Environment Variable Injection Path
**What goes wrong:** Resolved secrets are added to `DeployConfig.environment` but never reach the Lambda function because the CDK stack doesn't read from that HashMap.
**Why it happens:** The CDK deploy currently uses `DeployExecutor::run_cdk_deploy()` which sets `SERVER_NAME` and `AWS_REGION` as env vars, but there's no mechanism to pass arbitrary env vars from `DeployConfig.environment` to the CDK process.
**How to avoid:** Extend `run_cdk_deploy()` to pass all `config.environment` entries as env vars to the CDK process. The CDK TypeScript stack should read these and set them on the Lambda function.
**Warning signs:** Secrets are "found" in the resolution report but not available at runtime.

## Code Examples

### Common Operation 1: Loading .env Without Process Mutation
```rust
// Source: dotenvy docs (https://docs.rs/dotenvy/0.15.7)
use std::collections::HashMap;
use std::path::Path;

/// Load .env file into a HashMap without modifying process environment.
pub fn load_dotenv(project_root: &Path) -> HashMap<String, String> {
    let env_path = project_root.join(".env");
    if !env_path.exists() {
        return HashMap::new();
    }

    match dotenvy::from_path_iter(&env_path) {
        Ok(iter) => iter.filter_map(|item| item.ok()).collect(),
        Err(e) => {
            eprintln!("Warning: Failed to parse .env file: {}", e);
            HashMap::new()
        }
    }
}
```

### Common Operation 2: Injecting Env Vars into Child Process
```rust
// Source: existing pattern in cargo-pmcp/src/commands/dev.rs line 207
// Current code:
let status = Command::new("cargo")
    .args(["run", "--bin", &server_binary])
    .env("MCP_HTTP_PORT", port.to_string())
    .env("RUST_LOG", "info")
    .status();

// After modification for .env injection:
let dotenv_vars = load_dotenv(&project_root);
let mut cmd = Command::new("cargo");
cmd.args(["run", "--bin", &server_binary])
    .env("MCP_HTTP_PORT", port.to_string())
    .env("RUST_LOG", "info");

// .env vars are set first, then existing env takes precedence
// (Command inherits parent env by default, which overwrites .env values)
for (key, value) in &dotenv_vars {
    // Only set if not already in the environment (D-13)
    if std::env::var(key).is_err() {
        cmd.env(key, value);
    }
}

let status = cmd.status();
```

### Common Operation 3: SDK Secret Reader
```rust
// Source: mirrors existing pattern in src/assets/mod.rs
// src/secrets/mod.rs

//! Runtime secret access for MCP servers.
//!
//! Provides helpers to read secrets that were injected as environment
//! variables during deployment. For setting secrets, use the CLI:
//!
//! ```bash
//! # Local development
//! cargo pmcp secret set chess/ANTHROPIC_API_KEY --prompt
//!
//! # pmcp.run
//! cargo pmcp secret set --server chess ANTHROPIC_API_KEY --target pmcp --prompt
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use pmcp::secrets;
//!
//! // Optional secret
//! if let Some(key) = secrets::get("ANALYTICS_KEY") {
//!     configure_analytics(&key);
//! }
//!
//! // Required secret (returns error with actionable guidance)
//! let api_key = secrets::require("ANTHROPIC_API_KEY")?;
//! ```

use thiserror::Error;

/// Error returned when a required secret is not available.
#[derive(Debug, Error)]
pub enum SecretError {
    /// The secret environment variable is not set.
    #[error("Missing required secret '{name}'. Set with: cargo pmcp secret set <server>/{name} --prompt")]
    Missing {
        /// The environment variable name that was expected.
        name: String,
    },
}

/// Get an optional secret from environment variables.
///
/// Returns `None` if the environment variable is not set.
/// Use [`require`] for secrets that must be present.
pub fn get(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Get a required secret from environment variables.
///
/// Returns an actionable error message if the secret is not set,
/// including the exact CLI command to set it.
pub fn require(name: &str) -> Result<String, SecretError> {
    std::env::var(name).map_err(|_| SecretError::Missing {
        name: name.to_string(),
    })
}
```

### Common Operation 4: Secret Resolution Report
```rust
// CLI output for secret resolution (formatting per Claude's discretion)
fn print_secret_report(
    resolution: &SecretResolution,
    server_id: &str,
    target: &str,
    global_flags: &GlobalFlags,
) {
    if !global_flags.should_output() {
        return;
    }

    if resolution.found.is_empty() && resolution.missing.is_empty() {
        println!("   No secrets required");
        return;
    }

    if !resolution.found.is_empty() {
        println!("   Found {} secret(s):", resolution.found.len());
        for key in resolution.found.keys() {
            println!("     {} {}", "✓".green(), key);
        }
    }

    if !resolution.missing.is_empty() {
        let severity = if target == "pmcp-run" { "info" } else { "warn" };
        println!("   Missing {} secret(s):", resolution.missing.len());
        for req in &resolution.missing {
            let env_name = req.env_var.as_deref().unwrap_or(&req.name);
            let required_marker = if req.required { " (required)" } else { "" };
            println!("     {} {}{}", "✗".yellow(), env_name, required_marker);

            if target == "pmcp-run" {
                // D-07: show exact command
                println!("       Run: cargo pmcp secret set --server {} {} --target pmcp --prompt",
                    server_id, env_name);
            }
        }
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| dotenv crate | dotenvy crate | 2022 (RUSTSEC-2021-0141) | dotenvy is the maintained fork; dotenv is unmaintained |
| AWS Lambda Secrets Extension | Environment variables | N/A (design choice) | Simpler, works offline, no Lambda Extension cold start overhead |
| SDK-level secret framework | Thin env-var reader | D-11 (this phase) | Deliberate simplicity; framework later based on developer feedback |

**Deprecated/outdated:**
- `dotenv` crate: Unmaintained since 2020, security advisory RUSTSEC-2021-0141. Use `dotenvy` instead.

## Discretion Recommendations

### .env parsing: Use dotenvy (RECOMMENDED)
dotenvy 0.15.7 is a single, well-maintained dependency (73KB compressed) that handles all `.env` edge cases. Hand-rolling a parser would require handling: quoted values with spaces (`KEY="hello world"`), inline comments (`KEY=value # comment`), multiline values, `export` prefix stripping, Windows CRLF, BOM. The risk of bugs in a hand-rolled parser outweighs the small dependency cost. dotenvy is already used by Axum and other major Rust web frameworks.

### --dry-run flag: Add it (RECOMMENDED)
A `--dry-run` flag for `cargo pmcp deploy` that shows the secret resolution report without actually deploying is low-effort (skip the target dispatch) and high-value for debugging. It helps developers verify their `.env` file and env vars before committing to a deployment.

### Warning/info message formatting: Use colored output (RECOMMENDED)
Found secrets get green checkmarks, missing secrets get yellow warning markers. For pmcp-run target, missing secrets show the exact `cargo pmcp secret set` command. The `colored` crate is already a dependency. Follow the existing `should_output()` guard pattern.

### Test strategy: Unit tests on resolve function + integration test for dev flow (RECOMMENDED)
The resolution function is pure (takes requirements + env HashMap, returns found/missing) and is trivially unit-testable with no mocking. The `cargo pmcp dev` .env loading can be tested with a tempdir containing a `.env` file and verifying the child Command receives the expected env vars. SDK `secrets::require()` can be tested by setting/unsetting env vars in test functions.

## Open Questions

1. **CDK env var passthrough mechanism**
   - What we know: `DeployExecutor::run_cdk_deploy()` passes `SERVER_NAME` and `AWS_REGION` to the CDK process. `DeployConfig.environment` exists with `RUST_LOG=info`.
   - What's unclear: The CDK TypeScript stack must read these env vars and set them on the Lambda function. Need to verify the CDK stack template generator handles arbitrary env vars.
   - Recommendation: Extend `run_cdk_deploy()` to pass all `config.environment` entries. If the CDK stack doesn't already read arbitrary env vars, the deploy init template needs updating.

2. **Secret name to env var name mapping for pmcp.run guidance**
   - What we know: `SecretRequirement` has both `name` (human-readable) and `env_var` (env var name).
   - What's unclear: When showing `cargo pmcp secret set --server <id> <SECRET_NAME>`, should we use the `name` field or the `env_var` field? They serve different purposes.
   - Recommendation: Use `env_var` if set, fall back to `name`. The `--server` flag scopes by server ID. Show both name and env_var in the report if they differ.

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `cargo-pmcp/src/secrets/` -- full module structure, `SecretProvider` trait, `LocalSecretProvider`, `PmcpRunSecretProvider`
- Codebase inspection: `cargo-pmcp/src/deployment/metadata.rs` -- `SecretRequirement` struct, `McpMetadata::extract()`
- Codebase inspection: `cargo-pmcp/src/deployment/config.rs` -- `DeployConfig.environment` and `DeployConfig.secrets` HashMaps
- Codebase inspection: `cargo-pmcp/src/deployment/trait.rs` -- `DeploymentTarget::secrets()` stub method, `SecretsAction` enum
- Codebase inspection: `cargo-pmcp/src/commands/dev.rs` -- child process spawn with `Command::env()`
- Codebase inspection: `src/assets/mod.rs` and `src/assets/loader.rs` -- pattern for SDK module with free functions
- Codebase inspection: `src/lib.rs` -- where `pub mod secrets` would be added

### Secondary (MEDIUM confidence)
- [dotenvy crate docs](https://docs.rs/dotenvy/latest/dotenvy/) -- API for `.env` parsing
- [dotenvy GitHub](https://github.com/allan2/dotenvy) -- well-maintained fork of dotenv
- crates.io: dotenvy 0.15.7 confirmed current

### Tertiary (LOW confidence)
- None -- all findings verified against codebase and official docs.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- dotenvy is the de facto standard, codebase already has all other needed deps
- Architecture: HIGH -- all integration points identified by code inspection, patterns established by assets module
- Pitfalls: HIGH -- identified from actual code paths (CDK injection, dotenvy API variants, env_var vs name)

**Research date:** 2026-03-29
**Valid until:** 30 days (stable domain, no fast-moving dependencies)
