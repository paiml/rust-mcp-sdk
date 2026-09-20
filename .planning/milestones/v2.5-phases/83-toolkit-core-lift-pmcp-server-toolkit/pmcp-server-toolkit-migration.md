# Migrating to `pmcp-server-toolkit`

**Audience:** Developers who currently build MCP servers against `pmcp-run`'s
internal `mcp-server-common`, or who hand-rolled the AuthProvider /
SecretsProvider / static-handler / HMAC-token plumbing themselves.

**Source of truth:** This document is the Phase 83 anchor for REF-03. Phase 89
expands this into a full book chapter in `pmcp-book/src/`; this file is the
developer-facing one-pager.

## What changed in Phase 83

The proto-SDK that lived at `pmcp-run/built-in/shared/mcp-server-common/` is
now a public crates.io crate: `pmcp-server-toolkit`.

It ships:

- `AuthProvider` impls (`StaticAuthProvider` at the crate root; AWS-gated
  providers under `pmcp_server_toolkit::auth`).
- `SecretsProvider` trait + `EnvSecrets` (+ AWS-gated `SsmSecrets`,
  `SecretsManagerSecrets`, `OrgSecretsManagerProvider`). Per Phase 83 review
  R6, the secret type is the toolkit-owned `SecretValue` (NOT
  `pmcp_code_mode::TokenSecret`) so the trait works under
  `--no-default-features`.
- `StaticResourceHandler`, `StaticPromptHandler` + `prompt_handlers_from_config`.
- HMAC token machinery (re-exported from `pmcp-code-mode` per D-16; lives
  under `pmcp_server_toolkit::code_mode::*` when the `code-mode` feature is on).
- `[[tools]]` → `ToolInfo` synthesizer with zero per-tool Rust handlers
  (`synthesize_from_config`).
- `[code_mode]` config → `ValidationPipeline` wiring
  (`validation_pipeline_from_config`). Per Phase 83 review R9, `token_secret`
  is `env:`-only by default; inline literals are rejected unless
  `allow_inline_token_secret_for_dev = true` is explicitly set.
- `SqlConnector` trait stub (Phase 83 ships 2 methods per review R2;
  Phase 84 ships `execute()` + placeholder translation once real connectors
  validate the contract).
- `ServerBuilderExt` — chainable extension trait on `pmcp::ServerBuilder`
  with both panicking (`tools_from_config`, `code_mode_from_config`) and
  fallible (`try_tools_from_config`, `try_code_mode_from_config`) variants
  per review R7.

## One-page migration

### Before

Your `Cargo.toml`:

```toml
[dependencies]
mcp-server-common = { path = "../shared/mcp-server-common" }
pmcp = "..."
async-trait = "..."
# ... and all the deps mcp-server-common needed transitively
```

Your `main.rs`:

```rust
use mcp_server_common::{StaticAuthProvider, SecretsProvider, /* ... */};
```

### After

Your `Cargo.toml`:

```toml
[dependencies]
pmcp = "2.8.1"
pmcp-server-toolkit = { version = "0.1.0", features = ["code-mode"] }
# That's it — the toolkit pulls the rest transitively.
```

Your `main.rs` — per Phase 83 review R3, this is ONE crate-root import line
(the headline D-15 DX promise):

```rust
use pmcp_server_toolkit::{
    AuthProvider, ServerBuilderExt, ServerConfig,
    SecretValue, SecretsProvider,
    StaticAuthProvider, StaticPromptHandler, StaticResourceHandler,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = ServerConfig::from_toml_strict_validated(include_str!("config.toml"))?;

    // Per review R7, use the try_* variants in production:
    let server = pmcp::Server::builder()
        .name(&cfg.server.name)
        .version(&cfg.server.version)
        .try_tools_from_config(&cfg)?
        .try_code_mode_from_config(&cfg)?
        .resources_arc(std::sync::Arc::new(StaticResourceHandler::from(&cfg)))
        .auth_provider_arc(std::sync::Arc::new(StaticAuthProvider::new("dev-token")))
        .build()?;
    server.run_stdio().await?;
    Ok(())
}
```

See `crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs` for a
runnable Shape C ≤15-line `main.rs` that compiles in CI.

## Symbol mapping

| Old (`mcp-server-common`)                 | New (`pmcp-server-toolkit` — crate root)                                                                                                                                                                                       |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `mcp_server_common::AuthProvider`         | `pmcp_server_toolkit::AuthProvider`                                                                                                                                                                                            |
| `mcp_server_common::StaticAuthProvider`   | `pmcp_server_toolkit::StaticAuthProvider`                                                                                                                                                                                      |
| `mcp_server_common::SecretsProvider`      | `pmcp_server_toolkit::SecretsProvider`                                                                                                                                                                                         |
| `mcp_server_common::SecretValue`          | `pmcp_server_toolkit::SecretValue`                                                                                                                                                                                             |
| `mcp_server_common::EnvSecrets`           | `pmcp_server_toolkit::EnvSecrets`                                                                                                                                                                                              |
| `mcp_server_common::StaticResourceHandler`| `pmcp_server_toolkit::StaticResourceHandler`                                                                                                                                                                                   |
| `mcp_server_common::StaticPromptHandler`  | `pmcp_server_toolkit::StaticPromptHandler` (+ `prompt_handlers_from_config` at the crate root for multi-prompt servers)                                                                                                        |
| (custom HMAC code)                        | `pmcp_server_toolkit::code_mode::HmacTokenGenerator` (re-exported from `pmcp-code-mode`)                                                                                                                                       |
| (custom `ToolInfo` construction)          | `pmcp_server_toolkit::synthesize_from_config` (power user) OR `pmcp_server_toolkit::ServerBuilderExt::try_tools_from_config` (common path, review R7)                                                                          |
| (custom code-mode glue)                   | `pmcp_server_toolkit::code_mode::validation_pipeline_from_config` (power user) OR `pmcp_server_toolkit::ServerBuilderExt::try_code_mode_from_config` (common path, review R7)                                                  |

## Dropped from `mcp-server-common`

Per CONTEXT.md D-14 these features were intentionally NOT lifted into the
toolkit:

- `ddb`, `dynamo-config` — pmcp-run-specific concerns; keep the dep explicit
  if your server uses DDB-backed task storage.
- `openapi-code-mode`, `js-runtime`, `mcp-code-mode` — Phase 3 OpenAPI
  territory; the OpenAPI server core retains these.

If your server uses any of these, keep the corresponding deps explicit in
your `Cargo.toml`.

## Phase 83 review-driven behaviors to know about

- **Review R3 — single crate-root import:** Every public symbol you need is
  re-exported at the toolkit crate root. If you find yourself writing
  `pmcp_server_toolkit::auth::*` or `pmcp_server_toolkit::config::*`, file
  an issue — the re-export is missing (with the documented exception of
  power-user `code_mode::*` helpers).
- **Review R5 — secret-leak compile guards:** `SecretValue` does NOT impl
  `Debug`/`Display`/`Clone`/`Serialize`/`Deserialize`/`PartialEq`.
  `format!("{:?}", secret)` won't compile. Trybuild tests at
  `crates/pmcp-server-toolkit/tests/compile_fail/` enforce this.
- **Review R6 — secret type stable across features:** `SecretsProvider::get`
  returns `SecretValue` (toolkit-owned, feature-independent), not the
  `code-mode`-gated `TokenSecret`. `cargo build --no-default-features`
  succeeds for the toolkit crate.
- **Review R7 — `try_*` variants:** Production code should prefer
  `try_tools_from_config` / `try_code_mode_from_config` over the panicking
  forms. The panicking variants exist for the Shape C ≤15-line `main.rs`
  demo path; production deployments deserve `Result`-based error surfacing.
- **Review R8 — `validate()`:** Prefer `from_toml_strict_validated` over
  `from_toml` to catch empty required fields (`server.name`,
  `server.version`, tool names, table names).
- **Review R9 — `token_secret` env-only by default:** Inline literals are
  rejected. Use `token_secret = "env:VAR_NAME"`. If a dev/test workflow
  truly needs an inline literal, set
  `[code_mode] allow_inline_token_secret_for_dev = true` and document it
  loudly — your CI should refuse PRs that set this flag.

## REF-01 superset guarantee

Your existing `config.toml` from a pmcp-run sql-api server (`open-images` /
`imdb` / `msr-vtt`) parses unchanged through `ServerConfig::from_toml`. If
yours doesn't, it's a P83 bug — file an issue. The three reference fixtures
ship under `crates/pmcp-server-toolkit/tests/fixtures/` and are exercised by
the `reference_configs.rs` integration test.

## Where to go next

- **Phase 84:** per-backend `SqlConnector` impls (Postgres / MySQL / Athena
  + SQLite feature flag). This phase will also land the FULL trait surface
  (`execute()` + placeholder translation per review R2's
  semver-evolution plan).
- **Phase 85:** `pmcp-sql-server` Shape A — a pure-config-driven binary
  reproducing the `open-images` reference server.
- **Phase 86:** `cargo pmcp new --kind sql-server` scaffold + Shape C
  ≤15-line `main.rs` example + Shape D deploy.
- **Phase 87:** `pmcp-config-helper` — Type 2 MCP authoring skills server.
- **Phase 88:** dogfood — `crates/pmcp-server` on the toolkit.
- **Phase 89:** this one-pager expands into a full book chapter.

## Reporting issues

`pmcp-server-toolkit` issues belong in the `paiml/rust-mcp-sdk` issue
tracker. Cross-reference Phase 83 by including `phase-83` in the issue
labels. For migration breakages specifically, include:

- The pre-shim `Cargo.toml` and `lib.rs` excerpt.
- The post-shim build/test failure verbatim.
- Whether your server was using one of the dropped features.

This makes the diagnosis loop (R3 missing re-export vs R6 cross-feature
trap vs dropped-feature regression) deterministic.
