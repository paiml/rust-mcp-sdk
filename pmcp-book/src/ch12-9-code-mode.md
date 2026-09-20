# Chapter 12.9: Code Mode — Validated LLM Code Execution

Code Mode is PMCP's framework for safely executing LLM-generated code
(GraphQL, JavaScript, SQL, MCP compositions) with cryptographic approval
tokens that bind the validated code to a specific user, session, and schema
version. PMCP ships Code Mode as two cooperating crates: `pmcp-code-mode`
(runtime types, validation pipeline, executors, policy evaluators) and
`pmcp-code-mode-derive` (the `#[derive(CodeMode)]` proc macro that wires
two MCP tools — `validate_code` and `execute_code` — onto a `ServerBuilder`
without per-server handler boilerplate). This chapter walks one worked
example end-to-end: `examples/s41_code_mode_graphql.rs`. After the chapter
you should be able to run that example, see both the success and rejection
paths fire, and choose between the derive macro and a manual `CodeExecutor`
impl for your own server.

## The Problem (Why Cryptographic Approval Tokens)

When an LLM generates code to run against your backend — a GraphQL query,
a JavaScript plan, a SQL statement, an MCP tool composition — you face
three questions:

1. **Is this code safe?** Does it pass parser and security checks
   (syntax, depth limits, blocked fields, injection patterns)?
2. **Is the caller authorized?** Does it satisfy your policy engine
   (Cedar, AWS Verified Permissions, or a custom `PolicyEvaluator`)
   for *this* user, session, and schema version?
3. **Is the code tamper-free between approval and execution?** When the
   client comes back with "execute that approved query", is it actually
   the same bytes that passed validation — not a substituted payload?

Without Code Mode, servers hand-roll validation, token generation, and
policy enforcement for every code-generating workflow. The result is
inconsistent security boundaries and duplicated boilerplate that drifts
across servers. Code Mode answers all three questions in one place:
a validation pipeline that ends with an HMAC-signed approval token, then
an execution path that refuses any code whose hash doesn't match the token.

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

The HMAC-SHA256 token binds: **code hash, user ID, session ID, server ID,
context hash, risk level, and expiry**. Any modification to the code after
validation invalidates the token. Tokens are stateless — they are HMAC-verified
on every `execute_code` call, never stored server-side. Default TTL is
5 minutes; configurable via `CodeModeConfig::token_ttl_seconds`.

## Adding Code Mode with `#[derive(CodeMode)]`

The `#[derive(CodeMode)]` macro is the canonical entry point. It generates
a `register_code_mode_tools(builder)` method on your server struct that
adds the `validate_code` and `execute_code` tools to a `pmcp::ServerBuilder`,
eliminating ~80 lines of handler boilerplate per server. Manual
`CodeModeHandler` registration is the advanced/escape-hatch path — only
reach for it when you need to customize tool metadata or split the
validation and execution surfaces across separate transports.

### Step 1: Add Dependencies

Add the two Code Mode crates alongside `pmcp` in your `Cargo.toml`. The
versions below match the pins in this workspace as of writing — verify
against `crates/pmcp-code-mode/Cargo.toml` and
`crates/pmcp-code-mode-derive/Cargo.toml` for the latest release pins.

```toml
[dependencies]
pmcp = { version = "2.7", features = ["full"] }
pmcp-code-mode = "0.5.1"
pmcp-code-mode-derive = "0.3.0"
```

The `pmcp-code-mode` crate is feature-gated: the default build supports
GraphQL only. To enable JavaScript validation add `features =
["openapi-code-mode"]`; for SQL add `["sql-code-mode"]`; for MCP tool
composition add `["mcp-code-mode"]`. The derive macro itself has no
features — it dispatches to whichever validation method the parent
crate exposes based on the `language` attribute (see Step 3).

### Step 2: Derive and Configure

Annotate your server struct with `#[derive(CodeMode)]`. The macro
requires four fields by name (v0.1.0 field-name convention — these
names are the API, not a style guideline):

```rust,ignore
#[derive(CodeMode)]
struct MyGraphQLServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    #[allow(dead_code)]
    // Required by #[derive(CodeMode)] convention; used when policy evaluation is wired
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<GraphQLExecutor>,
}
```

| Field name        | Required type                  | Purpose                                |
|-------------------|--------------------------------|----------------------------------------|
| `code_mode_config`| `CodeModeConfig`               | Pipeline configuration (TTL, allowlists, auto-approve thresholds) |
| `token_secret`    | `TokenSecret`                  | HMAC signing secret (≥16 bytes, zeroize-on-drop) |
| `policy_evaluator`| `Arc<impl PolicyEvaluator>`    | Authorization backend (Cedar / AVP / custom)     |
| `code_executor`   | `Arc<impl CodeExecutor>`       | Backend that runs the validated code             |

If any required field is absent, the macro emits a single compile
error naming all the missing fields. No field can be renamed in
v0.1.0; the macro resolves them by literal name.

The s41 example omits both struct-level attributes because it accepts
the GraphQL default and uses a placeholder `ValidationContext`. In
production you should specify `context_from` so the validation
context is bound to the live user and session rather than placeholder
strings. A custom `context_from` looks like this:

```rust,ignore
#[derive(CodeMode)]
#[code_mode(context_from = "get_context", language = "graphql")]
struct MyServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<MyExecutor>,
}

impl MyServer {
    fn get_context(&self, extra: &pmcp::RequestHandlerExtra) -> ValidationContext {
        // Build real context from the MCP session
        ValidationContext::new("user-123", "session-456", "schema-v1", "perms-v1")
    }
}
```

When `context_from` is set, the generated `register_code_mode_tools`
method requires `self: &Arc<Self>` so the user-defined accessor can
be invoked on every request. When `context_from` is omitted the macro
emits a `#[deprecated]` warning and uses static placeholder values —
useful for examples and tests, never for production.

### Step 3: Pick a Language

The `language` attribute selects which validation method the generated
`validate_code` handler calls. The table below is sourced directly
from `crates/pmcp-code-mode-derive/src/lib.rs::gen_validation_call` —
if you add a new language to the derive macro you must add a row here
and a `CodeLanguage` variant in `pmcp-code-mode`.

| Language   | `language =` value     | Validation method                  | Feature required        |
|------------|------------------------|------------------------------------|-------------------------|
| GraphQL    | `"graphql"` (default)  | `validate_graphql_query_async`     | *(none)*                |
| JavaScript | `"javascript"` / `"js"`| `validate_javascript_code_async`   | `openapi-code-mode`     |
| SQL        | `"sql"`                | `validate_sql_query_async`         | `sql-code-mode`         |
| MCP        | `"mcp"`                | `validate_mcp_composition`         | `mcp-code-mode`         |

Specifying an unknown `language` value is a compile error with a
suggestion message listing the four supported values.

### Step 4: Choose Your Executor

Code Mode separates *what the code is* (validated by the pipeline) from
*what running it means* (the `CodeExecutor` impl). You have two paths.

**Direct `CodeExecutor` impl.** Best when your backend is a database
connection, an HTTP client to a specific service, or an in-process
schema — anywhere the executor's behavior is unique to your server.
You implement one async method, `execute(&self, code, variables) ->
Result<Value, ExecutionError>`. The s41 example takes this path:

```rust,ignore
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

Full example: [`examples/s41_code_mode_graphql.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s41_code_mode_graphql.rs).

**Standard adapters.** Best when your backend is a generic HTTP client,
an AWS SDK call, or routing to another MCP server — patterns common
enough that PMCP ships ready-made executors:

<!-- synthetic -->
```rust,ignore
// JavaScript + HTTP calls (Cost Coach pattern); needs `js-runtime` feature
let executor = Arc::new(JsCodeExecutor::new(http_client, ExecutionConfig::default()));

// JavaScript + SDK operations; needs `js-runtime` feature
let executor = Arc::new(SdkCodeExecutor::new(sdk_client, ExecutionConfig::default()));

// MCP tool composition; needs `mcp-code-mode` feature
let executor = Arc::new(McpCodeExecutor::new(mcp_router, ExecutionConfig::default()));
```

| Adapter            | Backend shape                       | Use when                                                |
|--------------------|-------------------------------------|---------------------------------------------------------|
| `JsCodeExecutor`   | JavaScript scripts → HTTP calls     | LLM generates JS that drives a REST/JSON API (Cost Coach) |
| `SdkCodeExecutor`  | JavaScript scripts → SDK methods    | LLM scripts call an AWS/Cloud SDK directly              |
| `McpCodeExecutor`  | MCP compositions → tool routing     | LLM composes existing MCP tools into a workflow         |

Adapters are feature-gated: `JsCodeExecutor` and `SdkCodeExecutor`
require the `js-runtime` feature on `pmcp-code-mode`; `McpCodeExecutor`
requires `mcp-code-mode`. They implement `CodeExecutor`, so dropping
one into the `code_executor` field of your derived server is sufficient
— no other wiring required.

### Step 5: Register on the Builder and Build

Construct your server, build a `pmcp::ServerBuilder`, and call the
macro-generated `register_code_mode_tools(builder)`:

```rust,ignore
let server = MyGraphQLServer {
    code_mode_config: CodeModeConfig::enabled(),
    token_secret: TokenSecret::new(b"example-secret-key-32-bytes!!!!".to_vec()),
    policy_evaluator: Arc::new(NoopPolicyEvaluator::new()),
    code_executor: Arc::new(GraphQLExecutor),
};

// 2. Register code mode tools on the builder
// (In a real server, you would also add other tools and then build/run the server)
let builder = pmcp::Server::builder();
#[allow(deprecated)]
let _builder = server
    .register_code_mode_tools(builder)
    .expect("Failed to register code mode tools");
println!("Registered validate_code and execute_code tools on builder.");
```

After this call the builder carries two new tools: `validate_code`
(parses, scans, policy-checks, explains, signs) and `execute_code`
(verifies the token, then runs the executor). You add the rest of
your server's tools on the same builder, call `.build()`, and run
the resulting `Server` on the transport of your choice.

## Worked Example: GraphQL Round-Trip (`s41_code_mode_graphql.rs`)

This section walks the success and rejection paths from the
end-to-end example. Run it with:

```bash
cargo run --example s41_code_mode_graphql --features full
```

Full example: [`examples/s41_code_mode_graphql.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s41_code_mode_graphql.rs).

**Success path.** A read-only `query` passes parsing, security
scanning, and the default policy. The pipeline emits an
`approval_token`, and the executor produces a mock result:

```rust,ignore
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
        println!("Explanation: {}", result.explanation);

        // Execute with the approval token
        if result.approval_token.is_some() {
            let exec_result = server.code_executor.execute(query, None).await;
            match exec_result {
                Ok(data) => println!(
                    "\nExecution result:\n{}",
                    serde_json::to_string_pretty(&data).expect("JSON serialization")
                ),
                Err(e) => println!("\nExecution error: {e:?}"),
            }
        }
    },
    Err(e) => {
        println!("Validation: FAILED - {e:?}");
    },
}
```

What each step does:

- `pipeline.validate_graphql_query(query, &context)` runs the entire
  pipeline (parse → security scan → policy check → explain → HMAC sign)
  against the supplied `ValidationContext`.
- A non-empty `result.approval_token` means *all* gates passed; a
  `None` token (or `is_valid = false`) means the code was rejected
  somewhere along the way.
- `server.code_executor.execute(query, None).await` runs the
  validated code through the executor. In a real handler this is
  what the `execute_code` tool calls — but only *after* re-verifying
  the token. In this example we short-circuit the verification step
  to keep the narrative tight; production code never bypasses it.

**Rejection path.** A `mutation` is sent against the default
`CodeModeConfig::enabled()`, which sets `allow_mutations: false`.
The pipeline returns `is_valid = false`, no `approval_token` is
minted, and the executor is never reached:

```rust,ignore
// --- REJECTION PATH ---
// Demonstrates that mutations are rejected when allow_mutations is false (the default).
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
        println!("This demonstrates that invalid code does NOT receive an approval token.");
    },
}
```

The load-bearing security property: **the rejection happens *before*
the token is minted.** No code can reach `execute_code` without a
valid token; no token exists for code that was rejected. There is
no "second chance" — the LLM cannot replay an earlier token against
a new mutation, and it cannot patch the rejected mutation and ask
the executor to run it without going back through the pipeline.

## Configuration in `config.toml` (Platform-Level Policy)

When deploying with `cargo pmcp deploy`, include a `config.toml` that
declares your server's available operations. The pmcp.run platform
reads this to populate the Code Mode policy page, allowing administrators
to enable or disable individual operations without redeploying:

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

This separates "what the runtime allows" (`CodeModeConfig` in code —
hard guarantees enforced by every server instance) from "what the
platform permits" (`config.toml` in the deploy bundle — operator
toggles managed in the pmcp.run admin UI). Both layers must agree
before an operation can pass validation.

Operations are automatically categorized (read/write/delete/admin)
based on the server type and HTTP method, GraphQL operation type, or
explicit `operation_category` overrides. Administrators can enable or
disable entire categories at once, or drill down to individual
operations for fine-grained control.

## Policy Evaluation (Cedar / AVP / Custom)

Code Mode supports pluggable policy evaluation between validation and
token generation. The `PolicyEvaluator` trait
(`crates/pmcp-code-mode/src/policy/mod.rs:38`) defines one async method
per supported code shape — `evaluate_operation` (always), plus
`evaluate_script` (under `openapi-code-mode`) and `evaluate_statement`
(under `sql-code-mode`). The pipeline calls the appropriate variant
after the security scan and before the approval token is generated.

- **Cedar:** local policy evaluation, no network round-trip. Enabled
  by the `cedar` feature on `pmcp-code-mode`. Suitable for in-process
  policy decisions when you control the policy bundle.
- **AWS Verified Permissions:** remote evaluation against an AVP
  policy store. Enabled by the `avp` feature on `pmcp-code-mode`.
  Suitable when policies are managed centrally and updated out-of-band.
- **Custom:** implement the `PolicyEvaluator` trait directly. Useful
  for OPA, a bespoke RBAC service, or test doubles. The minimum
  surface area is one method:

<!-- synthetic -->
```rust,ignore
#[async_trait::async_trait]
pub trait PolicyEvaluator: Send + Sync {
    async fn evaluate_operation(
        &self,
        operation: &OperationEntity,
        server_config: &ServerConfigEntity,
    ) -> Result<AuthorizationDecision, PolicyEvaluationError>;

    fn name(&self) -> &str;
}
```

> **Production warning.** The s41 example uses `NoopPolicyEvaluator`,
> which returns `allowed: true` for every request — *for testing and
> local development ONLY*. Production servers MUST implement
> `PolicyEvaluator` with a real authorization backend (Cedar, AVP, or
> custom). Deploying `NoopPolicyEvaluator` disables the entire policy
> layer; combined with a leaked token secret it would expose the
> executor to arbitrary code execution.

The policy evaluator runs after parsing and security scanning, before
the approval token is generated. **A denied operation never receives
a token.**

## Security Properties Reference

The load-bearing security properties of Code Mode, sourced from the
`pmcp-code-mode` crate:

- **Tokens are stateless.** Verification is HMAC-only; no server-side
  token store is required. This is what makes Code Mode safe to run
  behind load balancers and Lambda fan-out.
- **Default TTL: 5 minutes.** Configurable via
  `CodeModeConfig::token_ttl_seconds`. Short windows reduce the
  blast radius of leaked tokens.
- **`TokenSecret` is zeroize-on-drop.** Internally backed by
  `secrecy::SecretBox`; the in-memory secret is wiped when the
  `TokenSecret` is dropped, preventing post-mortem dumps from
  recovering it.
- **Minimum 16-byte secret.** `TokenSecret::new` returns `Result`
  (no panic) and rejects shorter inputs. HMAC-SHA256 needs adequate
  entropy in the key for the security analysis to hold.
- **Code canonicalization.** The hash bound into the approval token
  is computed over a canonicalized form of the source (whitespace,
  comments, casing for keywords). An attacker cannot reuse a token
  by re-formatting the validated code.

## Next Steps

- Run the worked example: `cargo run --example s41_code_mode_graphql
  --features full`.
- Source-of-truth docs:
  [`pmcp-code-mode`](https://docs.rs/pmcp-code-mode) (runtime types,
  pipeline, executors, policy traits) and
  [`pmcp-code-mode-derive`](https://docs.rs/pmcp-code-mode-derive)
  (derive macro attributes and expansion semantics).
- For platform-level policy management see the pmcp.run admin UI
  Code Mode page, fed by the `[code_mode]` block in your
  `config.toml` (covered in the "Configuration in `config.toml`"
  section above).
