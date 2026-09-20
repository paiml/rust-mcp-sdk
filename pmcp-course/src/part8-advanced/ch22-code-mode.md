# Code Mode: Validated LLM Code Execution

Code Mode is PMCP's framework for safely executing LLM-generated code (GraphQL queries, JavaScript plans, SQL statements, MCP tool compositions) with cryptographic approval tokens. PMCP ships Code Mode as two cooperating crates: `pmcp-code-mode` (runtime: validation pipeline, token signing, executor traits) and `pmcp-code-mode-derive` (the `#[derive(CodeMode)]` macro that wires it all into your server struct). The canonical entry point is the derive macro — every modern PMCP server reaches for `#[derive(CodeMode)]` first. This chapter walks one worked example end-to-end: `examples/s41_code_mode_graphql.rs`. By the end, you will know how to add Code Mode to any server, configure the validation policy for your target language, and trace a validated query from `validate_code` through HMAC token mint to `execute_code`.

## Learning Objectives

By the end of this chapter, you will be able to:

- Add Code Mode to any MCP server using `#[derive(CodeMode)]` with the four required field names (`code_mode_config`, `token_secret`, `policy_evaluator`, `code_executor`).
- Configure operation policies for your target system (GraphQL, JS/OpenAPI, SQL, MCP) via the `language` attribute on the derive macro.
- Declare operations in `config.toml` for platform-level policy management via `cargo pmcp deploy`.
- Choose between a direct `CodeExecutor` impl and the three standard adapters (`JsCodeExecutor`, `SdkCodeExecutor`, `McpCodeExecutor`) based on your backend pattern.
- Trace a validated query from `validate_code` → HMAC token → `execute_code`, and explain why mutations are rejected before token generation under default config.
- Apply the three layers of policy: runtime (`CodeModeConfig`), platform (`config.toml`), authorization (`PolicyEvaluator`).
- Identify when `NoopPolicyEvaluator` is appropriate (local development only) and when production must replace it.

## Why Code Mode Matters for Enterprise MCP

When an LLM generates code to run against your backend, three questions arise: **Is it safe?** Does it touch sensitive data or modify production state? **Is it authorized?** Does this user have permission for this operation? **Is it the same code the user approved?** Can we prove the executed code is byte-equal to what was reviewed?

Without Code Mode, every code-generating MCP server hand-rolls validation, token generation, and policy enforcement. Enterprise deployments need three properties that hand-rolled solutions usually fail to provide consistently:

1. **Consistent security boundaries** across many servers — a fleet of GraphQL, SQL, JS, and MCP-composition servers should share the same security primitives, not each invent its own.
2. **Auditable policy decisions** tied to user and session — when an operation is denied, you need to know who, what, when, and why.
3. **Tamper-proof binding between approved code and executed code** — once a user has approved a specific query, the LLM (or a man-in-the-middle) cannot quietly substitute a different one.

Code Mode delivers all three through an HMAC-SHA256 approval token. The token binds together: code hash, user ID, session ID, server ID, context hash, risk level, and expiry. Any modification to the code after validation invalidates the token. Tokens are stateless (no server-side store), so the system scales horizontally without coordination.

## How Code Mode Works (Pipeline Overview)

```text
LLM generates code
        │
        ▼
validate_code(code) ──────► ValidationPipeline
        │                     ├── Parse (language-specific)
        │                     ├── Security scan
        │                     ├── Policy evaluation (Cedar/AVP/custom)
        │                     ├── Explain (human-readable)
        │                     └── HMAC-sign
        ▼
approval_token ◄──────────── (code hash + user + session + expiry)
        │
User reviews explanation, approves
        │
        ▼
execute_code(code, token) ─► Token verification
        │                     ├── Signature check
        │                     ├── Code hash match
        │                     ├── Expiry check
        │                     └── Context match
        ▼
CodeExecutor::execute(code) → Result
```

Each pipeline stage is a separate, observable step. You can intercept any of them by implementing the corresponding trait — that is how custom `PolicyEvaluator` implementations slot in between security scanning and token signing. The HMAC-SHA256 token binding is the load-bearing security property: any change to the code after validation produces a hash mismatch at execute time, so the token cannot be reused for a tampered query.

## Adding Code Mode with `#[derive(CodeMode)]`

These are the five mechanical steps that take an existing PMCP server and add Code Mode to it. The canonical path uses the derive macro — manual handler registration exists as an escape hatch for advanced cases, but you should not start there.

### Step 1: Add Dependencies

```toml
[dependencies]
pmcp = { version = "2.7.0", features = ["full"] }
pmcp-code-mode = "0.5.1"
pmcp-code-mode-derive = "0.3.0"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
serde_json = "1"
```

### Step 2: Derive and Configure

Define a server struct with the four required field names and annotate it with `#[derive(CodeMode)]`:

```rust,ignore
use pmcp_code_mode::{
    CodeExecutor, CodeModeConfig, ExecutionError, NoopPolicyEvaluator, TokenSecret,
    ValidationContext, ValidationPipeline,
};
use pmcp_code_mode_derive::CodeMode;
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(CodeMode)]
struct MyGraphQLServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<GraphQLExecutor>,
}
```

The four field names are a convention (v0.1.0 of the derive crate): renaming any of them breaks compilation. The macro generates an inherent method, `register_code_mode_tools(builder)`, that takes a `ServerCoreBuilder` and returns it with the `validate_code` and `execute_code` tools registered.

The default `language` is `"graphql"`. Override it for other languages:

```rust,ignore
#[derive(CodeMode)]
#[code_mode(language = "javascript")]
struct MyJavaScriptServer { /* same four fields */ }
```

### Step 3: Pick a Language

The `language` attribute selects the validation path at compile time:

| Language           | Attribute value       | Feature flag required |
| ------------------ | --------------------- | --------------------- |
| GraphQL            | `"graphql"` (default) | *(none)*              |
| JavaScript/OpenAPI | `"javascript"`        | `openapi-code-mode`   |
| SQL                | `"sql"`               | `sql-code-mode`       |
| MCP composition    | `"mcp"`               | `mcp-code-mode`       |

GraphQL is the default because it is the most-deployed Code Mode language and needs no extra feature flag. The other three are opt-in to keep compile times sharp for servers that don't need them.

### Step 4: Choose Your Executor

The `code_executor` field is `Arc<impl CodeExecutor>`. You have two options:

**Option A — Direct implementation** when your backend pattern is unusual (e.g., GraphQL against a specific in-process schema, SQL against a domain-specific database wrapper):

```rust,ignore
struct GraphQLExecutor;

#[pmcp_code_mode::async_trait]
impl CodeExecutor for GraphQLExecutor {
    async fn execute(
        &self,
        code: &str,
        _variables: Option<&Value>,
    ) -> Result<Value, ExecutionError> {
        // In production, this would execute the GraphQL query against a real backend.
        // For this example, return mock data based on the query.
        Ok(json!({
            "data": {
                "query": code,
                "result": [
                    {"id": "1", "name": "Alice"},
                    {"id": "2", "name": "Bob"},
                ]
            }
        }))
    }
}
```

**Option B — Standard adapter** when your backend matches one of the three common patterns:

<!-- synthetic -->
```rust,ignore
// JS + HTTP — Cost Coach calling REST APIs
let executor = Arc::new(JsCodeExecutor::new(http_client, ExecutionConfig::default()));

// JS + SDK — direct AWS SDK calls
let executor = Arc::new(SdkCodeExecutor::new(sdk_client, ExecutionConfig::default()));

// MCP tool composition — routing to other MCP servers
let executor = Arc::new(McpCodeExecutor::new(mcp_router, ExecutionConfig::default()));
```

Choose the adapter that matches your backend pattern. Choose a direct impl only when none of the three adapters fit.

### Step 5: Register on the Builder and Build

```rust,ignore
#[tokio::main]
async fn main() {
    let server = MyGraphQLServer {
        code_mode_config: CodeModeConfig::enabled(),
        token_secret: TokenSecret::new(b"example-secret-key-32-bytes!!!!".to_vec()),
        policy_evaluator: Arc::new(NoopPolicyEvaluator::new()),
        code_executor: Arc::new(GraphQLExecutor),
    };

    let builder = pmcp::Server::builder();
    let _builder = server
        .register_code_mode_tools(builder)
        .expect("Failed to register code mode tools");
    println!("Registered validate_code and execute_code tools on builder.");
    // In a real server, add more tools, then `.build()` and `.run_stdio()`.
}
```

After `register_code_mode_tools(builder)`, the builder exposes `validate_code` and `execute_code` as standard MCP tools. Clients call them via `tools/call` like any other tool.

## Worked Example: GraphQL Round-Trip (`s41_code_mode_graphql.rs`)

The worked example in `examples/s41_code_mode_graphql.rs` runs both the success path and the rejection path in a single binary so you can see both behaviors in one run.

Full example: [`examples/s41_code_mode_graphql.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s41_code_mode_graphql.rs).

### Success path — a valid GraphQL query

```rust,ignore
// --- SUCCESS PATH ---
println!("--- Success Path: Valid GraphQL Query ---");
let query = "query { users { id name } }";
println!("Query: {query}");

match pipeline.validate_graphql_query(query, &context) {
    Ok(result) => {
        println!("Validation: PASSED (is_valid={})", result.is_valid);
        println!("Risk level: {}", result.risk_level);
        println!(
            "Approval token: {}...",
            result
                .approval_token
                .as_ref()
                .map(|t| &t[..t.len().min(20)])
                .unwrap_or("none")
        );
        // Execute with the approval token
        if result.approval_token.is_some() {
            let exec_result = server.code_executor.execute(query, None).await;
            // ...
        }
    },
    Err(e) => { println!("Validation: FAILED - {e:?}"); },
}
```

The query passes parsing, security scan, and policy evaluation; the pipeline returns a `ValidationResult` whose `approval_token` is `Some(...)`. The server then calls `code_executor.execute(query, None)` with the validated query and observes a mock JSON result. In a real deployment, this is where the token would round-trip through `execute_code` so token verification runs before execution.

### Rejection path — a mutation under default config

```rust,ignore
// --- REJECTION PATH ---
println!("\n--- Rejection Path: Mutation Blocked by Config ---");
let mutation = "mutation { createUser(name: \"evil\") { id } }";
println!("Query: {mutation}");

match pipeline.validate_graphql_query(mutation, &context) {
    Ok(result) => {
        if result.is_valid {
            println!("Validation: PASSED (unexpected for mutation with default config)");
        } else {
            println!("Validation: REJECTED (expected)");
            for violation in &result.violations {
                println!("  Violation: {} - {}", violation.rule, violation.message);
            }
            println!("This demonstrates that mutations do NOT receive an approval token.");
            println!(
                "Approval token present: {}",
                result.approval_token.is_some()
            );
        }
    },
    Err(e) => {
        println!("Validation: REJECTED (expected) - {e:?}");
    },
}
```

This is the load-bearing security property in action. The default `CodeModeConfig::enabled()` sets `allow_mutations: false`, so the mutation is rejected **before** the token is minted. `result.approval_token` is `None`. Because there is no token, there is nothing for the LLM (or an attacker) to forward to `execute_code` — the system fails closed. To allow mutations, you must explicitly opt in by constructing `CodeModeConfig { allow_mutations: true, ..CodeModeConfig::enabled() }`; doing so is a deliberate, auditable policy decision rather than the default.

## Configuration in `config.toml` (Platform-Level Policy)

The `config.toml` file separates "what the runtime allows" (`CodeModeConfig`, set in Rust at server-start time) from "what the platform permits" (declared in `config.toml`, surfaced in the pmcp.run admin UI). Administrators can toggle individual operations on or off without redeploying the server. When you deploy with `cargo pmcp deploy`, the `config.toml` is included in the deploy ZIP and read by the platform to populate the Code Mode policy page.

### OpenAPI example

```toml
[server]
name = "cost-coach"
type = "openapi-api"

[code_mode]
allow_writes = false
allow_deletes = false

[[code_mode.operations]]
name = "getCostAndUsage"
description = "Retrieve AWS cost and usage data"
path = "/ce/GetCostAndUsage"
method = "POST"

[[code_mode.operations]]
name = "deleteBudget"
description = "Delete a budget"
path = "/budgets/DeleteBudget"
method = "POST"
destructive_hint = true
```

### GraphQL example

```toml
[server]
name = "open-images"
type = "graphql-api"

[code_mode]
allow_writes = false

[[code_mode.operations]]
name = "searchImages"
operation_type = "query"

[[code_mode.operations]]
name = "deleteImage"
operation_type = "mutation"
destructive_hint = true
```

### SQL example

```toml
[server]
name = "analytics"
type = "sql"

[code_mode]
allow_writes = true
allow_deletes = false
blocked_tables = ["audit_log", "credentials"]

[database]
[[database.tables]]
name = "orders"
description = "Customer order history"

[[database.tables]]
name = "products"
description = "Product catalog"
```

### Categorization

Operations are sorted into **read**, **write**, **delete**, and **admin** categories based on:

| Server type | Read         | Write              | Delete          |
| ----------- | ------------ | ------------------ | --------------- |
| OpenAPI     | `GET`        | `POST` / `PUT`     | `DELETE`        |
| GraphQL     | `query`      | `mutation` (write) | `mutation` (delete via `destructive_hint`) |
| SQL         | `SELECT`     | `INSERT` / `UPDATE` (gated by `allow_writes`) | `DELETE` (gated by `allow_deletes`) |
| MCP-API     | by annotation| by annotation      | by annotation   |

The `operation_category` field overrides automatic categorization when you need explicit control.

## Policy Evaluation (Cedar / AVP / Custom)

Code Mode supports pluggable per-request authorization through the `PolicyEvaluator` trait. The evaluator runs after parsing and security scanning, **before** the approval token is generated. A denied operation never receives a token; therefore it never reaches `execute_code`. This ordering is security-critical: if policy evaluation happened *after* token mint, an attacker who captured a valid token for one user could replay it from a different user's session.

### Cedar (local, no network)

<!-- synthetic -->
```rust,ignore
// Cedar feature flag: pmcp-code-mode = { version = "0.5.1", features = ["cedar"] }
let evaluator = Arc::new(CedarPolicyEvaluator::new(policy_set));
```

Use Cedar when you want fast, in-process policy decisions and your policies are static or refreshed periodically. Latency is sub-millisecond.

### AWS Verified Permissions (AVP, remote)

<!-- synthetic -->
```rust,ignore
// Provided by the pmcp-code-mode-avp companion crate.
let evaluator = Arc::new(AvpPolicyEvaluator::new(avp_client, policy_store_id));
```

Use AVP when you want centralized policy management across many services and per-request audit trails in CloudTrail.

### Custom

<!-- synthetic -->
```rust,ignore
let evaluator = Arc::new(MyCustomEvaluator::new(authz_client));
```

Implement the `PolicyEvaluator` trait yourself when you have an existing authorization backend (Open Policy Agent, Spicedb, internal RPC, etc.).

## Security Properties Reference

- **Tokens are stateless** — verified via HMAC signature, not looked up in a server-side store. The system scales horizontally without coordination.
- **Default TTL: 5 minutes** — configurable via `CodeModeConfig::token_ttl_seconds`. Choose the shortest TTL your UX can tolerate.
- **`TokenSecret` is `zeroize`-on-drop** — backed by `secrecy::SecretBox` so the secret is wiped from memory when the server shuts down.
- **Minimum 16-byte secret** — enforced at `TokenSecret::new` construction time. Too-short secrets produce an error (`Result`), not a panic. Production deployments should use 32+ bytes.
- **Code canonicalization** prevents whitespace-based bypass — `"query { x }"` and `"query{x}"` produce identical hashes.

## Hands-On (Run the Example)

Three follow-on exercises in [Chapter 22 Exercises](./ch22-exercises.md) give you hands-on practice with the concepts above. Before you start them, run the worked example end-to-end:

```bash
cargo run --example s41_code_mode_graphql --features full
```

What to observe in the output:

- The success path prints `Validation: PASSED`, a risk level, an approval token prefix (first 20 characters), and a mock GraphQL result with two users.
- The rejection path prints `Validation: REJECTED (expected)` for the mutation, lists the violations, and confirms no approval token was issued (`Approval token present: false`).
- The example demonstrates BOTH paths in a single binary run, so you can compare them side by side.

Modify-and-experiment tips:

- Change `code_mode_config: CodeModeConfig::enabled()` to `code_mode_config: CodeModeConfig { allow_mutations: true, ..CodeModeConfig::enabled() }` and re-run. The mutation now passes validation and produces a token.
- Shorten the TTL by adding `token_ttl_seconds: 5` to the config; you will see a normal pass on validate, but if you wait six seconds before calling `execute_code` the token will be rejected as expired.
- Try modifying the query string between `validate_code` and `execute_code` (in a real client flow). The HMAC code-hash mismatch rejects the modified query — this is the load-bearing tamper-protection property.

Next: [Chapter 22 Exercises](./ch22-exercises.md).

## Knowledge Check

Use these questions to check your understanding before moving to the exercises.

1. **HMAC binding.** What does the HMAC approval token bind to, and why does this prevent the LLM from re-using a previous token for a modified query? (Hint: the binding is over more than just the code hash — list everything that goes into the HMAC and explain how each component prevents a different attack.)

2. **Required field names.** Why does `#[derive(CodeMode)]` insist on the four specific field names (`code_mode_config`, `token_secret`, `policy_evaluator`, `code_executor`), and what happens if you rename one of them? (Hint: this is a v0.1.0 convention chosen for clarity over flexibility.)

3. **NoopPolicyEvaluator caveat.** Why does the s41 example use `NoopPolicyEvaluator`, and what must change for production? Read the `LOCAL DEV` comment in the example file. What happens if a production server forgets to swap it out?

4. **Executor selection.** When would you choose `JsCodeExecutor` vs. `SdkCodeExecutor` vs. `McpCodeExecutor` vs. implementing `CodeExecutor` directly? Give one concrete server scenario for each.

5. **Policy ordering.** The chapter calls out that the `PolicyEvaluator` runs *before* token generation. Describe an attack that would become possible if the order were reversed (policy after token mint).

When you can answer all five from memory, head to [Chapter 22 Exercises](./ch22-exercises.md).
