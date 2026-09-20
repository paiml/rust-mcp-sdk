# Hard Workflows: Server-Side Execution

Hard workflows execute entirely on the server side. When a user invokes a prompt, the server runs all steps, binds data between them, and returns complete results—all in a single round-trip.

## The Power of Server-Side Execution

```
┌────────────────────────────────────────────────────────────────────┐
│                    Hard Workflow Execution                         │
├────────────────────────────────────────────────────────────────────┤
│                                                                    │
│  Client                          Server                            │
│    │                               │                               │
│    │──── prompts/get ─────────────►│                               │
│    │     (quarterly_report Q3)     │                               │
│    │                               │                               │
│    │                               │ Step 1: sales_query(Q3)       │
│    │                               │   └─► bind("sales_data")      │
│    │                               │                               │
│    │                               │ Step 2: calculate_metrics     │
│    │                               │   └─► uses sales_data         │
│    │                               │   └─► bind("metrics")         │
│    │                               │                               │
│    │                               │ Step 3: format_report         │
│    │                               │   └─► uses sales_data, metrics│
│    │                               │   └─► bind("report")          │
│    │                               │                               │
│    │◄── complete conversation trace│                               │
│    │     (all results included)    │                               │
│    ▼                               ▼                               │
│                                                                    │
│  Total: 1 round trip (vs 6+ for soft workflow)                     │
└────────────────────────────────────────────────────────────────────┘
```

## SequentialWorkflow: The DSL

The PMCP SDK provides `SequentialWorkflow` for declarative workflow definition:

```rust
use pmcp::server::workflow::{SequentialWorkflow, WorkflowStep, ToolHandle};
use pmcp::server::workflow::dsl::*;

let workflow = SequentialWorkflow::new(
    "code_review",                    // Workflow name (becomes prompt name)
    "Comprehensive code review"        // Description
)
// Define arguments users provide
.argument("code", "Source code to review", true)        // required
.argument("language", "Programming language", false)     // optional

// Define sequential steps
.step(
    WorkflowStep::new("analyze", ToolHandle::new("analyze_code"))
        .arg("code", prompt_arg("code"))
        .arg("language", prompt_arg("language"))
        .bind("analysis")  // Store output for later steps
)
.step(
    WorkflowStep::new("review", ToolHandle::new("review_code"))
        .arg("analysis", from_step("analysis"))  // Use previous output
        .bind("review")
)
.step(
    WorkflowStep::new("format", ToolHandle::new("format_results"))
        .arg("analysis", from_step("analysis"))
        .arg("review", from_step("review"))
        .arg("format", constant(json!("markdown")))
        .bind("final_report")
);
```

### Registering with Server

```rust
let server = Server::builder()
    .name("code-review-server")
    .version("1.0.0")
    // Register tools that the workflow uses
    .tool_typed("analyze_code", analyze_code)
    .tool_typed("review_code", review_code)
    .tool_typed("format_results", format_results)
    // Register the workflow (creates a prompt handler)
    .prompt_workflow(workflow)?
    .build()?;
```

When a user invokes `/code_review`, the server:
1. Receives `prompts/get` with workflow name and arguments
2. Executes all steps sequentially
3. Binds outputs between steps automatically
4. Returns a conversation trace showing all results

## The DSL Building Blocks

### WorkflowStep

Each step represents a tool call:

```rust
WorkflowStep::new(
    "step_name",                    // Identifies this step
    ToolHandle::new("tool_name")    // The tool to call
)
.arg("param", /* source */)         // Tool parameter
.bind("binding_name")               // Store output for other steps
```

### Data Sources (DSL Helpers)

The DSL provides four ways to source argument values:

```rust
use pmcp::server::workflow::dsl::*;

// 1. From workflow arguments (user-provided)
.arg("code", prompt_arg("code"))

// 2. From a previous step's entire output
.arg("data", from_step("analysis"))

// 3. From a specific field of a previous step's output
.arg("score", field("analysis", "confidence_score"))

// 4. Constant values
.arg("format", constant(json!("markdown")))
.arg("max_issues", constant(json!(10)))
```

### Binding Names vs Step Names

**Critical distinction**: Reference bindings, not step names:

```rust
// Step name: "analyze"
// Binding name: "analysis_result"
WorkflowStep::new("analyze", ToolHandle::new("analyzer"))
    .bind("analysis_result")  // ← This is the BINDING name

// Correct: reference the BINDING name
.arg("data", from_step("analysis_result"))  // ✓

// Wrong: referencing the step name
.arg("data", from_step("analyze"))  // ✗ Error!
```

## Complete Example: Code Review Workflow

```rust
use pmcp::server::workflow::{SequentialWorkflow, WorkflowStep, ToolHandle};
use pmcp::server::workflow::dsl::*;
use pmcp::{RequestHandlerExtra, Result, Server};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// Tool input types
#[derive(Debug, Deserialize, JsonSchema)]
struct AnalyzeCodeInput {
    code: String,
    #[serde(default = "default_language")]
    language: String,
}

fn default_language() -> String { "rust".to_string() }

#[derive(Debug, Deserialize, JsonSchema)]
struct ReviewCodeInput {
    analysis: String,
    focus: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FormatCodeInput {
    code: String,
    issues: Vec<String>,
}

// Tool implementations
async fn analyze_code(input: AnalyzeCodeInput, _extra: RequestHandlerExtra) -> Result<Value> {
    Ok(json!({
        "language": input.language,
        "lines_of_code": input.code.lines().count(),
        "analysis_summary": format!("Analyzed {} lines", input.code.lines().count()),
        "issue_details": ["High complexity", "Missing error handling"]
    }))
}

async fn review_code(input: ReviewCodeInput, _extra: RequestHandlerExtra) -> Result<Value> {
    Ok(json!({
        "review_summary": format!("Reviewed with focus: {}", input.focus.join(", ")),
        "recommendations": ["Refactor complex functions", "Add error handling"],
        "approval_status": "conditional"
    }))
}

async fn format_results(input: FormatCodeInput, _extra: RequestHandlerExtra) -> Result<Value> {
    let annotations = input.issues.iter()
        .enumerate()
        .map(|(i, issue)| format!("// TODO {}: {}", i + 1, issue))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(json!({
        "formatted_code": format!("{}\n\n{}", annotations, input.code),
        "issues_annotated": input.issues.len()
    }))
}

// Workflow definition
fn create_code_review_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "code_review",
        "Comprehensive code review with analysis and formatting"
    )
    .argument("code", "Source code to review", true)
    .argument("language", "Programming language (default: rust)", false)

    // Step 1: Analyze code
    .step(
        WorkflowStep::new("analyze", ToolHandle::new("analyze_code"))
            .arg("code", prompt_arg("code"))
            .arg("language", prompt_arg("language"))
            .bind("analysis_result")
    )

    // Step 2: Review code (uses analysis from step 1)
    .step(
        WorkflowStep::new("review", ToolHandle::new("review_code"))
            .arg("analysis", field("analysis_result", "analysis_summary"))
            .arg("focus", constant(json!(["security", "performance"])))
            .bind("review_result")
    )

    // Step 3: Format results (uses data from both previous steps)
    .step(
        WorkflowStep::new("format", ToolHandle::new("format_results"))
            .arg("code", prompt_arg("code"))
            .arg("issues", field("review_result", "recommendations"))
            .bind("formatted_result")
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    let server = Server::builder()
        .name("code-review-server")
        .version("1.0.0")
        .tool_typed("analyze_code", analyze_code)
        .tool_typed("review_code", review_code)
        .tool_typed("format_results", format_results)
        .prompt_workflow(create_code_review_workflow())?
        .build()?;

    // User invokes: /code_review "fn main() {}" rust
    // Server executes all 3 steps automatically
    // Returns complete conversation trace with all results

    Ok(())
}
```

## Workflow Validation

Workflows are **automatically validated** when you register them with `.prompt_workflow()`. If validation fails, registration returns an error and the server won't build.

Common validation errors:

| Error | Cause | Fix |
|-------|-------|-----|
| `UnknownBinding` | `from_step("x")` where no step binds to "x" | Check binding names, add `.bind("x")` |
| `UndefinedArgument` | `prompt_arg("x")` where x not declared | Add `.argument("x", ...)` |
| `InvalidMapping` | Reference to undefined source | Verify DSL helper usage |

For testing, you can also call `.validate()` directly:

```rust
#[test]
fn test_workflow_structure() {
    let workflow = create_my_workflow();
    workflow.validate().expect("Workflow should be valid");
}
```

## Hybrid Workflows: Graceful Handoff

When some steps require LLM reasoning, use **hybrid workflows**. The server executes what it can, then hands off to the AI:

```rust
fn create_task_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "add_project_task",
        "Add task to project with intelligent name matching"
    )
    .argument("project", "Project name (can be fuzzy)", true)
    .argument("task", "Task description", true)

    // Step 1: Server executes (deterministic)
    .step(
        WorkflowStep::new("list_pages", ToolHandle::new("list_pages"))
            .with_guidance("I'll first get all available project names")
            .bind("pages")
    )

    // Step 2: Server can't complete (requires fuzzy matching)
    // Provides guidance + resources for AI to continue
    .step(
        WorkflowStep::new("add_task", ToolHandle::new("add_task"))
            .with_guidance(
                "I'll now:\n\
                 1. Find the project from the list that best matches '{project}'\n\
                 2. Format the task according to the guide below\n\
                 3. Call add_task with the formatted_task parameter"
            )
            .with_resource("docs://task-format")?  // Embed docs for AI
            // No .arg() mappings - server detects incomplete args
            // and gracefully hands off to client LLM
            .bind("result")
    )
}
```

### How Hybrid Execution Works

1. Server receives `prompts/get` with `{project: "MCP Tester", task: "Fix bug"}`
2. Server executes Step 1 (`list_pages`) - deterministic API call
3. Server attempts Step 2 but:
   - Can't map "MCP Tester" to exact page name
   - Detects incomplete argument mapping
4. Server returns partial conversation trace with:
   - Step 1 results (page list)
   - Guidance for AI to complete Step 2
   - Embedded resource content (task formatting docs)
5. AI client receives trace, performs fuzzy matching ("MCP Tester" → "mcp-tester")
6. AI calls `add_task` with correctly formatted task

### When to Use Hybrid

| Server Can Handle | AI Must Handle |
|-------------------|----------------|
| API calls with exact parameters | Fuzzy matching user input |
| Data transformations | Contextual decisions |
| Resource fetching | User clarification |
| Sequential execution | Creative interpretation |

### Resource Embedding: The Developer's Leverage

Even when you can't fully automate tool binding, **embedding relevant resources** into the workflow response significantly improves AI success rates. This is one of the most powerful levers MCP developers have.

```rust
// Workflow step with embedded resources
.step(
    WorkflowStep::new("create_record", ToolHandle::new("database_insert"))
        .with_guidance("Create the record using the schema and validation rules below")
        // Embed documentation the AI needs to complete the step
        .with_resource("db://schema/customers")?      // Table structure
        .with_resource("db://constraints/customers")? // Validation rules
        .with_resource("docs://naming-conventions")?  // Format guidelines
        .bind("result")
)
```

**Why resource embedding matters:**

Without embedded resources, the AI must:
1. Guess which resources might be relevant
2. Make additional `resources/read` calls
3. Hope it found the right documentation
4. Parse and understand the context

With embedded resources, the AI receives:
1. Exactly the documentation it needs
2. In the same response as the workflow
3. Pre-selected by the developer who knows the domain
4. Ready to use immediately

**What to embed:**

| Resource Type | Example | Why It Helps |
|---------------|---------|--------------|
| **Schema definitions** | `db://schema/orders` | AI knows exact field names and types |
| **Validation rules** | `config://validation/email` | AI formats data correctly |
| **Format templates** | `docs://task-format` | AI follows required patterns |
| **Configuration** | `config://regions` | AI uses valid enumeration values |
| **Examples** | `docs://examples/queries` | AI learns by example |
| **Constraints** | `docs://limits/api` | AI respects rate limits, size limits |

**The control hierarchy:**

```
┌─────────────────────────────────────────────────────────────┐
│                  MCP Developer Control                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  MOST CONTROL          ──────────────►       LEAST CONTROL  │
│                                                             │
│  Hard Workflow    Hybrid + Resources    Soft Workflow       │
│  ─────────────    ──────────────────    ────────────        │
│  Server executes  Server provides       Text guidance       │
│  all steps        context + guidance    only                │
│                                                             │
│  • Deterministic  • AI completes with   • AI figures out    │
│  • Single trip      full context          everything        │
│  • Guaranteed     • High success rate   • Unpredictable     │
│    results        • Developer curated   • Multiple trips    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Best practice**: When you can't make a step fully deterministic, ask yourself: "What documentation would I need to complete this step?" Then embed those resources.

## Advanced Patterns

### Multiple Steps Using Same Output

Fan-out pattern—one output feeds multiple steps:

```rust
SequentialWorkflow::new("analysis", "Multi-faceted analysis")
    .step(
        WorkflowStep::new("fetch", ToolHandle::new("fetch_data"))
            .arg("source", prompt_arg("source"))
            .bind("data")  // Single binding
    )
    .step(
        WorkflowStep::new("analyze", ToolHandle::new("analyzer"))
            .arg("input", from_step("data"))  // Uses "data"
            .bind("analysis")
    )
    .step(
        WorkflowStep::new("summarize", ToolHandle::new("summarizer"))
            .arg("input", from_step("data"))  // Also uses "data"
            .bind("summary")
    )
    .step(
        WorkflowStep::new("validate", ToolHandle::new("validator"))
            .arg("input", from_step("data"))  // Also uses "data"
            .bind("validation")
    )
```

### Extracting Specific Fields

When tool outputs are complex, extract only what you need:

```rust
// Assume "analysis" output is:
// {
//   "summary": { "text": "...", "length": 42 },
//   "scores": { "confidence": 0.95, "accuracy": 0.88 },
//   "metadata": { "timestamp": "..." }
// }

.step(
    WorkflowStep::new("report", ToolHandle::new("reporter"))
        .arg("summary", field("analysis", "summary"))     // Extract object
        .arg("confidence", field("analysis", "scores"))   // Extract object
        .arg("timestamp", field("analysis", "metadata"))  // Extract object
        .bind("report")
)
```

### Steps Without Bindings

Terminal or side-effect-only steps don't need bindings:

```rust
.step(
    WorkflowStep::new("process", ToolHandle::new("processor"))
        .arg("input", prompt_arg("data"))
        .bind("result")  // ← Needed by next step
)
.step(
    WorkflowStep::new("log", ToolHandle::new("logger"))
        .arg("message", from_step("result"))
        // NO .bind() - just logs, output not used
)
.step(
    WorkflowStep::new("notify", ToolHandle::new("notifier"))
        .arg("status", constant(json!("complete")))
        // NO .bind() - terminal step, side-effect only
)
```

### Adding System Instructions

Guide LLM behavior across the workflow:

```rust
SequentialWorkflow::new("research", "Research workflow")
    .instruction(InternalPromptMessage::system(
        "You are a research assistant. Be thorough and cite sources."
    ))
    .instruction(InternalPromptMessage::system(
        "Format all responses in markdown with clear sections."
    ))
    .step(...)
    .step(...)
```

## Conversation Trace Format

When the server executes a workflow, it returns a conversation trace:

```
Message 1 [User]:
  "Execute code_review workflow with code: 'fn main() {}', language: 'rust'"

Message 2 [Assistant]:
  "I'll perform a code review in 3 steps: analyze, review, format"

Message 3 [Assistant]:
  "Calling analyze_code with {code: 'fn main() {}', language: 'rust'}"

Message 4 [User]:
  "Tool result: {analysis_summary: 'Analyzed 1 lines', issue_details: [...]}"

Message 5 [Assistant]:
  "Calling review_code with {analysis: '...', focus: ['security']}"

Message 6 [User]:
  "Tool result: {recommendations: ['Refactor...', 'Add error...']}"

Message 7 [Assistant]:
  "Calling format_results with {code: '...', issues: [...]}"

Message 8 [User]:
  "Tool result: {formatted_code: '// TODO 1: Refactor...\n\nfn main() {}'}"
```

The AI receives this complete trace and can synthesize a final response.

## Workflow vs Tool: When to Use Each

| Use Tool | Use Workflow |
|----------|--------------|
| Single operation | Multi-step process |
| AI decides when to call | User explicitly invokes |
| Flexible parameter choice | Fixed execution sequence |
| Independent action | Coordinated pipeline |

Workflows are essentially **compound tools** with deterministic execution and automatic data binding.

## Best Practices

### 1. Start Hard, Soften as Needed

```rust
// First: Try to make it fully deterministic
SequentialWorkflow::new("report", "Generate report")
    .step(...).step(...).step(...)

// If some steps need AI reasoning:
// Add .with_guidance() for hybrid execution

// If most steps need AI reasoning:
// Consider a soft workflow (text prompt) instead
```

### 2. Use Descriptive Binding Names

```rust
// Good: Clear what the binding contains
.bind("customer_orders")
.bind("revenue_metrics")
.bind("formatted_report")

// Bad: Ambiguous
.bind("data")
.bind("result")
.bind("output")
```

### 3. Validation: Automatic and Fail-Fast

Good news: **validation is automatic**. When you call `.prompt_workflow()`, the builder validates the workflow and returns an error if it's invalid:

```rust
let server = Server::builder()
    .name("my-server")
    .version("1.0.0")
    .tool_typed("analyze_code", analyze_code)
    .prompt_workflow(workflow)?  // ← Validates here, fails if invalid
    .build()?;
```

If there's a validation error (unknown binding, undefined argument, etc.), the server won't start. This is fail-fast behavior—you'll see the error immediately when starting your server, not when a user invokes the workflow.

**Validation errors are actionable:**

```
Error: Workflow validation failed: Unknown binding "analysis" in step "review".
Available bindings: ["analysis_result"]
Hint: Did you mean "analysis_result"?
```

### 4. Testing Workflows

Since validation is automatic at registration, the best way to catch errors early is with **unit tests**:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Test 1: Workflow structure is valid
    #[test]
    fn workflow_is_valid() {
        let workflow = create_code_review_workflow();

        // .validate() is useful in tests for explicit validation
        workflow.validate().expect("Workflow should be valid");

        // Check expected structure
        assert_eq!(workflow.name(), "code_review");
        assert_eq!(workflow.steps().len(), 3);
        assert!(workflow.output_bindings().contains(&"formatted_result".into()));
    }

    // Test 2: Workflow executes correctly
    #[tokio::test]
    async fn workflow_execution() {
        let server = Server::builder()
            .name("test")
            .version("1.0.0")
            .tool_typed("analyze_code", analyze_code)
            .tool_typed("review_code", review_code)
            .tool_typed("format_results", format_results)
            .prompt_workflow(create_code_review_workflow())
            .expect("Workflow should register")
            .build()
            .expect("Server should build");

        let handler = server.get_prompt("code_review").unwrap();

        let mut args = HashMap::new();
        args.insert("code".into(), "fn test() {}".into());
        args.insert("language".into(), "rust".into());

        let result = handler.handle(args, test_extra()).await
            .expect("Workflow should execute");

        // Assert on conversation trace
        assert_eq!(result.messages.len(), 8);  // Intent + plan + 3 steps × 2 messages
    }
}
```

### 5. CLI Validation with `cargo pmcp validate`

For project-wide validation before commits or in CI pipelines, use the CLI:

```bash
# Validate all workflows in the current server
cargo pmcp validate workflows

# Verbose output (shows all test output)
cargo pmcp validate workflows --verbose

# Validate a specific server in a workspace
cargo pmcp validate workflows --server ./servers/my-server

# Generate validation test scaffolding
cargo pmcp validate workflows --generate
```

**What `cargo pmcp validate workflows` does:**

1. **Compilation Check**: Runs `cargo check` to ensure the project compiles
2. **Test Discovery**: Finds workflow validation tests (patterns: `workflow`, `test_workflow`, `workflow_valid`, `workflow_validation`)
3. **Test Execution**: Runs all discovered tests with detailed output
4. **Summary**: Reports pass/fail status with actionable guidance

**Example output:**

```
🔍 PMCP Workflow Validation
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Step 1: Checking compilation...
  ✓ Compilation successful

Step 2: Looking for workflow validation tests...
  ✓ Found 2 workflow test pattern(s)

Step 3: Running workflow validation tests...
  ✓ Pattern 'workflow': 3 passed
  ✓ Pattern 'test_workflow': 2 passed

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✓ All 5 workflow validation tests passed!

  Your workflows are structurally valid and ready for use.
```

**Generating test scaffolding:**

If you don't have workflow tests yet, use `--generate`:

```bash
cargo pmcp validate workflows --generate
```

This creates `tests/workflow_validation.rs` with templates:

```rust
//! Workflow validation tests
//!
//! Generated by `cargo pmcp validate workflows --generate`

#[test]
fn test_workflow_is_valid() {
    let workflow = create_my_workflow();
    workflow.validate().expect("Workflow should be valid");
    assert_eq!(workflow.name(), "my_workflow");
}

#[test]
fn test_workflow_bindings() {
    let workflow = create_my_workflow();
    let bindings = workflow.output_bindings();
    assert!(bindings.contains(&"result".into()));
}

#[tokio::test]
async fn test_workflow_execution() {
    // Integration test template
}
```

### 6. Developer Experience Roadmap

Workflow validation happens at different stages:

| Stage | When | What's Caught | Status |
|-------|------|---------------|--------|
| **Registration** | Server startup | Binding errors, undefined args | ✅ Automatic |
| **Unit Tests** | `cargo test` | Structural + execution errors | ✅ Pattern above |
| **CLI Validation** | `cargo pmcp validate` | Project-wide validation | ✅ Available |
| **Compile-Time** | Compilation | Invalid workflows don't compile | 🔮 Future |
| **IDE** | While typing | Real-time feedback | 🔮 Future |

**Best practice**: Combine unit tests (`cargo test`) with CLI validation (`cargo pmcp validate`) in your CI pipeline. This ensures both structural correctness and execution behavior are verified before deployment.

**Future**: The PMCP SDK roadmap includes proc_macro support for compile-time checks, enabling IDE integration with real-time validation feedback.

## Summary

Hard workflows provide:

| Benefit | How |
|---------|-----|
| **Single round-trip** | Server executes all steps |
| **Deterministic execution** | Fixed sequence, no AI decisions |
| **Automatic data binding** | `from_step()`, `field()` DSL |
| **Early validation** | Catch errors at registration time |
| **Easy testing** | Pure function tests, no AI required |

The workflow spectrum:

| Type | Server Executes | AI Handles |
|------|-----------------|------------|
| **Hard** | All steps | Final synthesis only |
| **Hybrid** | Deterministic steps | Fuzzy matching, clarification |
| **Soft** | Nothing | All steps (follows text guidance) |

Remember: **Do as much as possible on the server side.** Hard workflows should be your default choice. Fall back to hybrid or soft only when genuine LLM reasoning is required.
