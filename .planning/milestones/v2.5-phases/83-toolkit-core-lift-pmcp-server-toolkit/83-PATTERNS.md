# Phase 83: Toolkit Core Lift (`pmcp-server-toolkit`) — Pattern Map

**Mapped:** 2026-05-18
**Files analyzed:** 18 (new files for `crates/pmcp-server-toolkit/` + fuzz + contract)
**Analogs found:** 17 / 18 (1 file — `secrets.rs` — has no in-workspace analog; uses external `mcp-server-common` lift verbatim plus secrecy/HmacTokenGenerator pattern from `pmcp-code-mode`)
**Lift source (external, NOT in this repo):** `pmcp-run/built-in/shared/mcp-server-common/src/{auth,secrets,resources,prompts}.rs`

---

## File Classification

| New/Modified File | Role | Data Flow | Closest In-Workspace Analog | Match Quality |
|-------------------|------|-----------|-----------------------------|---------------|
| `crates/pmcp-server-toolkit/Cargo.toml` | config | n/a | `crates/pmcp-tasks/Cargo.toml` + `crates/pmcp-code-mode/Cargo.toml` | exact (workspace-version + feature-matrix pattern) |
| `crates/pmcp-server-toolkit/src/lib.rs` | module-root | n/a | `crates/pmcp-code-mode/src/lib.rs` | exact (attribution header + flat re-export shape from D-15/D-16) |
| `crates/pmcp-server-toolkit/src/auth.rs` | trait-impl | request-response | `src/server/auth/mock.rs` + `src/server/auth/traits.rs` | exact (consumes `pmcp::AuthProvider`; no new trait) |
| `crates/pmcp-server-toolkit/src/secrets.rs` | trait + impls | request-response | `crates/pmcp-code-mode/src/token.rs` (`TokenSecret`) | partial (no `SecretsProvider` analog; secrecy newtype pattern is the model) |
| `crates/pmcp-server-toolkit/src/resources.rs` | trait-impl | request-response | `src/server/skills.rs` (IndexMap + `Content::resource_with_text`) + `src/server/simple_resources.rs` (`StaticResource`) | exact |
| `crates/pmcp-server-toolkit/src/prompts.rs` | trait-impl | request-response | `src/server/simple_prompt.rs` (`SimplePrompt`) + `src/server/skills.rs::SkillPromptHandler` | exact |
| `crates/pmcp-server-toolkit/src/hmac.rs` *(see Notes — actually a re-export module per D-16)* | re-export | n/a | `crates/pmcp-code-mode/src/lib.rs:160-163` (`pub use token::*`) | exact (re-export only; NO duplicate impl) |
| `crates/pmcp-server-toolkit/src/config.rs` | model | transform | `cargo-pmcp/src/commands/configure/config.rs` (`#[serde(deny_unknown_fields)]`) + `crates/pmcp-code-mode/src/config.rs` (field-alias pattern) | exact |
| `crates/pmcp-server-toolkit/src/tools.rs` | service | transform | `src/types/tools.rs:209-263` (`ToolInfo::new` / `ToolInfo::with_annotations`) | exact (net-new code; uses constructor pattern from analog) |
| `crates/pmcp-server-toolkit/src/code_mode.rs` | service + re-export | transform | `crates/pmcp-code-mode/src/code_executor.rs:55-67` + `crates/pmcp-code-mode/src/lib.rs:118-186` | exact |
| `crates/pmcp-server-toolkit/src/sql/mod.rs` *(or `connector.rs`)* | trait | request-response | `src/server/traits.rs:38-58` (async_trait + Send+Sync shape) + spike 005 verbatim | exact (trait shape from spike) |
| `crates/pmcp-server-toolkit/src/code_mode.rs::assemble_code_mode_prompt` | utility | transform | `src/server/skills.rs::SkillPromptHandler` (description+template composition) | role-match |
| `crates/pmcp-server-toolkit/src/builder_ext.rs` | extension-trait | transform | `src/server/builder.rs:200-272` (`tool_arc` / `prompt_arc`) + `src/server/builder.rs:440-445` (`auth_provider_arc`) | exact |
| `crates/pmcp-server-toolkit/tests/fixtures/*.toml` | fixture | n/a | `crates/pmcp-code-mode/tests/property_tests.rs` (uses inline fixtures via `CodeModeConfig::from_toml`) | role-match |
| `crates/pmcp-server-toolkit/tests/code_mode_policy.rs` | test | integration | `crates/pmcp-code-mode/tests/property_tests.rs` | exact |
| `crates/pmcp-server-toolkit/tests/backend_core_smoke.rs` | test | integration | `tests/in_process_handler_pattern.rs` (Phase 82 builder-Arc round-trip) | exact |
| `crates/pmcp-server-toolkit/fuzz/Cargo.toml` + `fuzz_targets/pmcp_server_toolkit_config_parser.rs` | fuzz | transform | `cargo-pmcp/fuzz/fuzz_targets/pmcp_config_toml_parser.rs` + `cargo-pmcp/fuzz/Cargo.toml` | exact (mirror Phase 77 pattern; different `T` for `toml::from_str`) |
| `contracts/binding.yaml` (extend) | contract | n/a | `contracts/binding.yaml` lines 1-90 | exact (append rows; per RESEARCH §"Open Questions" #1) |

---

## Pattern Assignments

### 1. `crates/pmcp-server-toolkit/Cargo.toml` (new crate manifest)

**Primary analog:** `crates/pmcp-tasks/Cargo.toml` — newest workspace crate, modern `aws-*` + workspace-version pattern.
**Secondary analog:** `crates/pmcp-code-mode/Cargo.toml` — feature-matrix shape (default-features=false on AWS, `[package.metadata.docs.rs]`).
**Tertiary analog:** `crates/mcp-tester/Cargo.toml:21` — exact workspace-version dep syntax.

**Package + metadata header pattern** (from `pmcp-code-mode/Cargo.toml:1-14`):
```toml
[package]
name = "pmcp-server-toolkit"
version = "0.1.0"              # D-07: fresh 0.x
edition = "2021"
license = "MIT"
repository = "https://github.com/paiml/rust-mcp-sdk"
description = "Runtime library for config-driven MCP servers — auth, secrets, static resources/prompts, [[tools]] synthesizer, code-mode wiring"
readme = "README.md"
keywords = ["mcp", "toolkit", "config-driven", "code-mode", "lambda"]
categories = ["development-tools", "web-programming"]

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

**Workspace-version dep pattern** (from `crates/mcp-tester/Cargo.toml:21` + `crates/pmcp-tasks/Cargo.toml:10`):
```toml
[dependencies]
pmcp = { version = "2.8.1", path = "../..", default-features = false }
pmcp-code-mode = { version = "0.5.1", path = "../pmcp-code-mode", default-features = false, optional = true }
```
**Why both `version` AND `path`:** Cargo uses `path` locally; `cargo publish` emits `version`. CONTEXT.md D-05. **Verified at `crates/mcp-tester/Cargo.toml:21`** and `crates/pmcp-tasks/Cargo.toml:10`.

**AWS feature-gated dep pattern** (from `crates/pmcp-tasks/Cargo.toml:21-26`):
```toml
# Why: default features enable `rustls` → aws-smithy-runtime/tls-rustls →
# aws-smithy-http-client/legacy-rustls-ring → rustls 0.21 / rustls-webpki 0.101
# which is flagged by RUSTSEC-2026-0098/0099/0104. Opt into the modern
# `default-https-client` path (rustls 0.23 via aws-lc-rs) instead.
aws-sdk-secretsmanager = { version = "1", default-features = false, features = ["default-https-client", "rt-tokio", "behavior-version-latest"], optional = true }
aws-sdk-ssm = { version = "1", default-features = false, features = ["default-https-client", "rt-tokio", "behavior-version-latest"], optional = true }
aws-config = { version = "1", default-features = false, features = ["default-https-client", "rt-tokio", "credentials-process", "sso", "behavior-version-latest"], optional = true }
```
**Critical:** Always include the `# Why:` comment with RUSTSEC IDs and `default-features = false`. This is the security-pinned pattern from Phase ~76.

**Feature matrix** (per CONTEXT.md D-14):
```toml
[features]
default = ["code-mode"]                              # D-12
code-mode = ["dep:pmcp-code-mode"]                   # D-06
aws = ["dep:aws-config", "dep:aws-sdk-secretsmanager", "dep:aws-sdk-ssm"]
avp = ["code-mode", "pmcp-code-mode/avp"]
input-validation = ["dep:jsonschema"]
sqlite = ["dep:rusqlite"]
```

**Crate-size guard** (per RESEARCH §"Pitfall 6" — pmcp 2.2.0 blew the 10MB limit):
```toml
exclude = [".planning/", ".pmat/", "fixtures/", "tests/", "fuzz/"]
```
*(Verify with `cargo package --list` before publish.)*

---

### 2. `crates/pmcp-server-toolkit/src/lib.rs` (module root + crate re-exports)

**Primary analog:** `crates/pmcp-code-mode/src/lib.rs:1-213` (attribution header + clippy-allow block for lifted code + module declarations + crate-root re-exports).

**Attribution header pattern** (from `crates/pmcp-code-mode/src/lib.rs:1-20`):
```rust
// Originated from pmcp-run/built-in/shared/mcp-server-common (https://github.com/guyernest/pmcp-run)
// Moved into rust-mcp-sdk workspace as a first-class public SDK crate for Phase 83.
//
// Clippy pedantic/nursery allows for code imported from pmcp-run.
// These will be cleaned up incrementally in future phases.
#![allow(clippy::use_self)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::needless_raw_string_hashes)]
// ... (extend as the lift surfaces specific clippy hits; mirror pmcp-code-mode's set)
```

**Crate-root re-export pattern** (D-15/D-16 — from `crates/pmcp-code-mode/src/lib.rs:118-186`):
```rust
// Headline types re-exported at crate root per D-15
pub use auth::{AuthProvider, StaticAuthProvider};      // re-exports `pmcp::AuthProvider` trait
pub use secrets::{SecretsProvider, EnvSecrets};
pub use config::ServerConfig;
pub use resources::StaticResourceHandler;
pub use prompts::StaticPromptHandler;
pub use sql::{SqlConnector, Dialect};
pub use builder_ext::ServerBuilderExt;

// code_mode submodule fully re-exports pmcp-code-mode (D-16)
#[cfg(feature = "code-mode")]
pub mod code_mode {
    pub use pmcp_code_mode::{
        ApprovalToken, HmacTokenGenerator, TokenGenerator, TokenSecret,
        canonicalize_code, compute_context_hash, hash_code,
        CodeExecutor, NoopPolicyEvaluator, PolicyEvaluator, AuthorizationDecision,
        ValidationContext, ValidationPipeline, CodeModeConfig,
    };

    #[cfg(feature = "avp")]
    pub use pmcp_code_mode::{AvpClient, AvpConfig, AvpPolicyEvaluator};

    // Net-new code in the toolkit (NOT re-exports)
    pub use super::code_mode_impl::{executor_from_config, assemble_code_mode_prompt};
}
```

**Anti-pattern from analog (DO NOT replicate):** `pmcp-code-mode/src/lib.rs:9-20` carries ~14 `#![allow(clippy::...)]` lines. These were added incrementally during the original lift. For the toolkit, start with a smaller set and extend only as clippy actually flags hits — don't pre-blanket the file.

---

### 3. `crates/pmcp-server-toolkit/src/auth.rs` (AuthProvider impls — lift)

**Primary analog:** `src/server/auth/mock.rs:1-120` — shows the canonical `impl AuthProvider for Foo` shape.
**Trait definition (DO NOT redefine):** `src/server/auth/traits.rs:447-474`.

**Trait shape (consume `pmcp::AuthProvider`; do not redefine):**
```rust
// From src/server/auth/traits.rs:447-474 — already public on `pmcp::AuthProvider`
#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn validate_request(
        &self,
        authorization_header: Option<&str>,
    ) -> Result<Option<AuthContext>>;

    fn auth_scheme(&self) -> &'static str { "Bearer" }
    fn is_required(&self) -> bool { true }
}
```

**Impl-side pattern** (lift body from external `mcp-server-common/auth.rs`; structure mirrors `src/server/auth/mock.rs:56-120`):
```rust
use async_trait::async_trait;
use pmcp::server::auth::{AuthContext, AuthProvider};
use pmcp::Result;

/// Static bearer-token auth, suitable for dev and tests.
/// Lifted verbatim from mcp-server-common/src/auth.rs::StaticAuthProvider.
pub struct StaticAuthProvider {
    expected_token: String,
}

impl StaticAuthProvider {
    pub fn new(expected_token: impl Into<String>) -> Self {
        Self { expected_token: expected_token.into() }
    }
}

#[async_trait]
impl AuthProvider for StaticAuthProvider {
    async fn validate_request(
        &self,
        authorization_header: Option<&str>,
    ) -> Result<Option<AuthContext>> {
        // lift body verbatim from mcp-server-common
        // ...
    }
}
```

**Anti-pattern:** RESEARCH §"Anti-Patterns" — **don't redefine `AuthProvider` / `ResourceHandler` / `PromptHandler` traits**. Use `pmcp::*` trait and only provide impls. Verified at `src/server/auth/traits.rs:450`.

---

### 4. `crates/pmcp-server-toolkit/src/secrets.rs` (SecretsProvider trait + impls — lift)

**Primary analog:** `crates/pmcp-code-mode/src/token.rs:14-69` (`TokenSecret` secrecy-wrapped newtype) — the secrecy pattern to apply to every secret-bearing return type.
**No in-workspace `SecretsProvider` analog exists.** Lift body verbatim from external `mcp-server-common/src/secrets.rs` (701 LoC, three impls: `EnvSecrets`, `SsmSecrets`, `SecretsManagerSecrets`).

**Secrecy newtype pattern to enforce** (from `crates/pmcp-code-mode/src/token.rs:38-46`):
```rust
// SAFETY NOTE: TokenSecret intentionally does NOT derive or implement:
// - Debug (prevents logging secret bytes)
// - Display (prevents printing secret bytes)
// - Clone (prevents accidental copies that bypass zeroize)
// - Serialize / Deserialize (prevents JSON/wire leakage)
// - PartialEq / Eq (prevents timing side-channel comparisons)
// These denials are verified by negative trait tests in Plan 05.
pub struct TokenSecret(SecretBox<[u8]>);

impl TokenSecret {
    pub fn new(secret: impl Into<Vec<u8>>) -> Self {
        let bytes: Vec<u8> = secret.into();
        Self(SecretBox::new(Box::from(bytes.as_slice())))
    }

    pub fn from_env(var: &str) -> Result<Self, std::env::VarError> {
        let val = std::env::var(var)?;
        Ok(Self::new(val.into_bytes()))
    }

    pub fn expose_secret(&self) -> &[u8] {
        self.0.expose_secret()
    }
}
```

**Trait signature constraint** (per RESEARCH §"Security Domain" V6 + threat model T-83-secrets):
```rust
// Return TokenSecret (or equivalent secrecy newtype) NOT String / Vec<u8>:
#[async_trait]
pub trait SecretsProvider: Send + Sync {
    async fn get(&self, name: &str) -> Result<TokenSecret>;
    //                                     ^^^^^^^^^^^^^^^
    //   NEVER `String` or `Vec<u8>` — verify in lift; if mcp-server-common's
    //   lifted impl returns String, wrap-in-TokenSecret at the boundary.
}
```

**Anti-pattern (high-blast-radius):** Returning `String` or `Vec<u8>` from `SecretsProvider::get` — bypasses zeroize-on-drop guarantee and risks `Debug`/log leakage. **If the verbatim lift returns `String`, refactor the return type at lift time and update callers.** This is a Phase 67.1 CMSUP-02 invariant.

---

### 5. `crates/pmcp-server-toolkit/src/resources.rs` (StaticResourceHandler — lift)

**Primary analog:** `src/server/skills.rs:1-80, 36-50` — the IndexMap + `Content::resource_with_text` MIME-typed-wire pattern.
**Secondary analog:** `src/server/simple_resources.rs:1-150` (`StaticResource` + `ResourceCollection`).
**Trait definition (DO NOT redefine):** `src/server/mod.rs:253-270`.

**Trait shape** (consume `pmcp::ResourceHandler`):
```rust
// From src/server/mod.rs:253-270 — already public on `pmcp::ResourceHandler`
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait ResourceHandler: Send + Sync {
    async fn read(&self, uri: &str, extra: RequestHandlerExtra)
        -> Result<crate::types::ReadResourceResult>;
    async fn list(&self, _cursor: Option<String>, extra: RequestHandlerExtra)
        -> Result<crate::types::ListResourcesResult>;
}
```

**IndexMap pattern for deterministic iteration** (from `src/server/skills.rs:37-50`):
```rust
use indexmap::IndexMap;  // NOT HashMap — required for stable listing order

pub struct StaticResourceHandler {
    // IndexMap so resource ordering is deterministic across runs —
    // required for stable example output, snapshot tests, predictable host UX.
    resources: IndexMap<String, /* ResourceEntry */>,
}
```

**MIME-typed-wire pattern** (from `src/server/skills.rs:11-16`):
```rust
//! Wire shape: reads return `Content::resource_with_text` (NOT `Content::text`)
//! so per-resource MIME types survive the wire round-trip — reference files
//! like `schema.graphql` keep their `application/graphql` MIME type.

// Inside ResourceHandler::read impl:
let content = Content::resource_with_text(uri, body, mime_type);
//                     ^^^^^^^^^^^^^^^^^^
//   NOT Content::Text — MIME preservation requires Content::Resource
```

**Constructor-from-config pattern** (per CONTEXT.md TKIT-04 — the toolkit needs `StaticResourceHandler::from(&ServerConfig)`):
```rust
impl From<&crate::config::ServerConfig> for StaticResourceHandler {
    fn from(cfg: &crate::config::ServerConfig) -> Self {
        let mut resources = IndexMap::with_capacity(cfg.resources.len());
        for r in &cfg.resources {
            resources.insert(r.uri.clone(), /* build entry from r */);
        }
        Self { resources }
    }
}
```

---

### 6. `crates/pmcp-server-toolkit/src/prompts.rs` (StaticPromptHandler — lift)

**Primary analog:** `src/server/simple_prompt.rs:14-134` (`SimplePrompt` — argument-validation pattern, `PromptHandler` impl, `metadata()` returning `PromptInfo`).
**Secondary analog:** `src/server/skills.rs::SkillPromptHandler` (dual-surface prompt-fallback pattern referenced in RESEARCH).
**Trait definition (DO NOT redefine):** `src/server/mod.rs:235-251` — `pmcp::PromptHandler`.

**PromptHandler impl shape** (from `src/server/simple_prompt.rs:96-134`):
```rust
#[async_trait]
impl PromptHandler for StaticPromptHandler {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> Result<GetPromptResult> {
        // 1. Validate required arguments
        for arg in &self.arguments {
            if arg.required && !args.contains_key(&arg.name) {
                return Err(pmcp::Error::validation(format!(
                    "Required argument '{}' is missing",
                    arg.name
                )));
            }
        }
        // 2. Render template (lift-specific body from mcp-server-common)
        // ...
    }

    fn metadata(&self) -> Option<PromptInfo> {
        let mut info = PromptInfo::new(&self.name);
        if let Some(desc) = &self.description {
            info = info.with_description(desc);
        }
        if !self.arguments.is_empty() {
            info = info.with_arguments(self.arguments.clone());
        }
        Some(info)
    }
}
```

**Orthogonality note (per RESEARCH §"Risks + Landmines" #3):** Toolkit's `StaticPromptHandler` does **NOT** touch skills. Document in rustdoc: "This handler is orthogonal to `pmcp::server::skills::Skill` / `bootstrap_skill_and_prompt`. Downstream consumers can register both surfaces side-by-side; the toolkit makes no assumption about skill registration."

---

### 7. `crates/pmcp-server-toolkit/src/hmac.rs` — DOES NOT EXIST AS NEW CODE

**Per D-16 + RESEARCH §"Don't Hand-Roll" — HMAC is re-export only.** No `hmac.rs` file; the re-exports live in `src/code_mode.rs`:

**Pattern (from `crates/pmcp-code-mode/src/lib.rs:160-163`):**
```rust
// In crates/pmcp-server-toolkit/src/code_mode.rs (under feature = "code-mode"):
pub use pmcp_code_mode::{
    canonicalize_code, compute_context_hash, hash_code,
    ApprovalToken, HmacTokenGenerator, TokenGenerator, TokenSecret,
};
```

**Anti-pattern (RESEARCH §"Anti-Patterns"):** **Don't redefine HMAC token machinery.** The `TokenSecret` type at `crates/pmcp-code-mode/src/token.rs:38` is already secrecy/zeroize-backed with intentionally-denied `Debug`/`Clone`/`Serialize` per CMSUP-02. Reproducing risks drift from the audited impl.

---

### 8. `crates/pmcp-server-toolkit/src/config.rs` (ServerConfig + sub-structs — net-new)

**Primary analog:** `cargo-pmcp/src/commands/configure/config.rs:113-194` — strict `#[serde(deny_unknown_fields)]` on every variant; tagged-enum-per-variant pattern.
**Secondary analog:** `crates/pmcp-code-mode/src/config.rs:93-300` — field-alias pattern for unprefixed/prefixed config keys.

**Strict-parse struct pattern** (from `cargo-pmcp/src/commands/configure/config.rs:114-127`):
```rust
/// Top-level config — every section gets `#[serde(deny_unknown_fields)]` per D-13.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    #[serde(default)]
    pub server: ServerSection,

    #[serde(default)]
    pub metadata: MetadataSection,

    #[serde(default)]
    pub database: DatabaseSection,

    #[serde(default, rename = "code_mode")]
    pub code_mode: Option<CodeModeSection>,

    #[serde(default)]
    pub tools: Vec<ToolDecl>,

    #[serde(default)]
    pub prompts: Vec<PromptDecl>,

    #[serde(default)]
    pub resources: Vec<ResourceDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ServerSection {
    pub name: String,
    pub version: String,
    // ...
}
```

**Field-alias pattern for prefixed/unprefixed names** (from `crates/pmcp-code-mode/src/config.rs:189-236`):
```rust
// CodeModeConfig accepts BOTH sql_allow_writes AND allow_writes (etc.):
#[serde(default, alias = "allow_writes")]
pub sql_allow_writes: bool,

#[serde(default, alias = "allow_deletes")]
pub sql_allow_deletes: bool,

#[serde(default, alias = "blocked_tables")]
pub sql_blocked_tables: HashSet<String>,
```
**Why this matters:** REF-01 superset invariant (CONTEXT.md, RESEARCH §"Pitfall 1"). Reference config.tomls (open-images, imdb, msr-vtt) use unprefixed names; toolkit must accept both. Aliases are the standard solution — **don't loosen `deny_unknown_fields` to compensate.**

**`from_toml` parse-entrypoint pattern** (from `crates/pmcp-code-mode/src/config.rs:365-380`):
```rust
impl ServerConfig {
    /// Parse `ServerConfig` from a TOML config string.
    pub fn from_toml(toml_str: &str) -> Result<Self, ConfigError> {
        toml::from_str(toml_str).map_err(ConfigError::Parse)
    }
}
```

**Anti-pattern (RESEARCH §"Pitfall 1"):** Don't loosen `#[serde(deny_unknown_fields)]` to satisfy REF-01 superset. REF-01 is enforced by ADDING fields (with `#[serde(default)]` if optional) or by adding `alias = "..."` for renamed keys.

---

### 9. `crates/pmcp-server-toolkit/src/tools.rs` (ToolInfo synthesizer — NET-NEW)

**Primary analog:** `src/types/tools.rs:209-263` — `ToolInfo::new` and `ToolInfo::with_annotations` constructors.
**Secondary analog:** `src/server/builder.rs:200-222` (`tool_arc` registration site — shows the trio `(name, ToolInfo, Arc<dyn ToolHandler>)`).
**Trait analog for handler `Send + Sync`:** `src/server/traits.rs:23-36`.

**ToolInfo constructor (use, do NOT struct-literal — `#[non_exhaustive]`):**
```rust
// From src/types/tools.rs:247-264 — RESEARCH §"Anti-Patterns": don't struct-literal #[non_exhaustive] types
let info = ToolInfo::with_annotations(
    decl.name.clone(),
    Some(decl.description.clone()),
    schema,
    annotations,
);

// And ToolAnnotations is built via fluent builder (src/types/tools.rs:55-135):
let annotations = ToolAnnotations::new()
    .with_read_only(decl.annotations.read_only_hint.unwrap_or(true))
    .with_destructive(decl.annotations.destructive_hint.unwrap_or(false))
    .with_idempotent(decl.annotations.idempotent_hint.unwrap_or(true))
    .with_open_world(decl.annotations.open_world_hint.unwrap_or(false));
```

**Synthesizer signature (D-10 — low-level fn):**
```rust
use pmcp::server::ToolHandler;
use pmcp::types::ToolInfo;
use std::sync::Arc;

pub fn synthesize_from_config(
    config: &crate::config::ServerConfig,
) -> Result<Vec<(String, ToolInfo, Arc<dyn ToolHandler>)>, crate::error::ToolkitError> {
    // Decompose into ≤3 helpers, each ≤cog 25 (per RESEARCH §"Pitfall 5" + Phase 75 CI gate):
    //   1. fn build_input_schema(params: &[ParamDecl]) -> Value
    //   2. fn build_annotations(decl: &AnnotationsDecl) -> ToolAnnotations
    //   3. loop body that composes (1) + (2) + handler construction
    //
    // SynthesizedToolHandler MUST return Some(ToolInfo) from metadata()
    // per RESEARCH §"Risks + Landmines" #2 — tool_arc consumes ToolInfo
    // via handler.metadata() at registration time.
}
```

**SynthesizedToolHandler::metadata() invariant** (from `src/server/builder.rs:205-211`):
```rust
// Phase 82's tool_arc behavior:
let mut info = handler
    .metadata()
    .unwrap_or_else(|| ToolInfo::new(name.clone(), None, serde_json::json!({})));
// ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// FALLBACK is empty {}. Synthesizer's handler MUST return Some(real info)
// or the registered ToolInfo will have empty input_schema. Property test asserts this.
```

**Cognitive-complexity ceiling (per RESEARCH §"Pitfall 5"):** Decompose `synthesize_from_config` into ≤3 helpers each ≤cog 25. Annotated `#[allow(clippy::cognitive_complexity)]` is permitted **only if irreducible** (Phase 75 D-03). The synthesizer is NOT irreducible — refactor.

---

### 10. `crates/pmcp-server-toolkit/src/code_mode.rs` (executor wiring + assembly fn — NET-NEW)

**Primary analog:** `crates/pmcp-code-mode/src/code_executor.rs:55-67` — `CodeExecutor` trait signature.
**Re-export analog:** `crates/pmcp-code-mode/src/lib.rs:118-186`.

**Executor-from-config signature (D-11):**
```rust
#[cfg(feature = "code-mode")]
pub fn executor_from_config(
    config: &crate::config::ServerConfig,
) -> Result<Box<dyn pmcp_code_mode::CodeExecutor>, crate::error::ToolkitError> {
    // Map config.code_mode (with all aliases) into pmcp_code_mode::CodeModeConfig
    // Build ValidationPipeline + NoopPolicyEvaluator (or AvpPolicyEvaluator if feature=avp)
    // Return Boxed trait object
}
```

**Assembly fn signature (D-12 — TKIT-10):**
```rust
use crate::sql::SqlConnector;

pub fn assemble_code_mode_prompt(
    connector: &dyn SqlConnector,
    config: &crate::config::ServerConfig,
) -> String {
    let dialect = connector.dialect();
    let schema_text = connector.schema_text_sync();  // or async variant; P83 stub returns canned
    let dialect_guidance = dialect.placeholder_guidance();

    let curated_descriptions = config.database.tables
        .iter()
        .map(|t| format!("- `{}`: {}", t.name, t.description))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "# Code Mode — {name}\n\n{guidance}\n\n## Schema\n\n{schema_text}\n\n## Curated Tables\n\n{curated}\n",
        name = dialect.name(),
        guidance = dialect_guidance,
        schema_text = schema_text,
        curated = curated_descriptions,
    )
}
```

**P83 stub `MockSqlConnector` for tests (Claude's-discretion-resolved):**
```rust
#[cfg(test)]
pub(crate) struct MockSqlConnector {
    pub dialect: Dialect,
    pub schema: String,
}

#[async_trait::async_trait]
impl SqlConnector for MockSqlConnector {
    fn dialect(&self) -> Dialect { self.dialect }
    async fn execute(&self, _sql: &str, _params: &[(String, serde_json::Value)])
        -> Result<Vec<serde_json::Value>, /* error */> { Ok(vec![]) }
    async fn schema_text(&self) -> Result<String, /* error */> { Ok(self.schema.clone()) }
}
```

---

### 11. `crates/pmcp-server-toolkit/src/sql/mod.rs` (SqlConnector trait stub + Dialect enum — NET-NEW per spike 005)

**Primary analog:** `src/server/traits.rs:38-58` — `async_trait` + `Send + Sync` + `Result<T>` shape for handler-style traits.
**Trait shape:** verbatim from `.planning/spikes/005-multi-dialect-sql-connector/README.md:147-159`.

**Trait + enum (from spike 005):**
```rust
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait SqlConnector: Send + Sync + 'static {
    fn dialect(&self) -> Dialect;

    async fn execute(
        &self,
        sql_with_named_placeholders: &str,
        named_params: &[(String, Value)],
    ) -> Result<Vec<Value>, ConnectorError>;

    async fn schema_text(&self) -> Result<String, ConnectorError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]                // permit additive Dialect::Oracle / ::SqlServer / ::DuckDb later
pub enum Dialect {
    Postgres,
    MySql,
    Athena,
    Sqlite,
}

impl Dialect {
    pub const fn name(self) -> &'static str { /* match arms */ }
    pub const fn placeholder_guidance(self) -> &'static str { /* match arms */ }
}

// Free helpers — NOT trait methods (spike 005 §"What to Avoid"):
pub fn translate_placeholders(dialect: Dialect, sql: &str) -> String { /* ... */ }
```

**Anti-pattern (RESEARCH §"Anti-Patterns" + spike 005):** **Don't put `translate_placeholders` on the `SqlConnector` trait.** Spike 005 explicitly warns: free helpers prevent per-backend overrides that introduce subtle drift. The translation rule is dialect-pure; method dispatch is the wrong axis of variation.

**Trait-stub-vs-impls discipline (RESEARCH §"Risks + Landmines" #8):** P83 ships **trait + enum + free helpers + MockSqlConnector for tests**. P84 ships per-backend impls (`pmcp-toolkit-postgres`, `-mysql`, `-athena`, plus the `sqlite` feature inside the toolkit). If P83 over-specifies (e.g., adds bound-method translation), P84 will be forced to refactor.

---

### 12. `crates/pmcp-server-toolkit/src/code_mode.rs::assemble_code_mode_prompt`

See §10 above. Pattern reused from `crates/pmcp-code-mode/src/handler.rs` (which composes prompt body templates) and `src/server/skills.rs::SkillPromptHandler` (description + body composition).

---

### 13. `crates/pmcp-server-toolkit/src/builder_ext.rs` (ServerBuilderExt trait — NET-NEW)

**Primary analog:** `src/server/builder.rs:200-272` — `tool_arc` and `prompt_arc` registration methods.
**Secondary analog:** `src/server/builder.rs:294-307` (`resources_arc`) + `:439-445` (`auth_provider_arc`).

**Extension-trait pattern (D-10 + D-11):**
```rust
use pmcp::ServerBuilder;  // public re-export, verified Phase 82
use std::sync::Arc;
use crate::config::ServerConfig;
use crate::tools::synthesize_from_config;

pub trait ServerBuilderExt {
    fn tools_from_config(self, config: &ServerConfig) -> Self;
    fn code_mode_from_config(self, config: &ServerConfig) -> Self;
}

impl ServerBuilderExt for ServerBuilder {
    fn tools_from_config(mut self, config: &ServerConfig) -> Self {
        let synthesized = synthesize_from_config(config)
            .expect("synthesize_from_config: ServerConfig is parse-time-invariant");
        for (name, _info, handler) in synthesized {
            // Phase 82's tool_arc — registers handler;
            // ToolInfo metadata flows through handler.metadata() (see §9 invariant)
            self = self.tool_arc(name, handler);
        }
        self
    }

    fn code_mode_from_config(mut self, config: &ServerConfig) -> Self {
        #[cfg(feature = "code-mode")]
        {
            // Build CodeExecutor + register validate_code/execute_code tools
            // See code_mode.rs::executor_from_config + register_code_mode_tools
        }
        self
    }
}
```

**Critical Phase 82 dependency verification** (per RESEARCH §"Pitfall 3"):
- `pmcp::ServerBuilder::tool_arc` — `src/server/builder.rs:203`
- `pmcp::ServerBuilder::prompt_arc` — `src/server/builder.rs:254`
- `pmcp::ServerBuilder::resources_arc` — `src/server/builder.rs:294`
- `pmcp::ServerBuilder::auth_provider_arc` — `src/server/builder.rs:442`
- `pmcp::Server::get_tool` / `get_prompt` — re-export of inner accessors (Phase 82 BLDR-04)

**Anti-pattern (RESEARCH §"Anti-Patterns"):** **Don't write a 20-line delegating wrapper around `ServerCoreBuilder::tool_arc`.** Phase 82 made `tool_arc` public on `pmcp::ServerBuilder` specifically to eliminate this paper-cut.

---

### 14. `crates/pmcp-server-toolkit/tests/fixtures/*.toml` (reference config snapshots)

**Primary analog:** None in workspace (reference configs live in external pmcp-run repo).
**Role-match analog:** `crates/pmcp-code-mode/tests/property_tests.rs` uses inline TOML fixtures via `CodeModeConfig::from_toml(toml_literal)`.

**Pattern:** Copy `open-images/config.toml`, `imdb/config.toml`, `msr-vtt/config.toml` from operator-supplied snapshots (per RESEARCH §"Assumptions Log" A4 — MEDIUM risk; planner should include a "fetch snapshots" task before declaring `config.rs` done).

**Critical:** Symlinks don't work cross-platform on Windows (per RESEARCH §"Recommended Project Structure" comment). Copy as physical files into `crates/pmcp-server-toolkit/tests/fixtures/`.

---

### 15. `crates/pmcp-server-toolkit/tests/code_mode_policy.rs` (integration test)

**Primary analog:** `crates/pmcp-code-mode/tests/property_tests.rs:13-100` — proptest-driven HMAC round-trip + policy invariants.

**Integration test pattern (from `crates/pmcp-code-mode/tests/property_tests.rs:14-55`):**
```rust
use pmcp_code_mode::{
    HmacTokenGenerator, RiskLevel, TokenGenerator, TokenSecret, ValidationContext,
};
use pmcp_server_toolkit::config::ServerConfig;
use proptest::prelude::*;

proptest! {
    /// [code_mode] allow_writes=false rejects INSERT/UPDATE/DELETE
    #[test]
    fn code_mode_blocks_writes_when_disabled(
        sql in "INSERT INTO foo VALUES \\(1, 2, 3\\)|UPDATE foo SET x = 1|DELETE FROM foo",
    ) {
        let cfg: ServerConfig = toml::from_str(r#"
            [server]
            name = "test"
            version = "0.1.0"

            [code_mode]
            enabled = true
            allow_writes = false
        "#).unwrap();

        let executor = pmcp_server_toolkit::code_mode::executor_from_config(&cfg).unwrap();
        // Build ValidationContext, call validate_code, assert policy rejection
        // ...
    }
}
```

---

### 16. `crates/pmcp-server-toolkit/tests/backend_core_smoke.rs` (D-03 anchor)

**Primary analog:** `tests/in_process_handler_pattern.rs:62-118` — drives a real built `pmcp::Server` through Phase 82's `_arc` methods end-to-end.

**Smoke-test pattern (from `tests/in_process_handler_pattern.rs:62-85`):**
```rust
#![cfg(not(target_arch = "wasm32"))]

use pmcp::{Server, RequestHandlerExtra};
use pmcp_server_toolkit::{
    AuthProvider, SecretsProvider, StaticResourceHandler, StaticPromptHandler,
    ServerBuilderExt, config::ServerConfig,
};
use std::sync::Arc;

#[tokio::test]
async fn backend_core_construction_surface_smoke() {
    // Replays mcp-sql/graphql/openapi-core construction without cloning pmcp-run repo.
    let cfg = ServerConfig::from_toml(include_str!("fixtures/open-images-config.toml"))
        .expect("parse open-images config");

    let server = Server::builder()
        .name(&cfg.server.name)
        .version(&cfg.server.version)
        .tools_from_config(&cfg)
        .code_mode_from_config(&cfg)
        .resources_arc(Arc::new(StaticResourceHandler::from(&cfg)))
        .prompt_arc("describe_schema", Arc::new(StaticPromptHandler::from(&cfg)))
        .auth_provider_arc(Arc::new(/* StaticAuthProvider or similar */))
        .build()
        .expect("server build");

    // Probe public accessors (Phase 82 BLDR-04)
    assert!(server.get_tool("validate_code").is_some());
    assert!(server.get_prompt("describe_schema").is_some());
}
```

**Why this test stands in for cross-repo verification (D-03):** Each pmcp-run backend core (mcp-sql, mcp-graphql, mcp-openapi) constructs the same handler set from `mcp-server-common`. By round-tripping the same construction surface through the toolkit's public API, P83 proves the toolkit covers the import surface — without making P83 verification depend on cloning the pmcp-run repo in CI.

---

### 17. `crates/pmcp-server-toolkit/fuzz/Cargo.toml` + `fuzz_targets/pmcp_server_toolkit_config_parser.rs`

**Primary analog:** `cargo-pmcp/fuzz/Cargo.toml:1-53` + `cargo-pmcp/fuzz/fuzz_targets/pmcp_config_toml_parser.rs:1-27`. This IS the pattern to mirror — Phase 77 fuzz target shape.

**Fuzz Cargo.toml pattern (from `cargo-pmcp/fuzz/Cargo.toml:1-21, 40-45`):**
```toml
[package]
name = "pmcp-server-toolkit-fuzz"
version = "0.0.0"
publish = false
edition = "2021"

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
pmcp-server-toolkit = { path = ".." }
toml = "1.0"

# Prevent this from interfering with workspace
[workspace]

[[bin]]
name = "pmcp_server_toolkit_config_parser"
path = "fuzz_targets/pmcp_server_toolkit_config_parser.rs"
doc = false
test = false
bench = false
```

**Fuzz target pattern (from `cargo-pmcp/fuzz/fuzz_targets/pmcp_config_toml_parser.rs:1-26`):**
```rust
//! Phase 83 fuzz target: stress the `pmcp-server-toolkit` config parser against
//! arbitrary byte sequences. The parser must not panic on adversarial input —
//! `toml::from_str` either succeeds (valid TOML matching schema) or returns Err.
//!
//! Threat model: T-83-config-DoS (parser DoS — adversarial TOML input panics).
//!
//! Run: `cargo +nightly fuzz run pmcp_server_toolkit_config_parser`
//! Smoke: `cargo +nightly fuzz run pmcp_server_toolkit_config_parser -- -max_total_time=60`
//!
//! On stable, `cargo +nightly check --bin pmcp_server_toolkit_config_parser` from
//! `crates/pmcp-server-toolkit/fuzz/` verifies the target compiles.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    let _: Result<pmcp_server_toolkit::config::ServerConfig, _> =
        toml::from_str(s);
});
```

**Critical (RESEARCH §"Wave 0 Gaps" + §"Phase Requirements"):** Cannot reuse `cargo-pmcp`'s Phase 77 fuzz target — it targets `cargo_pmcp::test_support::configure_config::TargetConfigV1`, a different schema. The toolkit needs its own fuzz target with its own `ServerConfig` type.

---

### 18. `contracts/binding.yaml` extension (RESEARCH §"Open Questions" #1 recommends extend, not new file)

**Primary analog:** `contracts/binding.yaml:1-90` — entries with `contract`, `equation`, `function`, `module_path`, `signature`, `status`, `notes`.

**Append-row pattern (from `contracts/binding.yaml:38-62`):**
```yaml
# === Toolkit Core (Phase 83) ===
- contract: mcp-protocol-sdk-v1.yaml          # or new contracts/toolkit-v1.yaml
  equation: tool_dispatch_integrity
  function: synthesize_from_config
  module_path: pmcp_server_toolkit::tools
  signature: 'pub fn synthesize_from_config(config: &ServerConfig) -> Result<Vec<(String, ToolInfo, Arc<dyn ToolHandler>)>, ToolkitError>'
  status: implemented
  notes: TKIT-07 — turns [[tools]] config entries into pmcp ToolInfo+handler pairs

- contract: mcp-protocol-sdk-v1.yaml
  equation: auth_provider_contract
  function: StaticAuthProvider::validate_request
  module_path: pmcp_server_toolkit::auth
  signature: 'async fn validate_request(&self, header: Option<&str>) -> Result<Option<AuthContext>>'
  status: implemented
  notes: TKIT-02 — impl of pmcp::AuthProvider trait, bearer-token dev/test auth
```

One row per TKIT-02..10 + TEST-02/TEST-03 public symbol. Run `pmat comply check` before commit (CLAUDE.md §"Contract-First Development").

---

## Shared Patterns (cross-cutting)

### Pattern A: `async_trait` for all async handler traits
**Source:** `src/server/traits.rs:1-69`, `src/server/auth/traits.rs:447-474`.
**Apply to:** `auth.rs`, `secrets.rs`, `resources.rs`, `prompts.rs`, `sql/mod.rs`.
```rust
use async_trait::async_trait;

#[async_trait]
pub trait Foo: Send + Sync {
    async fn bar(&self, /* ... */) -> Result<X, E>;
}
```

### Pattern B: `Result<T, ToolkitError>` with `thiserror`
**Source:** `crates/pmcp-code-mode/src/token.rs:150-161` (`TokenDecodeError`).
**Apply to:** `config.rs`, `tools.rs`, `code_mode.rs` (synthesizer + executor errors).
```rust
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ToolkitError {
    #[error("config parse error: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("tool synthesis error: {0}")]
    Synth(String),

    #[error("code-mode wiring error: {0}")]
    CodeMode(String),
}

pub type Result<T> = std::result::Result<T, ToolkitError>;
```
**Why `#[non_exhaustive]` on the public enum:** future-additive evolution without breaking semver (matches `pmcp::types::ToolInfo`, `ToolAnnotations` discipline at `src/types/tools.rs:18, 174`).

### Pattern C: Constructor methods, NOT struct-literal syntax (`#[non_exhaustive]` types)
**Source:** `src/types/tools.rs:18, 174` — `ToolInfo` and `ToolAnnotations` are `#[non_exhaustive]`.
**Apply to:** Every usage of `ToolInfo`, `ToolAnnotations`, `CallToolResult`, `PromptMessage`.
```rust
// CORRECT:
ToolInfo::with_annotations(name, Some(desc), schema, annotations)
ToolAnnotations::new().with_read_only(true).with_idempotent(true)
Content::resource_with_text(uri, body, mime_type)

// WRONG (won't compile from outside the crate):
ToolInfo { name, description: Some(desc), ... }
```

### Pattern D: `IndexMap` (NOT `HashMap`) for ordered config-derived collections
**Source:** `src/server/skills.rs:37-50` rationale.
**Apply to:** `resources.rs`, `prompts.rs`, `tools.rs` (any config-listing-driven map).
```rust
use indexmap::IndexMap;  // workspace dep, version 2.10 + serde feature
```

### Pattern E: Secrecy newtype around all bytes-bearing secrets
**Source:** `crates/pmcp-code-mode/src/token.rs:14-69`.
**Apply to:** `secrets.rs` (every `SecretsProvider::get` return) + any HMAC-key handling.
**Negative trait assertion test required** (per CMSUP-02): assert `TokenSecret` does NOT implement `Debug` / `Clone` / `Serialize` / `Deserialize`.

### Pattern F: `#[serde(deny_unknown_fields)]` on every public config struct
**Source:** `cargo-pmcp/src/commands/configure/config.rs:116, 131, 146, 158`.
**Apply to:** Every named struct in `config.rs`. Per D-13.

### Pattern G: PMAT cognitive-complexity ≤25 per function (Phase 75 CI gate)
**Source:** CLAUDE.md §"CI Quality Gates"; RESEARCH §"Pitfall 5".
**Apply to:** All net-new code, especially `synthesize_from_config`, `executor_from_config`, `assemble_code_mode_prompt`.
**Hard cap:** cog 50 with annotated `// Why:` `#[allow(clippy::cognitive_complexity)]` (per Phase 75 D-03), and ONLY if irreducible.

### Pattern H: Workspace-version dep with `default-features = false`
**Source:** `crates/pmcp-tasks/Cargo.toml:10` + `crates/mcp-tester/Cargo.toml:21`.
**Apply to:** Toolkit's `pmcp` and `pmcp-code-mode` deps.
**Why:** `pmcp` default features pull `logging` (tracing-subscriber); toolkit consumers may not want that transitively (RESEARCH §"Risks + Landmines" #6).

### Pattern I: Attribution header on every lifted file
**Source:** `crates/pmcp-code-mode/src/lib.rs:1-2`.
**Apply to:** `auth.rs`, `secrets.rs`, `resources.rs`, `prompts.rs` (the four lift files).
```rust
// Originated from pmcp-run/built-in/shared/mcp-server-common/src/{file}.rs
// (https://github.com/guyernest/pmcp-run)
// Promoted to rust-mcp-sdk workspace as part of Phase 83 toolkit lift.
```

---

## Anti-Patterns to Avoid

These come from RESEARCH §"Anti-Patterns to Avoid" + §"Risks + Landmines" + workspace-observed patterns to NOT replicate.

1. **Don't redefine `AuthProvider` / `ResourceHandler` / `PromptHandler` traits.** Use `pmcp::*`. Verified at `src/server/auth/traits.rs:450`, `src/server/mod.rs:238,256`.

2. **Don't redefine HMAC token machinery.** Re-export `pmcp_code_mode::{HmacTokenGenerator, TokenSecret, hash_code, canonicalize_code}`. Source: `crates/pmcp-code-mode/src/token.rs:1-200`.

3. **Don't put `translate_placeholders` on `SqlConnector`.** Spike 005 §"What to Avoid" — free helper, not trait method.

4. **Don't loosen `#[serde(deny_unknown_fields)]` for REF-01 superset.** Add fields (with `#[serde(default)]`) or `alias = "..."`. D-13 invariant.

5. **Don't use struct-literal syntax for `#[non_exhaustive]` types** (`ToolInfo`, `ToolAnnotations`, `CallToolResult`, `PromptMessage`). Use constructors.

6. **Don't bake AVP/Cedar specifically into the toolkit core.** Default to `NoopPolicyEvaluator`; AVP via `avp` feature only (re-export from `pmcp-code-mode`).

7. **Don't lift `mcp-server-common`'s test files verbatim.** D-17 calls for new test files specifically targeting toolkit invariants. Old tests had different focus.

8. **Don't write a 20-line delegating wrapper around `tool_arc`.** Phase 82 made it public — call it directly via `ServerBuilderExt`.

9. **Don't reintroduce dropped features.** Per D-14, the toolkit drops `ddb`, `dynamo-config`, `openapi-code-mode`, `js-runtime`, `mcp-code-mode` from `mcp-server-common`. These are pmcp-run-specific or Phase 3 OpenAPI territory.

10. **Don't blanket-allow clippy lints up front.** `crates/pmcp-code-mode/src/lib.rs:6-20` carries 14+ pre-emptive allows from the original lift. Add only what clippy actually flags during P83 — the toolkit should be cleaner than the source.

11. **Don't return `String` or `Vec<u8>` from `SecretsProvider::get`.** Wrap in `TokenSecret` (or equivalent secrecy newtype) at the trait boundary. If the verbatim lift returns raw bytes, refactor the return type at lift time.

12. **Don't reuse `cargo-pmcp`'s Phase 77 fuzz target.** Different schema (`TargetConfigV1` vs `ServerConfig`). The toolkit needs its own fuzz target.

13. **Don't fuse the toolkit with `cargo-pmcp`.** D-09 rationale: toolkit is a runtime library (Lambda zip), `cargo-pmcp` is a CLI binary (clap + AWS account SDKs). They serve different runtime profiles.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `crates/pmcp-server-toolkit/src/secrets.rs` (`SecretsProvider` trait itself) | trait | request-response | No in-workspace `SecretsProvider` trait exists. The lift body comes from external `mcp-server-common/src/secrets.rs` (701 LoC). Apply pattern E (secrecy newtype) to constrain the return type, but the trait shape itself is net-new-to-this-workspace. Property tests + doctests must establish the contract from scratch. |

---

## Metadata

**Analog search scope:**
- `src/` — pmcp root crate (`src/server/`, `src/types/`, `src/server/auth/`)
- `crates/pmcp-code-mode/` — primary re-export source + secrecy newtype model
- `crates/pmcp-tasks/`, `crates/mcp-tester/`, `crates/mcp-preview/` — workspace-version + feature-matrix pattern
- `cargo-pmcp/src/commands/configure/config.rs` — `#[serde(deny_unknown_fields)]` pattern
- `cargo-pmcp/fuzz/` — Phase 77 fuzz target pattern
- `tests/in_process_handler_pattern.rs` — Phase 82 builder smoke-test pattern
- `.planning/spikes/005-multi-dialect-sql-connector/README.md` — verbatim `SqlConnector` trait shape
- `contracts/binding.yaml` — public-API contract entry shape

**Files scanned:** 22 (12 source files + 4 Cargo.toml + 3 test files + 1 fuzz target + 1 contract + 1 spike doc)
**Pattern extraction date:** 2026-05-18

**Confidence:**
- Lift files (`auth.rs`, `secrets.rs`, `resources.rs`, `prompts.rs`): HIGH for impl-side shape (Pattern A/E/I); MEDIUM for verbatim body fidelity (external source not visible in this repo per RESEARCH §"Risks + Landmines" #4 — operator must supply or planner must task an explicit fetch step).
- Net-new files (`config.rs`, `tools.rs`, `code_mode.rs`, `sql/mod.rs`, `builder_ext.rs`): HIGH — every pattern verified against named workspace files with line numbers.
- Cargo.toml + fuzz + test scaffolding: HIGH — analogs are 1:1 match (pmcp-tasks for crate manifest; cargo-pmcp/fuzz for fuzz target; in_process_handler_pattern.rs for smoke test).
