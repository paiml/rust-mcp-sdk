# Phase 83 Plan 06 Preflight — pmcp-code-mode API Notes

**Date:** 2026-05-18
**Purpose:** Capture the exact `pmcp-code-mode` public constructor + registration
APIs BEFORE Plan 06 Task 1 implements wiring. Per Phase 83 review R1 this
eliminates execution-time discovery and prevents the agent from hallucinating
constructor signatures.

All signatures below are copied verbatim from the on-disk source under
`crates/pmcp-code-mode/src/`. Line numbers are anchored against the HEAD this
preflight was written from (HEAD after Plan 05 SUMMARY, commit `c590067a`).

## 1. CodeExecutor construction

### Trait

- Defined at: `crates/pmcp-code-mode/src/code_executor.rs:54-68`
- Always available (no feature gate per D-04 in pmcp-code-mode's lib.rs)
- Shape (verbatim):

```rust
#[async_trait::async_trait]
pub trait CodeExecutor: Send + Sync {
    async fn execute(
        &self,
        code: &str,
        variables: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, ExecutionError>;
}
```

NOTE: the `variables` parameter is `Option<&serde_json::Value>` — NOT
`serde_json::Value` as the Plan 06 `<interfaces>` block guessed. Plan 06 must
match the verified signature.

### Concrete impls shipping in pmcp-code-mode

| Type | Path | Feature gate | Constructor |
|---|---|---|---|
| `JsCodeExecutor<H>` | `code_executor.rs:132-163` | `js-runtime` | `pub fn new(http: H, config: ExecutionConfig) -> Self` (requires `H: HttpExecutor + Clone`) |
| `SdkCodeExecutor<S>` | `code_executor.rs:180-215` | `js-runtime` | `pub fn new(sdk: S, config: ExecutionConfig) -> Self` (requires `S: SdkExecutor + Clone + 'static`) |
| `McpCodeExecutor<M>` | `code_executor.rs:236-269` | `mcp-code-mode` | `pub fn new(mcp: M, config: ExecutionConfig) -> Self` (requires `M: McpExecutor + Clone + 'static`) |

### Can the toolkit construct a CodeExecutor from `&CodeModeConfig` ALONE?

**No.** Every concrete `CodeExecutor` impl requires an injected backend
(`HttpExecutor`, `SdkExecutor`, `McpExecutor`). There is no factory function
in `pmcp-code-mode` that takes only `&CodeModeConfig` and returns
`Box<dyn CodeExecutor>` — by design, because the backend (database pool,
AWS SDK client, MCP router, etc.) lives in the consumer crate, not in
`pmcp-code-mode`.

This is the **R1 split** condition (per Plan 06 must_haves truth #2).

## 2. ValidationPipeline construction

`ValidationPipeline` is parameterised over `<T: TokenGenerator, E: ExplanationGenerator>`
with defaults `<HmacTokenGenerator, TemplateExplanationGenerator>`. The
`pmcp-code-mode` crate provides four public constructors for the
default-parameter form:

```rust
// crates/pmcp-code-mode/src/validation.rs:152-187
impl ValidationPipeline<HmacTokenGenerator, TemplateExplanationGenerator> {
    pub fn new(
        mut config: CodeModeConfig,
        token_secret: impl Into<Vec<u8>>,
    ) -> Result<Self, TokenError> { /* ... */ }

    // crates/pmcp-code-mode/src/validation.rs:189-212
    pub fn from_token_secret(
        config: CodeModeConfig,
        secret: &TokenSecret,
    ) -> Result<Self, TokenError> { /* ... */ }

    // crates/pmcp-code-mode/src/validation.rs:214-250
    pub fn with_policy_evaluator(
        mut config: CodeModeConfig,
        token_secret: impl Into<Vec<u8>>,
        evaluator: Arc<dyn PolicyEvaluator>,
    ) -> Result<Self, TokenError> { /* ... */ }

    // crates/pmcp-code-mode/src/validation.rs:252-267
    pub fn from_token_secret_with_policy(
        config: CodeModeConfig,
        secret: &TokenSecret,
        evaluator: Arc<dyn PolicyEvaluator>,
    ) -> Result<Self, TokenError> { /* ... */ }
}
```

- **Takes `&CodeModeConfig`?** No — takes `CodeModeConfig` by value (moves it into the pipeline).
- **Fallible?** Yes — returns `Result<Self, TokenError>`. The only documented failure is `TokenError::SecretTooShort` when the secret is < 16 bytes.
- **For Plan 06 wiring:** prefer `ValidationPipeline::from_token_secret(config, &secret)` because it takes a `&TokenSecret` reference (avoids a redundant `Vec<u8>` copy) and the toolkit's `SecretValue → TokenSecret` conversion already exists in `secrets.rs:110`.

### SQL validation entry point

`ValidationPipeline::validate_sql_query(sql, &ValidationContext) -> Result<ValidationResult, ValidationError>`
is gated behind `#[cfg(feature = "sql-code-mode")]` in
`crates/pmcp-code-mode/src/validation.rs:935-947`. The full pipeline for the
SC-3 anchor:

1. Parse SQL via `sqlparser` (`validate_sql_preamble`)
2. Run `check_sql_config_authorization(info, start)` which at
   `crates/pmcp-code-mode/src/validation.rs:1084` returns
   `ValidationResult::failure([PolicyViolation { type: "code_mode",
   rule: "writes_disabled", ... }])` when
   `!self.config.sql_allow_writes` and `info.statement_type ==
   SqlStatementType::Insert | Update`.

**Implication for toolkit Cargo.toml:** the toolkit's `code-mode` feature must
forward `pmcp-code-mode/sql-code-mode` so that `validate_sql_query` is
available under just `--features code-mode` (matching the Plan 06 verify
step). Without this, the integration test cannot compile.

## 3. HmacTokenGenerator construction

From `crates/pmcp-code-mode/src/token.rs:184-234`:

```rust
pub struct HmacTokenGenerator {
    secret: TokenSecret,
}

impl HmacTokenGenerator {
    pub const MIN_SECRET_LEN: usize = 16;

    pub fn new(secret: TokenSecret) -> Result<Self, TokenError> { /* ... */ }
    pub fn new_from_bytes(bytes: impl Into<Vec<u8>>) -> Result<Self, TokenError> { /* ... */ }
    pub fn from_env(env_var: &str) -> Result<Self, Box<dyn std::error::Error>> { /* ... */ }
}
```

- Accepts `TokenSecret` directly (NOT `Arc<TokenSecret>` — `TokenSecret`
  intentionally does not implement `Clone`).
- TTL is NOT a constructor parameter — it is per-token, passed to
  `TokenGenerator::generate(.., ttl_seconds: i64)` at signing time
  (`token.rs:174`). `CodeModeConfig.token_ttl_seconds: i64` flows in
  separately.
- Minimum secret length: 16 bytes (HMAC-SHA256 best practice). Plan 06's
  `executor_from_config` will surface a `TokenError::SecretTooShort` as a
  `ToolkitError::CodeMode(_)`.

## 4. Pre-existing handler registration helpers

`pmcp-code-mode` ships **`CodeModeToolBuilder`** (handler.rs:300-383), which
returns `Vec<ToolInfo>` for the validate/execute tools, but does NOT register
them on a `pmcp::ServerBuilder` directly. The trait `CodeModeHandler`
(handler.rs:134) defines the validate/execute methods, but there is no
turnkey `register_code_mode_tools_on(builder, executor, config) -> ServerBuilder`
helper — wiring `CodeModeHandler` into `pmcp::ServerBuilder` is left to the
consumer.

**Decision:** the toolkit will provide `register_code_mode_tools(builder,
config)` as a thin wrapper. For Plan 06 (which is intentionally scoped to the
"re-export + validation pipeline" surface and does NOT depend on Phase 84 SQL
connectors), the helper performs the following on success:

- If `config.code_mode.is_none()` — return `Ok(builder)` unchanged (no-op).
- If `config.code_mode.is_some()` — build a `ValidationPipeline` from config
  (proves the wiring compiles and resolves the secret) and discard it.
  Discarding is acceptable here because Plan 06's deliverable is the
  validation pipeline + the re-exports; **executor registration on the builder
  proper is deferred to Plan 08** (`code_mode_from_config` builder extension)
  per CONTEXT.md D-15 / D-16.

The R9 enforcement (inline-secret rejection) MUST surface from
`register_code_mode_tools` even when `code_mode.is_some()` so a
`code_mode_from_config(&cfg)` consumer cannot bypass the check by going
through the helper.

## 5. Feature-gate considerations

| pmcp-code-mode symbol | Required pmcp-code-mode feature |
|---|---|
| `ValidationPipeline::validate_sql_query` | `sql-code-mode` |
| `ValidationPipeline::validate_javascript_code` | `openapi-code-mode` |
| `JsCodeExecutor` / `SdkCodeExecutor` | `js-runtime` |
| `McpCodeExecutor` | `mcp-code-mode` |
| `AvpClient`, `AvpConfig`, `AvpPolicyEvaluator` | `avp` |
| `CedarPolicyEvaluator` | `cedar` |
| `HmacTokenGenerator`, `TokenSecret`, `ApprovalToken`, `NoopPolicyEvaluator`, `ValidationPipeline::{new, from_token_secret, with_policy_evaluator}` | none (always available) |

Plan 06 needs `sql-code-mode` (for the SC-3 integration test). Therefore the
toolkit's `code-mode` feature must forward it:

```toml
code-mode = ["dep:pmcp-code-mode", "pmcp-code-mode/sql-code-mode"]
```

The existing `avp = ["code-mode", "pmcp-code-mode/avp"]` cascade is unchanged.

## 6. Wiring strategy decision (per review R1)

Based on Sections 1–5 above:

- [ ] **Single-function path:** `executor_from_config(&ServerConfig) -> Result<Box<dyn CodeExecutor>>`. Rejected — Section 1 shows pmcp-code-mode does NOT ship a backend-agnostic CodeExecutor that can be constructed from `&CodeModeConfig` alone. Any single-function helper would either need to hard-code a backend (forcing the toolkit to depend on every connector crate) or stub out execution (violating "no `todo!()` survives").
- [x] **R1 split:** `validation_pipeline_from_config(&ServerConfig) -> Result<ValidationPipeline>` + `code_mode_tools_from_executor(executor: Box<dyn CodeExecutor>, config: &ServerConfig) -> Result<...>`. **Selected.**

**Rationale:** Section 1's CodeExecutor trait requires backend injection; the
toolkit cannot manufacture a backend from config alone without dragging in
Phase 84's SQL connectors (which would invert the dependency graph and force a
circular dep through the deploy story). The R1 split keeps Plan 06's
deliverable focused on the validation pipeline (config-driven, no backend
needed) while leaving the executor-tying surface for callers that already own
a backend. The `code_mode_tools_from_executor` helper exists as a pure-shape
identity-style helper today; Plan 08's `.code_mode_from_config()` builder
extension is where the actual tool registration on `pmcp::ServerBuilder` will
hook in.

## 7. Exact symbols Plan 06 Task 1 will use

From `pmcp_code_mode::*` (re-exported, verbatim):

- `pmcp_code_mode::CodeExecutor` (trait, no gate)
- `pmcp_code_mode::CodeModeConfig` (struct, no gate)
- `pmcp_code_mode::ValidationPipeline` (struct, no gate; SQL methods need `sql-code-mode`)
- `pmcp_code_mode::ValidationContext` (struct, no gate)
- `pmcp_code_mode::TokenSecret` (struct, no gate)
- `pmcp_code_mode::HmacTokenGenerator` (struct, no gate)
- `pmcp_code_mode::TokenGenerator` (trait, no gate)
- `pmcp_code_mode::ApprovalToken` (struct, no gate)
- `pmcp_code_mode::canonicalize_code`, `compute_context_hash`, `hash_code` (fns, no gate)
- `pmcp_code_mode::NoopPolicyEvaluator` (struct, no gate)
- `pmcp_code_mode::PolicyEvaluator` (trait, no gate)
- `pmcp_code_mode::AuthorizationDecision` (struct, no gate)
- `pmcp_code_mode::AvpClient`, `AvpConfig`, `AvpPolicyEvaluator` — re-exported under `#[cfg(feature = "avp")]`

From `pmcp_code_mode::*` used internally by `validation_pipeline_from_config`:

- `pmcp_code_mode::ValidationPipeline::from_token_secret(config, &secret)` — primary constructor call
- `pmcp_code_mode::TokenError` — surfaced into `ToolkitError::CodeMode(_)` via `From`/`map_err`

From the toolkit (cross-module):

- `crate::config::{ServerConfig, CodeModeSection, CodeModeLimits}` (Plan 04)
- `crate::error::{ConfigValidationError, Result, ToolkitError}` (Plan 04 + Plan 06 extends)
- `crate::secrets::SecretValue` (Plan 02, via `From<SecretValue> for pmcp_code_mode::TokenSecret`)

## 8. Toolkit Cargo.toml change required

To keep `cargo test -p pmcp-server-toolkit --features code-mode` working
end-to-end (Plan 06 verify), the `code-mode` feature line must forward
`pmcp-code-mode/sql-code-mode`:

```toml
# Before (current):
code-mode = ["dep:pmcp-code-mode"]

# After (Plan 06):
code-mode = ["dep:pmcp-code-mode", "pmcp-code-mode/sql-code-mode"]
```

Rationale: the SC-3 anchor test `allow_writes_false_rejects_insert` invokes
`ValidationPipeline::validate_sql_query` which is `#[cfg(feature =
"sql-code-mode")]`-gated. Without this forward, the test does not compile.
