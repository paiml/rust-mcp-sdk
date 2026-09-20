# Phase 76: cargo-pmcp IAM declarations — Pattern Map

**Mapped:** 2026-04-22
**Files analyzed:** 13 (4 modified, 9 created)
**Analogs found:** 13 / 13
**Scope:** cargo-pmcp crate only (leaf workspace member). All paths below are absolute.

## File Classification

| File (new/modified) | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `cargo-pmcp/src/deployment/config.rs` (MODIFY) | config schema (serde struct hierarchy) | parse-TOML / roundtrip | `AssetsConfig` / `CompositionConfig` / `DcrConfig` in same file | exact (in-file precedent) |
| `cargo-pmcp/src/commands/deploy/init.rs` (MODIFY) | template generator (`format!` → TS string) | string-emit | the two `format!` blocks at lines 492 & 649 (self-reference) | exact |
| `cargo-pmcp/src/commands/validate.rs` (MODIFY) | CLI subcommand variant | request-response (CLI) | existing `ValidateCommand::Workflows` variant in same file | exact (extend enum) |
| `cargo-pmcp/src/commands/deploy/deploy.rs` (MODIFY) | runtime validator hook | request-response (CLI) | `DeployExecutor::execute` body; early `DeployConfig::load` at line 39 | exact |
| `cargo-pmcp/src/deployment/iam.rs` (CREATE) | translation helper (config → TS strings) | transform | the inline `format!` emissions in `init.rs:594-613` | role-match; no dedicated module precedent |
| `cargo-pmcp/tests/iam_config.rs` (CREATE) | integration test (serde roundtrip) | parse/serialize | `tests/property_tests.rs` serde pattern (no plain non-proptest serde test file exists) | role-match |
| `cargo-pmcp/tests/iam_stack_ts_integration.rs` (CREATE) | integration test (render + grep) | string-search | no existing integration test renders `init.rs`; closest is `tests/auth_integration.rs` (in-process API + tempdir) | role-match |
| `cargo-pmcp/tests/iam_translation_props.rs` (CREATE) | property tests (proptest) | property-based | `tests/property_tests.rs` | exact |
| `cargo-pmcp/tests/iam_validate.rs` (CREATE) | unit tests per rule | assertion | `tests/auth_integration.rs` simple `#[test]` functions | role-match |
| `cargo-pmcp/tests/deploy_validate_gate.rs` (CREATE) | integration test (CLI variant) | request-response | `tests/auth_integration.rs` (exercises library APIs with tempdir) | role-match |
| `cargo-pmcp/tests/backward_compat_stack_ts.rs` (CREATE) | golden-file test (byte compare) | string-compare | no existing golden test in crate; closest is `tests/auth_integration.rs` fixture style | partial |
| `cargo-pmcp/fuzz/fuzz_targets/fuzz_iam_config.rs` (CREATE) | libfuzzer target | byte-input → parser | `fuzz/fuzz_targets/fuzz_config_parse.rs` | exact |
| `cargo-pmcp/examples/deploy_with_iam.rs` (CREATE) | runnable example | demonstrate | `examples/secrets_local_workflow.rs` | exact |

---

## Pattern Assignments

### 1. `cargo-pmcp/src/deployment/config.rs` (MODIFY — add `IamConfig`, `TablePermission`, `BucketPermission`, `IamStatement`)

**Analog:** `cargo-pmcp/src/deployment/config.rs` itself — use the `AssetsConfig` and `DcrConfig` precedents.

**Field wiring pattern** (how to add `iam` field to `DeployConfig`, mirror lines 18-24):
```rust
/// Assets configuration for bundling files with deployment
#[serde(default)]
pub assets: AssetsConfig,

/// Composition configuration for server-to-server communication
#[serde(default)]
pub composition: CompositionConfig,
```
→ New field: `#[serde(default, skip_serializing_if = "IamConfig::is_empty")] pub iam: IamConfig,` (the `skip_serializing_if` enforces D-05 backward-compat).

**Sub-struct derive pattern** (mirror lines 348-369 `AssetsConfig`):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetsConfig {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_dir: Option<String>,
}
```

**Named default-fn convention** (lines 65-67, 127-129, 167-181, 222-224, 254-256):
```rust
fn default_tier() -> String { "foundation".to_string() }
fn default_true() -> bool { true }
fn default_oauth_provider() -> String { "none".to_string() }
```
→ Apply to any non-trivial default (e.g., `fn default_include_indexes() -> bool { false }` if needed; or prefer `#[serde(default)]` on `bool` which gives `false` for free).

**Hand-written `Default` impl pattern** (lines 371-383 `AssetsConfig`, 294-307 `CognitoConfig`, 696-705 `DcrConfig`, 707-716 `CompositionConfig`):
```rust
impl Default for AssetsConfig {
    fn default() -> Self {
        Self {
            include: vec![],
            exclude: vec![...],
            base_dir: None,
        }
    }
}
```
→ Required for `IamConfig::default()`. Also add `pub fn is_empty(&self) -> bool { self.tables.is_empty() && self.buckets.is_empty() && self.statements.is_empty() }` next to it (mirrors `AssetsConfig::has_assets` at line 396).

**`default_for_server` extension point** (line 546 `assets: AssetsConfig::default()`):
```rust
assets: AssetsConfig::default(),
composition: CompositionConfig::default(),
project_root,
```
→ Insert `iam: IamConfig::default(),` before `project_root`.

**Key conventions to carry over:**
- `#[derive(Debug, Clone, Serialize, Deserialize)]` on every sub-struct (no `PartialEq`, no `Eq`, no `Default` via derive when fields have non-trivial defaults).
- `#[serde(rename_all = "camelCase")]` is NOT used in this file — TOML keys are snake_case literal, matches CR's `include_indexes` naming.
- Use `#[serde(skip_serializing_if = "Option::is_none")]` on every `Option<T>`.
- Use `#[serde(default)]` on every `Vec<T>`, `HashMap<K,V>`, `bool`-with-default-false.
- No `PartialEq` derive anywhere in this file → property-test roundtrips must compare via `toml::to_string` or explicit field-by-field.

**Divergences expected:**
- Add an `is_empty(&self) -> bool` method (no analog in this file has it; `AssetsConfig::has_assets` is the closest inverse).
- `IamStatement::effect` is a bounded string (`"Allow"`/`"Deny"`) — consider a custom `Deserialize` or validate at validator layer (Wave 4). CR does not require an enum type; string is simplest and matches existing `provider: String` convention.

---

### 2. `cargo-pmcp/src/commands/deploy/init.rs` (MODIFY — `McpRoleArn` CfnOutput + iam-statement emission; D-03 import fix)

**Analog:** this file itself — use the existing output emissions and the two `addToRolePolicy` blocks.

**`CfnOutput` emission pattern** (pmcp-run branch, lines 615-637; aws-lambda branch, lines 728-742):
```typescript
// Outputs
new cdk.CfnOutput(this, 'LambdaArn', {{
  value: mcpFunction.functionArn,
  description: 'MCP Server Lambda ARN',
}});
```
→ Add before the closing `}}` of the outputs section in BOTH branches:
```typescript
new cdk.CfnOutput(this, 'McpRoleArn', {{
  value: mcpFunction.role!.roleArn,
  description: 'MCP Server Lambda execution role ARN (stable export for downstream stacks)',
  exportName: `pmcp-${{serverId}}-McpRoleArn`,
}});
```
Note on the aws-lambda branch: the local is `serverName` (line 660) not `serverId`. Use `serverName` there. Confirms D-01 export-name shape.

**`addToRolePolicy` pattern to mirror** (lines 594-604):
```rust
    // 1. Read from DynamoDB McpServer table to discover foundation servers
    mcpFunction.addToRolePolicy(new iam.PolicyStatement({{
      effect: iam.Effect.ALLOW,
      actions: [
        'dynamodb:GetItem',
        'dynamodb:Query',
      ],
      resources: [
        `arn:aws:dynamodb:${{this.region}}:${{this.account}}:table/${{mcpServersTable}}`,
        `arn:aws:dynamodb:${{this.region}}:${{this.account}}:table/${{mcpServersTable}}/*`,
      ],
    }}));
```
→ New iam block renders into a single `String` via a helper in `iam.rs` (file #5) and is injected via a new positional placeholder `{}` in both `format!` blocks.

**Missing import (D-03) — aws-lambda branch imports** (lines 650-654):
```typescript
import * as cdk from 'aws-cdk-lib';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as apigatewayv2 from 'aws-cdk-lib/aws-apigatewayv2';
import * as logs from 'aws-cdk-lib/aws-logs';
import {{ Construct }} from 'constructs';
```
→ Add `import * as iam from 'aws-cdk-lib/aws-iam';` to match the pmcp-run branch (line 496). Always emit, regardless of whether `[iam]` is populated — TypeScript tolerates unused imports at module scope.

**Key conventions to carry over:**
- Braces inside `format!` strings are escaped `{{`/`}}`; variables use positional `{}` with the `format!` argument list at the end.
- 4-space indentation inside the TypeScript constructor body. Helper output from `iam.rs` MUST emit TS indented 4 spaces to line up with existing statements.
- Multi-line raw strings use `r#"..."#` delimiters.
- Two `format!` branches — Part 1 and Part 2 edits must update BOTH in sync; Wave 1 golden-file test pins both.

**Divergences expected:**
- Introduce a new positional placeholder `{iam_block}` (or keep positional `{}` to match the existing style — recommended, since the file uses positional throughout).
- `mcpFunction.role!` uses the TS non-null assertion — this is new to the file (no prior use).
- `exportName` parameter to `CfnOutput` is new to the file (existing outputs omit it).

---

### 3. `cargo-pmcp/src/commands/validate.rs` (MODIFY — add `ValidateCommand::Deploy`)

**Analog:** same file — extend the enum and dispatch.

**Enum variant pattern** (lines 13-28):
```rust
#[derive(Subcommand)]
pub enum ValidateCommand {
    /// Validate all workflows in the project
    Workflows {
        #[arg(long)]
        generate: bool,
        #[arg(long)]
        server: Option<String>,
    },
}
```
→ Add:
```rust
/// Validate the deployment config (.pmcp/deploy.toml), focusing on IAM footguns.
Deploy {
    #[arg(long)]
    server: Option<String>,
},
```

**Dispatch pattern** (lines 30-38):
```rust
impl ValidateCommand {
    pub fn execute(self, global_flags: &crate::commands::GlobalFlags) -> Result<()> {
        match self {
            ValidateCommand::Workflows { generate, server } => {
                validate_workflows(generate, global_flags.verbose, server)
            },
        }
    }
}
```
→ Add arm: `ValidateCommand::Deploy { server } => validate_deploy(server, global_flags.verbose),`

**Console styling** (lines 44-47):
```rust
println!("\n{}", style("PMCP Workflow Validation").cyan().bold());
println!("{}", style("━".repeat(50)).dim());
```
→ Mirror for the Deploy validator output heading.

**Key conventions to carry over:**
- `use console::style;` already at top of file — reuse for warning/error coloring.
- `anyhow::{Context, Result}` error type.
- `PMCP_QUIET` env var gates println output (line 42) — respect it in the Deploy path.
- Free functions (not methods) for the per-variant handler (`validate_workflows`); mirror with `validate_deploy`.

**Divergences expected:**
- The Deploy handler does NOT shell out to `cargo` — it calls library code (`DeployConfig::load` + new `iam::validate`).
- No test-scaffolding generation branch (unlike `--generate` on Workflows).

---

### 4. `cargo-pmcp/src/commands/deploy/deploy.rs` (MODIFY — invoke validator after `DeployConfig::load`, per D-04)

**Analog:** this file itself.

**Insertion point pattern** (lines 33-42):
```rust
pub fn execute(&self) -> Result<()> {
    let start = Instant::now();

    println!("🚀 Deploying to AWS Lambda...");
    println!();

    let config = crate::deployment::config::DeployConfig::load(&self.project_root)?;
    println!("📋 Server: {}", config.server.name);
    println!("🌍 Region: {}", config.aws.region);
    println!();
```
→ Immediately after line 39 (`let config = ...?;`) insert:
```rust
crate::deployment::iam::validate(&config.iam)
    .context("IAM validation failed — fix .pmcp/deploy.toml before deploying")?;
```
(Warnings printed inside `validate`; hard errors bubble up via `?`.)

**Unit-test convention** (lines 115-138):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_env_default_empty() {
        let executor = DeployExecutor::new(PathBuf::from("/tmp"));
        assert!(executor.extra_env.is_empty());
    }
}
```
→ Mirror style for any new in-file test covering the validate-before-deploy gate.

**Key conventions to carry over:**
- `anyhow::{bail, Context, Result}` — use `.context("...")` on the validator call.
- Emoji-led `println!` status lines (🚀 📋 🌍 ☁️  ✅ ❌).

**Divergences expected:**
- Adds a new cross-module dependency on `crate::deployment::iam::validate` (file #5).

---

### 5. `cargo-pmcp/src/deployment/iam.rs` (CREATE — translation + validation)

**Analog:** no dedicated module exists. Closest shape: the inline `addToRolePolicy` emissions in `init.rs:594-613` and the regex-based secret-ref parsing that already imports `regex = "1"`.

**Module entry pattern** (mirror `cargo-pmcp/src/deployment/mod.rs`):
```rust
pub mod builder;
pub mod config;
pub mod metadata;
pub mod naming;
pub mod operations;
pub mod outputs;
pub mod registry;
pub mod targets;
pub mod r#trait;

pub use builder::BinaryBuilder;
pub use config::DeployConfig;
```
→ Add `pub mod iam;` and consider re-exporting `pub use iam::{render_iam_block, validate};` for use from `init.rs` and `deploy.rs`.

**Error-returning function pattern** (mirror `AssetsConfig::resolve_files` at `config.rs:401-424`):
```rust
pub fn resolve_files(&self, project_root: &Path) -> Result<Vec<PathBuf>> {
    ...
    for pattern in &self.include {
        ...
    }
    Ok(files)
}
```
→ `pub fn validate(iam: &IamConfig) -> Result<()>` follows the same shape.

**String-builder for TS emission** (mirror the `format!` style from `init.rs:594-613`):
```rust
mcpFunction.addToRolePolicy(new iam.PolicyStatement({
  effect: iam.Effect.ALLOW,
  actions: [...],
  resources: [...],
}));
```
→ Helper `pub fn render_iam_block(iam: &IamConfig) -> String` builds a `String` via `use std::fmt::Write` and returns `""` when `iam.is_empty()` (enforces D-05 byte-identity for backward compat).

**Key conventions to carry over:**
- File-level `//!` doc comment (see `config.rs`, `deploy.rs` headers).
- `use anyhow::{Context, Result};` at top.
- Public API `pub fn`, module-private helpers plain `fn`.
- `#[cfg(test)] mod tests { ... }` at bottom with unit tests for each translation row.
- Use `regex::Regex` compiled once via `std::sync::OnceLock` (not `lazy_static`, not `once_cell` — neither is a current dep). Actually, the crate already uses `regex = "1"` — confirm a `OnceLock` pattern is idiomatic; if not, compile per-call (validator is not hot).

**Divergences expected:**
- No direct analog for the translation-rule dispatch; design is:
  ```rust
  fn render_table(p: &TablePermission) -> String { ... } // dynamodb actions per D-02
  fn render_bucket(p: &BucketPermission) -> String { ... }
  fn render_statement(s: &IamStatement) -> String { ... } // passthrough
  ```
- Warnings (unknown service prefix, cross-account ARN) print to stderr via `eprintln!` with `console::style("warning:").yellow()` to match `validate.rs` style. Hard errors return `Err`.
- Ordering: `tables → buckets → statements` (locked in RESEARCH.md Q4 for golden-file stability).

---

### 6. `cargo-pmcp/tests/iam_config.rs` (CREATE — serde roundtrip unit tests)

**Analog:** `cargo-pmcp/tests/property_tests.rs` (proptest-based; use for the TOML-literal roundtrip pattern at lines 50-80) combined with the plain `#[test]` style of `tests/auth_integration.rs:16-31`.

**Plain `#[test]` + `use cargo_pmcp::...` import pattern** (mirror `tests/auth_integration.rs:10-14`):
```rust
use cargo_pmcp::test_support::cache::{
    default_multi_cache_path, is_near_expiry, normalize_cache_key, TokenCacheEntry, TokenCacheV1,
    REFRESH_WINDOW_SECS,
};
```
→ Use:
```rust
use cargo_pmcp::deployment::config::{DeployConfig, IamConfig, TablePermission, BucketPermission, IamStatement};
```

**TOML-literal roundtrip pattern** (mirror `tests/property_tests.rs:59-72`):
```rust
let toml_str = format!(
    r#"[settings]
virtual_users = {virtual_users}
...
[[scenario]]
type = "tools/call"
weight = {weight}
tool = "{tool_name}"
"#
);
let config = LoadTestConfig::from_toml(&toml_str).unwrap();
prop_assert_eq!(config.settings.virtual_users, virtual_users);
```
→ Build cost-coach-shaped TOML strings inline, call `toml::from_str::<DeployConfig>(...)`, and `assert_eq!` on the `iam` field's shape.

**Key conventions to carry over:**
- File lives in `tests/` (integration test, separate crate boundary — public API only).
- No `#[cfg(test)]` needed at file level — `tests/*.rs` is already cfg-test.
- Snake-case `fn` names, `#[test]` attribute.
- `.unwrap()` is acceptable in test code (matches `property_tests.rs:73`).

**Divergences expected:**
- Use `assert_eq!` (not `prop_assert_eq!`) — these are plain unit tests, not proptest.
- Cover the key D-05 invariant: a `DeployConfig` with `iam: IamConfig::default()` serialized via `toml::to_string_pretty` does NOT contain an `[iam]` table.

---

### 7. `cargo-pmcp/tests/iam_stack_ts_integration.rs` (CREATE — renders `create_stack_ts`, greps emitted TS)

**Analog:** `tests/auth_integration.rs` (uses `tempfile::tempdir()` and exercises library APIs end-to-end). No existing test renders `init.rs` output, so this is a new pattern shape.

**Tempdir + library-call pattern** (mirror `tests/auth_integration.rs:33-56`):
```rust
#[tokio::test]
async fn cache_roundtrip_via_write_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".pmcp").join("oauth-cache.json");
    let mut c = TokenCacheV1::empty();
    ...
    c.write_atomic(&path).unwrap();
    let back = TokenCacheV1::read(&path).unwrap();
    assert_eq!(...);
}
```
→ Apply:
```rust
#[test]
fn pmcp_run_emits_mcp_role_arn_output() {
    let dir = tempfile::tempdir().unwrap();
    let init = InitCommand::new(dir.path().to_path_buf())
        .with_target_type("pmcp-run");
    init.create_stack_ts(&dir.path().join("deploy"), "demo-server").unwrap();
    let stack_ts = std::fs::read_to_string(dir.path().join("deploy/lib/stack.ts")).unwrap();
    assert!(stack_ts.contains("McpRoleArn"));
    assert!(stack_ts.contains("exportName: `pmcp-${serverId}-McpRoleArn`"));
}
```

**Key conventions to carry over:**
- `tempfile = "3"` is in `[dev-dependencies]` per existing usage — no new dep needed.
- Test function names describe the asserted invariant (mirror `tests/property_tests.rs`: `prop_valid_config_roundtrip`, `prop_empty_scenario_always_fails_validation`).
- String-contains assertions acceptable for structural checks (CR does not require byte-equality here — that's file #11).

**Divergences expected:**
- `InitCommand::create_stack_ts` is currently a private method (inspect `pub fn` vs `fn`). If private, Wave 1 must either (a) make it `pub(crate)` + test in-crate (i.e., move test to `#[cfg(test)]` at bottom of `init.rs`), or (b) add a narrow `pub fn render_stack_ts(&self, server_name: &str) -> String` helper so tests can call it without touching the filesystem. Recommended: (b) — cleaner separation and gives the golden-file test (file #11) the same hook.

---

### 8. `cargo-pmcp/tests/iam_translation_props.rs` (CREATE — proptest for translation rules)

**Analog:** `cargo-pmcp/tests/property_tests.rs`.

**`arb_*` Strategy functions** (lines 15-42):
```rust
fn arb_settings() -> impl Strategy<Value = Settings> {
    (1u32..=1000, 1u64..=3600, 100u64..=30000, 1u64..=500).prop_map(
        |(virtual_users, duration_secs, timeout_ms, expected_interval_ms)| Settings {
            virtual_users,
            duration_secs,
            timeout_ms,
            expected_interval_ms,
            request_interval_ms: None,
        },
    )
}

fn arb_scenario_step_zero_weight() -> impl Strategy<Value = ScenarioStep> {
    prop_oneof![
        "[a-z]{1,20}".prop_map(|tool| ScenarioStep::ToolCall { ... }),
        "file:///[a-z]{1,20}".prop_map(|uri| ScenarioStep::ResourceRead { ... }),
    ]
}
```
→ Port to `arb_valid_iam_config()` and `arb_invalid_iam_config()` per RESEARCH.md Q5.

**`proptest!` block** (lines 44-80):
```rust
proptest! {
    #[test]
    fn prop_valid_config_roundtrip(
        virtual_users in 1u32..=1000,
        ...
    ) {
        let toml_str = format!(...);
        let config = LoadTestConfig::from_toml(&toml_str).unwrap();
        prop_assert_eq!(config.settings.virtual_users, virtual_users);
    }

    #[test]
    fn prop_empty_scenario_always_fails_validation(settings in arb_settings()) {
        ...
        prop_assert!(result.is_err());
    }
}
```
→ Target properties:
- arb_valid_iam → `toml::to_string` → `toml::from_str` produces structurally equal IamConfig.
- arb_valid_iam → `render_iam_block` → balanced braces, contains one `addToRolePolicy` per statement.
- arb_invalid_allow_star → `validate` returns `Err`.

**Key conventions to carry over:**
- `use proptest::prelude::*;` at top.
- `prop_assert_eq!` / `prop_assert!` inside `proptest!` blocks.
- Bounded ranges (`1u32..=1000`) prevent OOM/slowdown — no unbounded `proptest::num::*` for Vec sizes.
- Regex literals `"[a-z]{1,20}"` as string-strategy shorthand.

**Divergences expected:**
- Sugar-keyword strategies use `prop_oneof![Just("read"), Just("write"), Just("readwrite")]` (Q5) — not present in property_tests.rs.
- IamConfig has no `PartialEq` derive → roundtrip check compares `toml::to_string` output, not `prop_assert_eq!(config, parsed)`.

---

### 9. `cargo-pmcp/tests/iam_validate.rs` (CREATE — unit tests per validation rule)

**Analog:** `tests/auth_integration.rs` (simple `#[test]` + positive/negative assertions).

**Per-rule test shape** (mirror `tests/auth_integration.rs:17-31`):
```rust
#[test]
fn normalize_covers_url_edge_cases() {
    assert_eq!(
        normalize_cache_key("HTTPS://API.Example.Com/").unwrap(),
        "https://api.example.com"
    );
    assert_eq!(
        normalize_cache_key("https://api.example.com:443").unwrap(),
        "https://api.example.com"
    );
}
```
→ One `#[test]` fn per row in the Validation Rules table (RESEARCH.md §"Validation Rules Reference Table"):
- `validate_rejects_allow_star_star`
- `validate_rejects_unknown_effect`
- `validate_rejects_empty_actions`
- `validate_rejects_empty_resources`
- `validate_rejects_malformed_action`
- `validate_rejects_invalid_sugar_keyword`
- `validate_warns_unknown_service_prefix`
- `validate_warns_cross_account_arn`

**Key conventions to carry over:**
- `use cargo_pmcp::deployment::iam::validate;` (public API).
- `anyhow::Result` in test sigs is acceptable but not required — `assert!(result.is_err())` on the returned `Result<()>` is fine.
- Group positive cases (valid → `Ok`) and negative cases (invalid → specific `Err`) in distinct tests for granular failure reporting.

**Divergences expected:**
- Warning-returning rules need an out-of-band capture (either (a) return `Result<Vec<Warning>>` instead of `Result<()>` so tests inspect warnings directly, or (b) capture stderr — prefer (a); cleaner API, matches the pattern in RESEARCH.md §Q8).

---

### 10. `cargo-pmcp/tests/deploy_validate_gate.rs` (CREATE — integration test for `ValidateCommand::Deploy`)

**Analog:** `tests/auth_integration.rs` for the tempdir + file-fixture shape.

**Fixture-file + execute pattern** (mirror `tests/auth_integration.rs:33-56` + `tests/property_tests.rs:59-72`):
```rust
let dir = tempfile::tempdir().unwrap();
let pmcp_dir = dir.path().join(".pmcp");
std::fs::create_dir_all(&pmcp_dir).unwrap();
std::fs::write(pmcp_dir.join("deploy.toml"), BAD_IAM_TOML).unwrap();
let cmd = ValidateCommand::Deploy { server: Some(dir.path().to_string_lossy().into()) };
let result = cmd.execute(&GlobalFlags::default());
assert!(result.is_err());
```

Where `BAD_IAM_TOML` is a const `&str` with an Allow-star-star violation.

**Key conventions to carry over:**
- Tempdir + `.pmcp/deploy.toml` path — matches `DeployConfig::load`'s expectations (`config.rs:444`).
- Golden TOML fixtures as `const &str` at top of file (no external fixture files — keeps Wave 4 self-contained).

**Divergences expected:**
- `ValidateCommand::execute` currently takes `&crate::commands::GlobalFlags` — inspect whether `GlobalFlags` is publicly constructable from integration tests. If not, Wave 4 must expose `pub fn validate_deploy_inner(server: Option<String>, verbose: bool) -> Result<()>` as the library entry point and exercise that.

---

### 11. `cargo-pmcp/tests/backward_compat_stack_ts.rs` (CREATE — golden-file byte-compare)

**Analog:** no existing golden-file test in the crate. Closest shape: `tests/auth_integration.rs` with a tempdir.

**Pattern to establish** (novel in this crate):
```rust
// tests/golden/pmcp-run-empty.ts is checked in; regenerated by running
// UPDATE_GOLDEN=1 cargo test -p cargo-pmcp --test backward_compat_stack_ts
const GOLDEN_PMCP_RUN_EMPTY: &str = include_str!("golden/pmcp-run-empty.ts");

#[test]
fn pmcp_run_stack_ts_is_byte_identical_when_iam_empty() {
    let init = InitCommand::new(PathBuf::from("/tmp"))
        .with_target_type("pmcp-run");
    // Hypothetical hook added in Wave 1 — see file #7 divergence note.
    let ts = init.render_stack_ts_for_test("demo-server", &IamConfig::default());
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::write("tests/golden/pmcp-run-empty.ts", &ts).unwrap();
    }
    assert_eq!(ts, GOLDEN_PMCP_RUN_EMPTY, "stack.ts drift with empty [iam]");
}
```

**Key conventions to carry over:**
- Use `include_str!` to embed the golden (compile-time check that the file exists).
- Environment-variable escape hatch (`UPDATE_GOLDEN=1`) is a widely-used Rust idiom for snapshot regeneration — new to this crate.
- One test per (target_type, fixture_shape) combination: `pmcp-run-empty`, `pmcp-run-cost-coach`, `aws-lambda-empty`.

**Divergences expected:**
- Requires the Wave 1 / Wave 3 hook in `init.rs` to expose a pure-string renderer (see file #7 divergence note). Without it, the test is forced to go through the filesystem — workable but uglier.
- Creates a new `cargo-pmcp/tests/golden/` directory — establishes a convention.

---

### 12. `cargo-pmcp/fuzz/fuzz_targets/fuzz_iam_config.rs` (CREATE — libfuzzer target)

**Analog:** `cargo-pmcp/fuzz/fuzz_targets/fuzz_config_parse.rs` (verbatim template).

**Full template to mirror** (all 19 lines):
```rust
//! Fuzz target for TOML config parsing.
//!
//! Feeds arbitrary byte sequences to `LoadTestConfig::from_toml()` and
//! verifies it never panics.
//!
//! Run with: `cargo +nightly fuzz run fuzz_config_parse`

#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = cargo_pmcp::loadtest::config::LoadTestConfig::from_toml(s);
    }
});
```
→ New target:
```rust
//! Fuzz target for IAM config TOML parsing, via DeployConfig.
//!
//! Run with: `cargo +nightly fuzz run fuzz_iam_config`

#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _: Result<cargo_pmcp::deployment::config::DeployConfig, _> = toml::from_str(s);
    }
});
```

**`fuzz/Cargo.toml` registration** (mirror lines 18-31):
```toml
[[bin]]
name = "fuzz_config_parse"
path = "fuzz_targets/fuzz_config_parse.rs"
doc = false
```
→ Append analogous `[[bin]]` entry for `fuzz_iam_config`.

**Key conventions to carry over:**
- `#![no_main]` at top.
- `libfuzzer_sys::fuzz_target!` macro.
- UTF-8 guard (non-UTF-8 bytes aren't meaningful TOML).
- `toml` dep already in `fuzz/Cargo.toml:14` — no new deps.

**Divergences expected:**
- Parse entry is `toml::from_str::<DeployConfig>` (not a library method like `LoadTestConfig::from_toml`) since no such wrapper exists for `DeployConfig`. RESEARCH.md A7 notes the option of adding `DeployConfig::load_from_str` — if added, use it here.

---

### 13. `cargo-pmcp/examples/deploy_with_iam.rs` (CREATE — runnable example)

**Analog:** `cargo-pmcp/examples/secrets_local_workflow.rs` (pure stdout demo, no AWS calls, runnable on any machine).

**Header + main shape** (mirror `examples/secrets_local_workflow.rs:1-18`):
```rust
//! Example: Local Secrets Management Workflow
//!
//! This example demonstrates how to use the local secrets provider
//! for development workflows with cargo-pmcp.
//!
//! Run with: cargo run -p cargo-pmcp --example secrets_local_workflow

use std::path::PathBuf;

fn main() {
    println!("=== Local Secrets Management Workflow ===\n");
    ...
}
```

**Key conventions to carry over:**
- `//!` module doc at top with "Run with:" line.
- Plain `fn main()` (no `anyhow::Result` return) when the example cannot fail — OR `fn main() -> anyhow::Result<()>` when calling fallible APIs (see `examples/secrets_provider_demo.rs` for the Result variant).
- Section separators: `println!("=== Section ===\n");` style.
- Inline TOML shown via triple-quoted `r#"..."#` or line-by-line `println!` (see `secrets_local_workflow.rs:74-82`).

**Divergences expected:**
- Example loads a real cost-coach-shaped TOML (embedded via `include_str!` or inline literal) and calls:
  - `toml::from_str::<DeployConfig>(...)` (or new `DeployConfig::load_from_str` per RESEARCH A7).
  - `cargo_pmcp::deployment::iam::validate(&config.iam)` (may print warnings).
  - `cargo_pmcp::deployment::iam::render_iam_block(&config.iam)` and print the resulting TS.
- Demonstrates one valid-and-accepted config AND one deliberately-invalid config (Allow-star-star) to show the validator rejecting it.
- Must exit 0 when run — do NOT `.unwrap()` the invalid case's `Err`; match on it and print the error message.

---

## Shared Patterns

### S1. Serde struct conventions

**Source:** `cargo-pmcp/src/deployment/config.rs` (whole file)
**Apply to:** Files #1, #6, #8, #13.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FooConfig {
    #[serde(default)]
    pub vec_field: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<String>,
    #[serde(default = "default_fn")]
    pub scalar_with_default: String,
}

impl Default for FooConfig { fn default() -> Self { ... } }
```

No `PartialEq` derive anywhere → roundtrip tests use `toml::to_string` comparison, not struct equality.

### S2. Module-level doc comment + `use anyhow::{Context, Result}`

**Source:** `cargo-pmcp/src/commands/validate.rs:1-11`, `cargo-pmcp/src/commands/deploy/deploy.rs:1-5`
**Apply to:** Files #3, #4, #5.

```rust
//! Workflow validation command
//!
//! Validates workflow definitions in the project by:
//! 1. Running `cargo check` to ensure compilation
//! ...

use anyhow::{Context, Result};
```

### S3. Console-styled output for CLI commands

**Source:** `cargo-pmcp/src/commands/validate.rs:10, 45-47, 93-101`
**Apply to:** Files #3, #5 (CLI-facing surfaces only).

```rust
use console::style;

println!("\n{}", style("PMCP Workflow Validation").cyan().bold());
println!("{}", style("━".repeat(50)).dim());
println!("  {} Compilation failed.", style("✗").red());
println!("  {} Compilation successful", style("✓").green());
eprintln!("  {} Unknown service prefix: '{}'", style("warning:").yellow(), prefix);
```

`PMCP_QUIET` env var gates non-essential output (line 42) — respect it.

### S4. Proptest file structure

**Source:** `cargo-pmcp/tests/property_tests.rs`
**Apply to:** File #8.

```rust
use proptest::prelude::*;

fn arb_foo() -> impl Strategy<Value = Foo> { ... }

proptest! {
    #[test]
    fn prop_invariant_name(field in strategy) {
        prop_assert_eq!(...);
    }
}
```

### S5. Libfuzzer target skeleton

**Source:** `cargo-pmcp/fuzz/fuzz_targets/fuzz_config_parse.rs`
**Apply to:** File #12.

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = /* parse entry */;
    }
});
```
Plus `[[bin]]` registration in `fuzz/Cargo.toml`.

### S6. Public API re-exports via `mod.rs`

**Source:** `cargo-pmcp/src/deployment/mod.rs:11-17`
**Apply to:** File #5 (add `pub mod iam;` and selective `pub use iam::{render_iam_block, validate};`).

---

## No Analog Found

None — every file has at least a role-match in the existing crate. File #11 (golden-file test) establishes a new convention but the mechanical pattern (include_str + UPDATE_GOLDEN escape hatch) is a widely-used Rust idiom with no in-crate precedent needed.

---

## Metadata

**Analog search scope:**
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/deployment/`
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/`
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/tests/`
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/fuzz/`
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/examples/`

**Files scanned (fully or targeted):**
- `cargo-pmcp/src/deployment/config.rs` (717 lines, full)
- `cargo-pmcp/src/deployment/mod.rs` (17 lines, full)
- `cargo-pmcp/src/commands/deploy/init.rs` (lines 1-60, 480-750 targeted)
- `cargo-pmcp/src/commands/deploy/deploy.rs` (140 lines, full)
- `cargo-pmcp/src/commands/validate.rs` (500 lines, full)
- `cargo-pmcp/tests/property_tests.rs` (182 lines, full)
- `cargo-pmcp/tests/auth_integration.rs` (first 80 lines)
- `cargo-pmcp/fuzz/fuzz_targets/fuzz_config_parse.rs` (19 lines, full)
- `cargo-pmcp/fuzz/Cargo.toml` (32 lines, full)
- `cargo-pmcp/examples/secrets_local_workflow.rs` (87 lines, full)

**Pattern extraction date:** 2026-04-22

## PATTERN MAPPING COMPLETE
