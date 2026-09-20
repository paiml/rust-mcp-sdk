# Chapter 22 Exercises

These exercises build your fluency with PMCP Code Mode. Each one targets a different aspect of the `#[derive(CodeMode)]` workflow — from mechanical wiring (Exercise 1) to operating the rejection path (Exercise 2) to swapping the policy layer (Exercise 3).

## Exercise 1: Wire `#[derive(CodeMode)]` into a Minimal Server

**Difficulty:** Introductory (15 min)

Practice the mechanical steps of adding Code Mode to a fresh server. You will copy the four required field names, build successfully, and confirm that the derive macro registers both `validate_code` and `execute_code` in `tools/list`.

**Steps:**

1. Create a new binary project (or add to an existing test crate).
2. Add the dependencies from [Chapter 22](./ch22-code-mode.md) Step 1 to your `Cargo.toml`:
   - `pmcp = { version = "2.7.0", features = ["full"] }`
   - `pmcp-code-mode = "0.5.1"`
   - `pmcp-code-mode-derive = "0.3.0"`
3. Define a server struct with the four required fields:
   - `code_mode_config: CodeModeConfig`
   - `token_secret: TokenSecret`
   - `policy_evaluator: Arc<dyn PolicyEvaluator>` (or a concrete type like `Arc<NoopPolicyEvaluator>`)
   - `code_executor: Arc<dyn CodeExecutor>`
4. Annotate the struct with `#[derive(CodeMode)]`. Accept the default `language = "graphql"`.
5. Implement a stub `CodeExecutor` that returns `serde_json::json!({ "stub": true })` for any input.
6. Use `NoopPolicyEvaluator::new()` for the policy field — **LOCAL DEV ONLY** (see Chapter 22's NoopPolicyEvaluator warning).
7. Construct an instance, call `server.register_code_mode_tools(Server::builder())`, build the server, and run it over stdio.
8. In another terminal, run `mcp-tester stdio ./target/debug/your-server` and inspect the `tools/list` response.

```rust,ignore
use pmcp_code_mode::{
    CodeExecutor, CodeModeConfig, ExecutionError, NoopPolicyEvaluator, TokenSecret,
};
use pmcp_code_mode_derive::CodeMode;
use serde_json::{json, Value};
use std::sync::Arc;

struct StubExecutor;

#[pmcp_code_mode::async_trait]
impl CodeExecutor for StubExecutor {
    async fn execute(&self, _code: &str, _vars: Option<&Value>) -> Result<Value, ExecutionError> {
        Ok(json!({ "stub": true }))
    }
}

#[derive(CodeMode)]
struct MinimalServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<StubExecutor>,
}
```

### Verify your solution

The `tools/list` response from `mcp-tester` includes both `validate_code` and `execute_code` with their expected JSON schemas. If either tool is missing, the derive macro did not register both — re-read the chapter's Step 5 and re-check the `server.register_code_mode_tools(builder)` call. If your build fails with "no field named code_mode_config on type MinimalServer", you renamed one of the four required field names — restore the exact names from the chapter.

**Questions to answer:**

- What happens if you rename one of the four required struct fields (e.g., `code_mode_config` → `cfg`)? Why does the derive macro choose this convention over allowing arbitrary field names?
- The `Arc<dyn PolicyEvaluator>` field needs to point at SOMETHING even for local dev. Why doesn't the derive macro provide a default `NoopPolicyEvaluator` implicitly?

---

## Exercise 2: Trigger and Inspect the Rejection Path

**Difficulty:** Intermediate (30 min)

Exercise the security-critical rejection path. You will send a GraphQL mutation through `validate_code` under the default `CodeModeConfig::enabled()` and confirm (a) no token is minted, (b) the same mutation succeeds after opting into `allow_mutations: true`, and (c) any post-validation modification to the code invalidates the token at `execute_code` time.

**Steps:**

1. Start from the server you built in Exercise 1 (or copy `examples/s41_code_mode_graphql.rs`).
2. Add a test (or a separate `main()` binary) that builds a `ValidationPipeline` directly via `ValidationPipeline::from_token_secret(config.clone(), &token_secret)` so you can drive validation without going through the MCP transport.
3. Call `pipeline.validate_graphql_query("mutation { deleteUser(id: \"123\") }", &context)`. Assert that the response has `is_valid == false` and `approval_token` is `None`.
4. Capture the rejection-reason string from the `violations` vector and print it.
5. Change the config from `CodeModeConfig::enabled()` to `CodeModeConfig { allow_mutations: true, ..CodeModeConfig::enabled() }`. Rebuild the pipeline.
6. Re-run the same mutation. Assert `is_valid == true` and `approval_token.is_some()`.
7. Now feed the approval token + a SLIGHTLY MODIFIED mutation (change `"123"` to `"124"`) into `execute_code` (in a real round-trip, you would call the `execute_code` MCP tool with both arguments). Confirm the verification step rejects the request with a code-hash mismatch.

```rust,ignore
// Default config: mutation should be rejected.
let config = CodeModeConfig::enabled();
let pipeline = ValidationPipeline::from_token_secret(config.clone(), &token_secret)
    .expect("pipeline construction");
let result = pipeline
    .validate_graphql_query("mutation { deleteUser(id: \"123\") }", &context)
    .expect("validate_graphql_query");
assert!(!result.is_valid);
assert!(result.approval_token.is_none());

// Opted-in config: mutation should pass.
let opted_in = CodeModeConfig { allow_mutations: true, ..CodeModeConfig::enabled() };
let pipeline2 = ValidationPipeline::from_token_secret(opted_in, &token_secret)
    .expect("pipeline construction");
let ok = pipeline2
    .validate_graphql_query("mutation { deleteUser(id: \"123\") }", &context)
    .expect("validate_graphql_query");
assert!(ok.is_valid);
assert!(ok.approval_token.is_some());
```

### Verify your solution

All three assertions pass: (a) default config rejects the mutation, (b) `allow_mutations: true` accepts it, (c) modifying the code after token mint invalidates the token at `execute_code`. If (c) does NOT reject — i.e., the modified mutation runs anyway — your HMAC verification is being bypassed somewhere; re-read the Chapter 22 "Security Properties Reference" section and verify that `execute_code` actually runs token verification before calling `code_executor.execute(...)`.

**Questions to answer:**

- Step 7 demonstrates the load-bearing security property of HMAC binding. Describe in your own words what attack this prevents. (Hint: think about a man-in-the-middle between the user's approval and the server's `execute_code` call.)
- Why does Code Mode reject mutations by default rather than by opt-in? Connect this back to Chapter 22's "Why Code Mode Matters for Enterprise MCP" framing.

---

## Exercise 3: Swap `NoopPolicyEvaluator` for a Custom `PolicyEvaluator`

**Difficulty:** Advanced (60 min)

Replace the local-dev stub with a real policy layer. You will implement a `PolicyEvaluator` that denies operations whose user ID is `"banned-user"` and confirm that denied operations never produce an approval token. This is exactly the production-replacement path that Chapter 22 warns about.

**Steps:**

1. Implement a struct `BannedUserPolicyEvaluator` that implements the `PolicyEvaluator` trait from `pmcp_code_mode`.
2. The `evaluate` method should return a denial decision (with reason `"user is banned"`) when `context.user_id == Some("banned-user")` and an allow decision otherwise.
3. Wire it into the server struct from Exercise 1 in place of `NoopPolicyEvaluator`. Change the field type from `Arc<NoopPolicyEvaluator>` to `Arc<BannedUserPolicyEvaluator>` (or `Arc<dyn PolicyEvaluator>`).
4. Write a test that calls `validate_code` with a `ValidationContext` whose `user_id` is `"banned-user"`. Assert no approval token is returned and that the rejection reason mentions "banned".
5. Repeat with `user_id = "alice"` (or any non-banned value). Assert the call succeeds with a non-empty approval token.
6. **Bonus:** log the policy decision (the reason string) to stderr on every evaluate call so the rejection path is observable in production logs.

```rust,ignore
use pmcp_code_mode::{PolicyEvaluator, /* PolicyDecision and related types */};

struct BannedUserPolicyEvaluator;

#[pmcp_code_mode::async_trait]
impl PolicyEvaluator for BannedUserPolicyEvaluator {
    // Method signature follows pmcp_code_mode::PolicyEvaluator;
    // consult the crate docs for the exact `evaluate` shape in your pinned version.
    async fn evaluate(&self, /* context */ /* ... */) -> /* PolicyDecision */ {
        // Deny when user_id == Some("banned-user"); Allow otherwise.
        todo!("Implement the deny/allow branch based on context.user_id")
    }
}
```

### Verify your solution

Both test cases pass — the banned user is rejected (no approval token), and the allowed user gets a non-empty token. The bonus stderr log shows the rejection reason for the banned-user case. If both calls produce a token regardless of the `user_id` value, your `evaluate` method is not being invoked — re-check that you swapped the `policy_evaluator` field on the server struct and that the type matches what `register_code_mode_tools` expects.

**Questions to answer:**

- In a production architecture, where would `PolicyEvaluator` typically run — in the same process as the server, or in a remote service? When does each choice make sense? (Hint: Chapter 22's "Policy Evaluation (Cedar / AVP / Custom)" section contrasts Cedar with AVP.)
- Chapter 22 calls out that the policy evaluator runs *before* token generation. Describe in your own words why that ordering is security-critical, and what attack becomes possible if the order were reversed.

---

**Reference:** All three exercises build on `examples/s41_code_mode_graphql.rs`, the canonical worked example from [Chapter 22: Code Mode](./ch22-code-mode.md). Exercises 1 and 2 closely mirror that example; Exercise 3 extends it by replacing `NoopPolicyEvaluator` — the substitution that Chapter 22 says every production deployment must make.

## Prerequisites

Before starting these exercises, ensure you have:

- Read [Chapter 22: Code Mode](./ch22-code-mode.md) end-to-end.
- A working Rust development environment with `pmcp` 2.7.0+ in your dependencies.
- `mcp-tester` installed (`cargo install mcp-tester` or via `cargo-pmcp`).
- Run `cargo run --example s41_code_mode_graphql --features full` once and observed both the success and rejection paths.

## Next Steps

After completing these exercises, continue to:

- [Chapter 23: Skills](./ch23-skills.md) — the third v2 advanced feature, complementary to Code Mode.
- [Appendix A: cargo pmcp Reference](../appendix/cargo-pmcp-reference.md) — CLI tooling for deploying Code Mode servers.
