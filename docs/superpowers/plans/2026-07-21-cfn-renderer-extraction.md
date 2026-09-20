# CFN Renderer Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A pure `pmcp-cfn-renderer` crate that renders `DeployDescriptor` → deterministic CloudFormation, wired into both CFN targets of cargo-pmcp, dropping the Node/CDK dependency for standard scaffolds.

**Architecture:** New workspace crate with 7 allowlist-scoped resource modules and a `render(descriptor, params)` pure function; semantic-golden tests against checked-in normalized `cdk synth` output are the parity arbiter; cargo-pmcp gains shape-aware artifact acquisition and a small aws-sdk CFN deploy engine; hand-customized stacks keep the legacy cdk path with a `custom_stack` taint.

**Tech Stack:** Rust; `serde`/`serde_json`; `pmcp-package` (descriptor types); CLI side only: `aws-sdk-s3`, `aws-sdk-cloudformation`, existing `toml`.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-21-cfn-renderer-extraction-design.md` — binding.
- `pmcp-cfn-renderer` dependencies are EXACTLY: `pmcp-package`, `serde`, `serde_json`, `semver`. NO aws-sdk, tokio, reqwest, filesystem, or network use anywhere in the crate. `toml` only as a dev-dependency (fixture tests).
- Renderer input is `pmcp_package::package::DeployDescriptor` — never cargo-pmcp's `DeployConfig`. Any missing field is promoted into the descriptor's closed set FIRST (edit `crates/pmcp-package/src/package/server.rs`; the `deny_unknown_fields` fixture test guards it).
- Resource surface is EXACTLY 7 modules: `lambda`, `iam`, `logs`, `http_api`, `cognito`, `dynamodb`, `outputs`. Anything a descriptor requests outside the surface → `RenderError` naming the section/field. Never a silent skip. Never grow toward CDK-completeness.
- Determinism: all maps are `BTreeMap`; no timestamps, no randomness, no absolute paths; `to_canonical_json()` is byte-identical across runs/platforms.
- **The semantic goldens are the parity authority.** Resource-module code in this plan is the starting point; where it disagrees with a normalized golden, the golden wins — fix the module, never the golden (goldens change only by regenerating from cdk with the documented script).
- Secret VALUES never appear in descriptors, params, templates, or goldens.
- Quality: `make quality-gate` before any push; verify complexity with the EXACT CI command `pmat quality-gate --fail-on-violation --checks complexity` (threshold 23 — the capture poll-loop lesson). Zero clippy warnings, `cargo fmt --all`.
- Commit after every task. Branch: `feat/pmcp-cfn-renderer`.

## File Structure

```
crates/pmcp-cfn-renderer/
  Cargo.toml
  src/lib.rs            # render() orchestrator + public re-exports
  src/error.rs          # RenderError (total, names section/field)
  src/params.rs         # RenderParams, ArtifactRef, RenderMetadata
  src/template.rs       # CfnTemplate, CfnResource, to_canonical_json
  src/logical_ids.rs    # stable logical-ID scheme
  src/resources/mod.rs
  src/resources/lambda.rs
  src/resources/iam.rs      # port of cargo-pmcp/src/deployment/iam.rs logic
  src/resources/logs.rs
  src/resources/http_api.rs
  src/resources/cognito.rs
  src/resources/dynamodb.rs
  src/resources/outputs.rs
  tests/goldens/<project>.golden.json   # normalized cdk-synth goldens (checked in)
  tests/semantic_golden.rs              # fixture render vs golden diff
  tests/determinism.rs
  xtask-notes.md -> (golden regeneration script lives in scripts/)
scripts/generate-cfn-goldens.sh          # one-time, needs Node locally, never CI
cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs        # synth switch
cargo-pmcp/src/deployment/targets/aws_lambda/{mod,deploy}.rs # engine switch
cargo-pmcp/src/deployment/targets/aws_lambda/artifact.rs     # NEW: shape-aware acquisition
cargo-pmcp/src/deployment/targets/aws_lambda/engine.rs       # NEW: CFN deploy engine
docs/runbooks/cfn-renderer-switch-gate.md                    # real-deploy gate checklist
```

---

### Task 1: Crate skeleton, core types, canonical JSON, logical IDs

**Files:**
- Create: `crates/pmcp-cfn-renderer/Cargo.toml`, `src/lib.rs`, `src/error.rs`, `src/params.rs`, `src/template.rs`, `src/logical_ids.rs`, `src/resources/mod.rs`, `tests/determinism.rs`
- Modify: root `Cargo.toml` (add `"crates/pmcp-cfn-renderer"` to `[workspace] members`)

**Interfaces:**
- Produces (later tasks rely on these EXACT signatures):
  - `pub fn render(descriptor: &DeployDescriptor, params: &RenderParams) -> Result<CfnTemplate, RenderError>` (lib.rs; in this task returns an empty-resource template)
  - `RenderParams { account_id: String, region: String, stack_name: String, artifact: ArtifactRef, environment: BTreeMap<String,String>, metadata: RenderMetadata }`
  - `ArtifactRef { s3_bucket: String, s3_key: String, digest: Option<String> }`
  - `RenderMetadata { version: String, server_type: Option<String>, server_id: Option<String>, template_id: Option<String>, snapshot_baked: bool }` (mirrors `metadata.rs::to_cdk_context` keys: `mcp:version`, `mcp:serverType`, `mcp:serverId`, `mcp:templateId`)
  - `CfnTemplate { description: String, resources: BTreeMap<String, CfnResource>, outputs: BTreeMap<String, CfnOutput>, metadata: BTreeMap<String, serde_json::Value> }` with `to_canonical_json(&self) -> String`
  - `CfnResource { type_: String ("Type"), properties: serde_json::Value, depends_on: Vec<String> }`
  - `logical_ids::for_function() -> "McpFunction"`, `for_log_group() -> "LogGroup"`, `for_http_api() -> "HttpApi"`, `for_table(name: &str) -> String` (PascalCase + "Table"), etc. — one fn per family, documented, no hashes.

- [ ] **Step 1: Crate manifest + workspace membership**

```toml
# crates/pmcp-cfn-renderer/Cargo.toml
[package]
name = "pmcp-cfn-renderer"
version = "0.1.0"
edition = "2021"
description = "Pure DeployDescriptor -> CloudFormation renderer for PMCP MCP servers"
license = "MIT"
repository = "https://github.com/paiml/rust-mcp-sdk"

[dependencies]
pmcp-package = { version = "0.1", path = "../pmcp-package" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
semver = { version = "1", features = ["serde"] }

[dev-dependencies]
toml = "0.9"
```

Add `"crates/pmcp-cfn-renderer"` to the root `Cargo.toml` workspace `members` array (alphabetical position near the other `crates/pmcp-*`). NOTE: `pmcp-package` is workspace-EXCLUDED (own `[workspace]` table) but path-dep from a member works — `crates/pmcp-agent` already does exactly this; copy its dependency line style.

- [ ] **Step 2: Write the failing determinism test**

```rust
// crates/pmcp-cfn-renderer/tests/determinism.rs
use pmcp_cfn_renderer::{render, ArtifactRef, RenderMetadata, RenderParams};
use std::collections::BTreeMap;

fn minimal_descriptor() -> pmcp_package::package::DeployDescriptor {
    // Smallest tracked-fixture shape: [target] + [aws] + [server] + required tables.
    toml::from_str(
        r#"
        [target]
        type = "aws-lambda"
        version = "1"
        [aws]
        region = "us-east-1"
        [server]
        name = "det-test"
        timeout_seconds = 30
        [auth]
        enabled = false
        provider = ""
        callback_urls = []
        [observability]
        error_threshold = 5
        "#,
    )
    .expect("minimal descriptor parses")
}

fn params() -> RenderParams {
    RenderParams {
        account_id: "123456789012".into(),
        region: "us-east-1".into(),
        stack_name: "det-test-stack".into(),
        artifact: ArtifactRef {
            s3_bucket: "pmcp-deploy-123456789012-us-east-1".into(),
            s3_key: "det-test/bootstrap.zip".into(),
            digest: None,
        },
        environment: BTreeMap::new(),
        metadata: RenderMetadata {
            version: "1.0.0".into(),
            server_type: None,
            server_id: None,
            template_id: None,
            snapshot_baked: false,
        },
    }
}

#[test]
fn render_is_byte_deterministic() {
    let d = minimal_descriptor();
    let p = params();
    let a = render(&d, &p).unwrap().to_canonical_json();
    let b = render(&d, &p).unwrap().to_canonical_json();
    assert_eq!(a, b);
    // canonical: sorted keys — serde_json::Value round-trip of the string
    // must serialize back identically
    let v: serde_json::Value = serde_json::from_str(&a).unwrap();
    assert_eq!(serde_json::to_string_pretty(&v).unwrap(), a);
}
```

NOTE: if the minimal TOML above fails `DeployDescriptor`'s `deny_unknown_fields`/required-field set, fix the TOML by copying the smallest real tracked fixture (`git ls-files '*/.pmcp/deploy.toml'` — pick the shortest) — do NOT loosen the descriptor.

- [ ] **Step 3: Run it — expect compile failure** (`cargo test -p pmcp-cfn-renderer` → unresolved imports).

- [ ] **Step 4: Implement the core types** — `error.rs`:

```rust
use std::fmt;

/// Total, descriptive render failure. NEVER silently skip descriptor content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// Descriptor requests something outside the 7-family allowlist surface.
    UnsupportedSection { section: String, detail: String },
    /// A field required for rendering is absent (promote it into the
    /// descriptor closed set first — see Global Constraints).
    MissingField { section: String, field: String },
    /// Invalid value (message names section.field and the offending value).
    Invalid { section: String, field: String, message: String },
}

impl fmt::Display for RenderError { /* match arms produce
    "unsupported [section]: detail" / "missing [section].field" /
    "invalid [section].field: message" — write them out */ }
impl std::error::Error for RenderError {}
```

`params.rs` and `template.rs` exactly per the Interfaces block above. `to_canonical_json` = `serde_json::to_string_pretty` over a `serde_json::Value` built exclusively from `BTreeMap`s (serde_json `Map` preserves insertion order — insert via BTreeMap iteration so order is sorted; add `#[serde(rename = "Type")]` etc. for CFN casing: `Resources`, `Outputs`, `Description`, `Properties`, `DependsOn`). `logical_ids.rs`: constant fns per Interfaces, plus `fn pascal(name: &str) -> String` (split on `-`/`_`, capitalize; document that renames rename IDs — acceptable under fleet recreation). `lib.rs::render` composes an empty template (Description = `format!("PMCP MCP server: {}", descriptor.server.name)`, empty resources) for now.

- [ ] **Step 5: Test passes** (`cargo test -p pmcp-cfn-renderer` → determinism green). `cargo fmt -p pmcp-cfn-renderer && cargo clippy -p pmcp-cfn-renderer --all-targets -- -D warnings`.

- [ ] **Step 6: Commit** — `feat(cfn-renderer): crate skeleton, core types, canonical JSON, logical-ID scheme`

---

### Task 2: Golden harness — normalizer, generation script, first golden

**Files:**
- Create: `crates/pmcp-cfn-renderer/tests/semantic_golden.rs`, `crates/pmcp-cfn-renderer/tests/normalize.rs` is NOT separate — normalizer lives in `tests/support/mod.rs`; `scripts/generate-cfn-goldens.sh`; `crates/pmcp-cfn-renderer/tests/goldens/README.md`
- Goldens land under `crates/pmcp-cfn-renderer/tests/goldens/<fixture-project-slug>.golden.json`

**Interfaces:**
- Consumes: `render`, `RenderParams` (Task 1).
- Produces: `support::normalize(template_json: &serde_json::Value) -> serde_json::Value` used by all later module tasks; the golden corpus.

**Normalization algorithm (the heart — implement exactly):**

```rust
// tests/support/mod.rs
/// Normalize a CFN template (cdk-synth OR renderer output) into a
/// comparable resource-graph form:
/// 1. DROP: `Resources.CDKMetadata`, `Parameters.BootstrapVersion`,
///    `Rules`, `Conditions` matching /CheckBootstrapVersion/, any
///    `Metadata` keys starting "aws:cdk:" or "cdk_", template-level
///    `Metadata`.
/// 2. DROP per-resource: `Metadata`, `UpdateReplacePolicy`,
///    `DeletionPolicy` when equal to the CFN default for that type.
/// 3. LOGICAL-ID CANONICALIZATION: build a map old-id -> canonical-id where
///    canonical-id = "<Type-suffix>-<n>" (e.g. "Function-0", "Role-0",
///    "Table-0"), assigned by sorting resources by (Type, canonical JSON of
///    Properties with all Ref/GetAtt targets replaced by " "). Then
///    rewrite every {"Ref": old} / {"Fn::GetAtt": [old, attr]} and
///    DependsOn entry to the canonical id. Two-pass fixpoint (sort again
///    after rewrite) — CDK ids and renderer ids both disappear.
/// 4. Sort DependsOn arrays; drop empty ones.
/// 5. Return the value rebuilt through BTreeMap (sorted keys).
pub fn normalize(template: &serde_json::Value) -> serde_json::Value { /* per above */ }
```

- [ ] **Step 1: Write the harness test** (fails: no goldens yet)

```rust
// tests/semantic_golden.rs
mod support;
use std::path::PathBuf;

#[test]
fn all_goldens_match_renderer_output() {
    let golden_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens");
    let mut checked = 0usize;
    for entry in std::fs::read_dir(&golden_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().map(|e| e == "json") != Some(true) { continue; }
        let golden: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        // Each golden file embeds its inputs:
        // { "descriptor_toml": "...", "params": {...}, "normalized": {...} }
        let descriptor: pmcp_package::package::DeployDescriptor =
            toml::from_str(golden["descriptor_toml"].as_str().unwrap()).unwrap();
        let params: pmcp_cfn_renderer::RenderParams =
            serde_json::from_value(golden["params"].clone()).unwrap();
        let rendered = pmcp_cfn_renderer::render(&descriptor, &params).unwrap();
        let rendered_v: serde_json::Value =
            serde_json::from_str(&rendered.to_canonical_json()).unwrap();
        assert_eq!(
            support::normalize(&rendered_v),
            golden["normalized"],
            "golden mismatch: {}", path.display()
        );
        checked += 1;
    }
    assert!(checked >= 1, "no goldens found — run scripts/generate-cfn-goldens.sh");
}
```

(`RenderParams` needs `#[derive(Deserialize)]` — add it in Task 1's types if missed.)

- [ ] **Step 2: Write `scripts/generate-cfn-goldens.sh`** (one-time, LOCAL, needs Node):

```bash
#!/usr/bin/env bash
# Regenerates crates/pmcp-cfn-renderer/tests/goldens/ from cdk synth.
# Requires: node/npx + cdk, jq, cargo. NEVER runs in CI.
# For each tracked */.pmcp/deploy.toml whose project has a deploy/ CDK dir:
#   1. (re)generate stack.ts via the CLI's fail-closed regeneration
#      (cargo run -p cargo-pmcp -- pmcp deploy --dry-run --regenerate-stack …
#       — if no dry-run flag exists, call the synth path per
#       cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs::run_cdk_synth)
#   2. npx cdk synth --quiet -> cdk.out/<Stack>.template.json
#   3. Normalize with the SAME rust normalizer:
#      cargo run -p pmcp-cfn-renderer --example normalize_json < template.json
#      (add examples/normalize_json.rs: stdin JSON -> stdout normalized JSON,
#       sharing tests/support via #[path] include)
#   4. Emit goldens/<slug>.golden.json = {descriptor_toml, params, normalized}
#      with params synthesized as in tests/determinism.rs (fixed account
#      123456789012, region from [aws], stack_name "<server.name>-stack",
#      artifact bucket "pmcp-deploy-123456789012-<region>", key
#      "<server.name>/bootstrap.zip") — normalize() must make cdk's real
#      asset-parameter references comparable: strip cdk asset Parameters and
#      replace their Ref in Function.Code with the literal params bucket/key.
set -euo pipefail
# … implement per the numbered comments above; keep it ~80 lines, jq for JSON assembly.
```

The script author note: fixtures whose projects have no `deploy/` dir or a non-CFN target are SKIPPED with a printed reason — the golden corpus is "every tracked CFN-target fixture," and the printed skip list is committed into `tests/goldens/README.md` (no silent truncation).

- [ ] **Step 3: Implement `support::normalize`** per the algorithm block. Unit-test it inline with a hand-written mini template containing a `CDKMetadata` resource, a `BootstrapVersion` parameter, and two cross-referencing resources — assert drops + canonical IDs + Ref rewrite.

- [ ] **Step 4: Run the generation script locally** (Node required once). Commit the produced goldens for AT LEAST the plain-Lambda fixture now (the full set lands as modules gain coverage — a golden whose families aren't implemented yet will fail; commit those goldens together with their module task instead; README lists which are pending which task).

- [ ] **Step 5: `cargo test -p pmcp-cfn-renderer`** — `all_goldens_match_renderer_output` FAILS against the plain-Lambda golden (renderer emits nothing yet). That failing test is Task 3's TDD driver. Mark it `#[ignore = "until Task 3"]` ONLY if committing mid-task; remove the ignore in Task 3.

- [ ] **Step 6: Commit** — `test(cfn-renderer): semantic-golden harness, normalizer, golden generation script`

---

### Task 3: `lambda` + `logs` + `outputs` modules (plain-Lambda kernel)

**Files:**
- Create: `src/resources/lambda.rs`, `src/resources/logs.rs`, `src/resources/outputs.rs`
- Modify: `src/lib.rs` (render orchestration), `src/resources/mod.rs`

**Interfaces:**
- Consumes: Task 1 types; Task 2 golden (plain-Lambda fixture) as the arbiter.
- Produces: `lambda::render_function(&DeployDescriptor, &RenderParams) -> (String /*logical id*/, CfnResource)`; `logs::render_log_group(fn_name: &str, retention_days: u32) -> (String, CfnResource)`; `outputs::render_outputs(&…) -> BTreeMap<String, CfnOutput>`.

- [ ] **Step 1: The golden IS the failing test** — un-ignore Task 2's harness for the plain fixture; run to see the diff.

- [ ] **Step 2: Implement `lambda.rs`** (starting point; golden wins on any property disagreement):

```rust
use crate::{error::RenderError, logical_ids, params::RenderParams, template::CfnResource};
use pmcp_package::package::DeployDescriptor;
use serde_json::json;

pub fn render_function(
    d: &DeployDescriptor,
    p: &RenderParams,
) -> Result<(String, CfnResource), RenderError> {
    let memory = d.server.memory_mb.unwrap_or(128); // scaffold default — verify vs golden
    let mut properties = json!({
        "FunctionName": d.server.name,
        "Runtime": "provided.al2023",
        "Handler": "bootstrap",
        "Architectures": ["arm64"],
        "MemorySize": memory,
        "Timeout": d.server.timeout_seconds,
        "Code": { "S3Bucket": p.artifact.s3_bucket, "S3Key": p.artifact.s3_key },
        "Role": { "Fn::GetAtt": [logical_ids::for_execution_role(), "Arn"] },
    });
    if !p.environment.is_empty() {
        properties["Environment"] = json!({ "Variables": p.environment });
    }
    Ok((logical_ids::for_function().to_string(),
        CfnResource { type_: "AWS::Lambda::Function".into(), properties, depends_on: vec![] }))
}
```

`logs.rs`: `AWS::Logs::LogGroup` with `LogGroupName: format!("/aws/lambda/{}", fn_name)`, `RetentionInDays: retention_days` (plain scaffold = 7, http/oauth = 30 — confirm split against goldens). `outputs.rs`: CfnOutput entries matching what `cargo-pmcp/src/deployment/outputs.rs` consumers read from `outputs.json` today (endpoint URL, function name — read `load_cdk_outputs` + the scaffold `CfnOutput` names in `commands/deploy/init.rs` and mirror the names EXACTLY; the deploy engine in Task 9 depends on these names). Note: a plain (no-http) fixture may have no URL output — mirror the scaffold. Wire into `lib.rs::render`: always function+logs+outputs; every OTHER descriptor feature present but unimplemented (auth.enabled, iam non-empty, …) must return `RenderError::UnsupportedSection` for now so nothing silently skips (Tasks 4–6 progressively remove these guards).

- [ ] **Step 3: Iterate until the plain-Lambda golden passes.** Every diff = either a module fix (normal) or a normalizer bug (fix in tests/support, re-run script).

- [ ] **Step 4: fmt/clippy/pmat** (`pmat quality-gate --fail-on-violation --checks complexity`).

- [ ] **Step 5: Commit** — `feat(cfn-renderer): lambda/logs/outputs modules — plain-Lambda golden green`

---

### Task 4: `iam` module — port validation + rendering from `deployment/iam.rs`

**Files:**
- Create: `src/resources/iam.rs`
- Modify: `src/lib.rs` (remove the iam UnsupportedSection guard)
- Reference (do not modify): `cargo-pmcp/src/deployment/iam.rs` (1387 lines — the authoritative validation semantics + table/bucket ARN expansion)

**Interfaces:**
- Produces: `iam::render_execution_role(&DeployDescriptor, &RenderParams) -> Result<(String, CfnResource), RenderError>`; `iam::validate(&IamSection) -> Result<Vec<Warning>, RenderError>` where `Warning { code: String, message: String }` (port the existing warning set verbatim — same codes/messages, tests prove it).

- [ ] **Step 1: Port the validation tests first.** Copy the test cases from `cargo-pmcp/src/deployment/iam.rs`'s `#[cfg(test)]` (wildcard-action warnings, invalid table names, fail-closed behavior) into `iam.rs` tests, adapted to descriptor types (`pmcp_package::package::{IamSection, IamStatement, TablePermission, BucketPermission}` — check exact type names in `server.rs` around line 200 and use those). Run → fail.

- [ ] **Step 2: Implement.** The role: `AWS::IAM::Role` with `AssumeRolePolicyDocument` (lambda.amazonaws.com service principal), `ManagedPolicyArns: ["arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"]` (verify vs golden), and `Policies: [{PolicyName: "pmcp-declared", PolicyDocument: {Statement: [...]}}]` where statements come from three sources exactly as `render_iam_block`/`render_table`/`render_bucket`/`render_statement` do today — table sugar expands to the table+index ARNs (`render_table_resources` logic: `arn:aws:dynamodb:{region}:{account}:table/{name}` + `/index/*` when `include_indexes`), bucket sugar to object-level ARNs, raw statements pass through validated. Region/account come from `RenderParams` (the TS version leaves them to CDK — here they're explicit; use `Fn::Sub` ONLY if the golden shows cdk did).

- [ ] **Step 3: Regenerate/commit the iam-carrying fixture goldens; iterate to green.**

- [ ] **Step 4: fmt/clippy/pmat. Commit** — `feat(cfn-renderer): iam module — validation + role rendering ported from stack.ts emission`

---

### Task 5: `http_api` module

**Files:** Create `src/resources/http_api.rs`; modify `src/lib.rs`.

**Interfaces:** `http_api::render(&DeployDescriptor, &RenderParams, function_logical_id: &str) -> Result<Vec<(String, CfnResource)>, RenderError>` returning Api + Integration + Route(s) + Stage; plus the URL output feeding `outputs::render_outputs`.

- [ ] **Step 1:** Commit the http-carrying fixture goldens (generated in Task 2's script run) — harness now fails on them.
- [ ] **Step 2:** Implement: `AWS::ApiGatewayV2::Api` (ProtocolType HTTP), `Integration` (IntegrationType AWS_PROXY, IntegrationUri = function ARN via `Fn::GetAtt`, PayloadFormatVersion "2.0"), `Route`(s) and `Stage` ($default, AutoDeploy true), `AWS::Lambda::Permission` for apigateway invoke — exact route keys and whether a permission resource exists come from the golden; start from the scaffold reading (`init.rs` ~line 844 on) and iterate.
- [ ] **Step 3:** Green, fmt/clippy/pmat, commit — `feat(cfn-renderer): http_api module — http fixture goldens green`

---

### Task 6: `cognito` + `dynamodb` modules (OAuth + DCR family)

**Files:** Create `src/resources/cognito.rs`, `src/resources/dynamodb.rs`; modify `src/lib.rs` (remove last UnsupportedSection guards for `[auth]`).

**Interfaces:** `cognito::render(&DeployDescriptor, &RenderParams) -> Result<Vec<(String, CfnResource)>, RenderError>` (UserPool, UserPoolResourceServer, UserPoolDomain, JWT authorizer wired into http_api routes); `dynamodb::render_table(name: &str, partition_key: (&str, &str)) -> (String, CfnResource)` (used by DCR ClientsTable: pk `client_id` S, BillingMode PAY_PER_REQUEST — verify vs golden; module is also the future `[[resources.dynamodb]]` landing zone).

- [ ] **Step 1:** Commit OAuth/DCR fixture goldens → failing.
- [ ] **Step 2:** Implement per scaffold (`init.rs` cognito + ClientsTable blocks ~line 1536) + golden iteration. `[auth]` fields drive: provider must be the cognito flavor for this module; an `[auth]` provider outside the supported set → `RenderError::Invalid` naming it.
- [ ] **Step 3:** ALL generated goldens now green: `cargo test -p pmcp-cfn-renderer` full pass, no ignores left, README's pending list empty.
- [ ] **Step 4:** fmt/clippy/pmat, commit — `feat(cfn-renderer): cognito + dynamodb modules — full golden corpus green`

---

### Task 7: Switch the `pmcp_run` synth step to the renderer

**Files:**
- Modify: `cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs` (the `run_cdk_synth` + `find_template_file` call sites, ~lines 93–140)
- Modify: `cargo-pmcp/Cargo.toml` (add `pmcp-cfn-renderer = { version = "0.1", path = "../crates/pmcp-cfn-renderer" }`)
- Test: unit tests in `deploy.rs`'s `#[cfg(test)]`

**Interfaces:**
- Consumes: `pmcp_cfn_renderer::{render, RenderParams, ArtifactRef, RenderMetadata}`.
- Produces: `fn synth_template(config: &DeployConfig, metadata: Option<&McpMetadata>) -> Result<SynthOutput>` where `SynthOutput { template_json: String, path: SynthPath }`, `enum SynthPath { Renderer, LegacyCdk { reason: String } }` — Task 10's runbook and the taint recording use `SynthPath`.

- [ ] **Step 1: Routing rule (write the test first).** `synth_template` routes: stack.ts matches the regenerated scaffold (the EXISTING `validate_and_regenerate_stack_ts` machinery already computes this — reuse its comparison, do not reimplement) → `Renderer`; modified → `LegacyCdk` + `eprintln!` warning naming the file + record `custom_stack = true` into the deploy metadata map that already flows to the platform (find where `server_type`/`snapshot_baked` are applied to the request in step 0 of `deploy_to_pmcp_run` and add the field alongside). Unit-test the routing decision with a temp project fixture both ways.
- [ ] **Step 2: Renderer path.** Build the descriptor by `toml::from_str::<DeployDescriptor>(&fs::read_to_string(project_root.join(".pmcp/deploy.toml"))?)` — NOT from `DeployConfig`. Build `RenderParams`: account/region from the existing config/credential plumbing in this file; `artifact` = the S3-ish key the platform flow already names for the bootstrap upload (read `read_bootstrap_upload`/upload-URL step and pass the same key; the platform rewrites Code server-side today — keep byte-parity with what cdk emitted for these fields per the golden). `metadata` from `McpMetadata` fields 1:1. Replace `run_cdk_synth` + `find_template_file` with `render(...)?.to_canonical_json()`; keep the upload flow byte-for-byte unchanged after that point.
- [ ] **Step 3:** `cargo test -p cargo-pmcp` (routing tests + existing suite green, `--test-threads=1` if flaky per repo note). fmt/clippy/pmat.
- [ ] **Step 4:** Commit — `feat(deploy): pmcp_run synth via pmcp-cfn-renderer (legacy cdk retained for custom stacks)`. NOTE in the commit body: **ship-gate = platform validator acceptance check (cross-team, tracked in the runbook — Task 10).**

---

### Task 8: Shape-aware artifact acquisition (`aws_lambda`)

**Files:**
- Create: `cargo-pmcp/src/deployment/targets/aws_lambda/artifact.rs`
- Modify: `cargo-pmcp/src/deployment/targets/aws_lambda/mod.rs` (`is_available`, wiring)

**Interfaces:**
- Produces: `pub enum ServerShape { BuiltIn { server_type: String }, CustomRust }`, `pub fn detect_shape(config: &DeployConfig) -> ServerShape` — rule: `[metadata].server_type` present → `BuiltIn`; else `CustomRust` (project has `Cargo.toml` + `src/`; if NEITHER marker → error telling the user which to add). `pub async fn acquire_artifact(shape: &ServerShape, config: &DeployConfig) -> Result<PathBuf /*zip*/>`.

- [ ] **Step 1: TDD `detect_shape`** with two temp-dir fixtures (metadata-bearing toml vs src/ project).
- [ ] **Step 2: Built-in fetch.** Verify the release asset URL format FIRST: read `.github/workflows/release-binary.yml`'s upload step and one real v0.19.0 release asset name (`gh release view v0.19.0 --repo paiml/rust-mcp-sdk --json assets -q '.assets[].name'`); implement `fn release_asset_url(version: &str, target_triple: &str) -> String` against the REAL observed format (do not guess). Download to `~/.pmcp/binaries/<name>-<version>-<triple>` cache, verify `ArtifactRef.digest` when present (sha256 the file), zip as `bootstrap` + bundle the project's config/schema files per `snapshot_baked` (true → bake configs into the zip beside bootstrap, mirroring what the built-in binary expects — check the pmcp-sql-server/workbook-server config-loading docs in `crates/pmcp-server-toolkit` README for the expected in-zip layout; false → env-var pointing at runtime-fetched config, exactly what the scaffold does today). Uses `reqwest` (already a cargo-pmcp dep) — this is CLI code, purity rules don't apply here.
- [ ] **Step 3: Custom path** delegates to the existing cargo-lambda build (find it in `DeployExecutor`/`build` flow, call the same function).
- [ ] **Step 4: `is_available()`:** drop the `npx cdk` probe entirely; probe `cargo-lambda` ONLY when `detect_shape == CustomRust`. Update `check_dependencies`'s missing-tool messages accordingly.
- [ ] **Step 5:** fmt/clippy/pmat; commit — `feat(deploy): shape-aware artifact acquisition — built-in servers need zero dev tooling`

---

### Task 9: CFN deploy engine (`aws_lambda` switch)

**Files:**
- Create: `cargo-pmcp/src/deployment/targets/aws_lambda/engine.rs`
- Modify: `aws_lambda/deploy.rs` (route: unmodified scaffold → engine; custom stack.ts → legacy `DeployExecutor` + taint warning — SAME routing helper as Task 7, lift it to `deployment/mod.rs` if sharing needs it), `cargo-pmcp/Cargo.toml` (`aws-sdk-cloudformation`, `aws-sdk-s3`, `aws-config` — pin to the workspace's existing aws-sdk versions if any crate already uses them; else latest stable)

**Interfaces:**
- Produces: `pub async fn deploy_stack(template_json: &str, params: &EngineParams) -> Result<DeploymentOutputs>` with `EngineParams { stack_name, region, artifact_zip: PathBuf, bucket: String }`.
- Bucket convention (no config field exists — this defines it): `pmcp-deploy-{account_id}-{region}`, `create_bucket` if missing (private, versioning off), documented in the runbook.

- [ ] **Step 1: Engine flow (write as small fns, cog ≤ 23 each — the poll-loop lesson):** resolve account via `aws_sdk_sts::get_caller_identity` (add `aws-sdk-sts`); ensure bucket; `put_object` the zip to `{server}/bootstrap-{sha256-prefix}.zip`; `create_stack` or `update_stack` (detect via `describe_stacks`; treat `No updates are to be performed` as success); poll `describe_stacks` every 5s to terminal (CREATE_COMPLETE/UPDATE_COMPLETE ok; ROLLBACK/FAILED → fetch last 10 `describe_stack_events` failures and bail with them); read stack Outputs into `DeploymentOutputs { url, additional_urls: vec![], regions: vec![region], stack_name, version: None, custom }` using the output names Task 3 fixed; ALSO write `deploy/outputs.json` in the same shape `load_cdk_outputs` reads today (compat for `status`/other consumers — mirror its parsing, see `deployment/outputs.rs:27`).
- [ ] **Step 2: Tests.** Unit-test the decision helpers (update-vs-create classification from mocked describe output shapes, event-message extraction) — NO live-AWS tests in CI (repo rule: no Docker/cloud in default harnesses); the live path is covered by the Task 10 real-deploy gate.
- [ ] **Step 3:** Wire `deploy_aws_lambda`: renderer (`render` with params from config + `detect_shape` artifact) → `deploy_stack`; legacy branch unchanged. fmt/clippy/pmat (watch cog on the poller — extract helpers exactly like capture.rs's `fetch_*_once`/`classify_*`).
- [ ] **Step 4:** Commit — `feat(deploy): CFN deploy engine — aws-lambda deploys without Node/CDK`

---

### Task 10: Runbook, publish wiring, quality gate

**Files:**
- Create: `docs/runbooks/cfn-renderer-switch-gate.md`
- Modify: `.github/workflows/release.yml` (publish `pmcp-cfn-renderer` after `pmcp-package`, before `cargo-pmcp` — copy an existing publish-step block verbatim and adjust), `CLAUDE.md` (publish-order list: insert as a new item between `pmcp-package` (13) and `pmcp-agent` (14) with a one-line dependency note), `cargo-pmcp/CHANGELOG.md` (Unreleased/0.20 entry)

- [ ] **Step 1: Write the runbook** — the real-deploy gate as a checklist: one dev-account deploy + `mcp-tester` E2E per family — (a) plain Lambda, (b) OAuth/Cognito, (c) DynamoDB-carrying (DCR), (d) widget-carrying (exercises post-deploy widget upload alongside the engine); each row records fixture used, stack name, date, result. Plus: the **platform-validator acceptance check** (pmcp.run confirms renderer-shaped plain-CFN templates pass their allowlist — REQUIRED before the Task 7 path ships to users), and the bucket convention + `custom_stack` taint semantics (CLI warns + records; platform MAY block server-side — CLI never enforces platform policy).
- [ ] **Step 2: Publish wiring** per Files above.
- [ ] **Step 3: Full gate:** `make quality-gate` && `pmat quality-gate --fail-on-violation --checks complexity` → both clean.
- [ ] **Step 4: Commit** — `docs(renderer): switch-gate runbook + release wiring for pmcp-cfn-renderer`

---

## Self-Review

- **Spec coverage:** §3 crate/API → T1; §4 surface → T3–T6 (7 modules; loud errors from T3's guards); §5 determinism → T1 (+goldens); §6a → T7; §6b engine + shape-aware artifacts → T8–T9; §6c fallback/taint → T7+T9 (shared routing); §7 verification → T2 (goldens) + T9.2 (unit) + T10 (real-deploy gate); §8 exclusions honored (no [[resources.*]], no endpoint flip, non-CFN targets untouched, legacy retained). Field-promotion rule: stated in Global Constraints; no promotion expected for v1 (memory/timeout/binary/auth/iam all exist).
- **Placeholders:** the two deliberate "verify against reality" steps (release asset format, in-zip config layout) name the exact file/command to consult — they are verification steps, not TBDs. Golden-wins iteration is the designed methodology, not underspecification.
- **Type consistency:** `RenderParams`/`ArtifactRef`/`CfnTemplate` identical across T1/T2/T7; `SynthPath` produced T7, consumed T10; output names fixed T3, consumed T9; `detect_shape` produced T8, consumed T9.
