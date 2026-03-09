# Chapter 7: Prompts — User-Triggered Workflows

Prompts are pre-defined workflows that users explicitly trigger from their MCP client. While **tools** let LLMs perform actions and **resources** provide reference data, **prompts** are _user-controlled workflows_ that orchestrate tools and resources to accomplish complex tasks.

Think of prompts as your MCP server's "quick actions"—common workflows that users can invoke with minimal input.

## Quick Start: Your First Prompt (15 lines)

Let's create a simple code review prompt:

```rust
use pmcp::{Server, SyncPrompt, types::{GetPromptResult, PromptMessage, Role, MessageContent}};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let code_review = SyncPrompt::new("code-review", |args| {
        let code = args.get("code").and_then(|v| v.as_str())
            .ok_or_else(|| pmcp::Error::validation("'code' required"))?;

        Ok(GetPromptResult::new(
            vec![
                PromptMessage {
                    role: Role::System,
                    content: MessageContent::Text {
                        text: "You are an expert code reviewer. Provide constructive feedback.".to_string(),
                    },
                },
                PromptMessage {
                    role: Role::User,
                    content: MessageContent::Text {
                        text: format!("Please review this code:\n\n```\n{}\n```", code),
                    },
                },
            ],
            Some("Code review prompt".to_string()),
        })
    })
    .with_description("Generate a code review prompt")
    .with_argument("code", "Code to review", true);

    Server::builder().prompt("code-review", code_review).build()?.run_stdio().await
}
```

**Test it:**
```bash
# Start server
cargo run

# In another terminal:
mcp-tester test stdio --list-prompts
# Shows: code-review

mcp-tester test stdio --get-prompt "code-review" '{"code": "fn main() {}"}'
# Returns structured prompt messages
```

You've created, registered, and tested an MCP prompt! Now let's understand how it works.

## The Prompt Analogy: Website for Agents

Continuing the website analogy from Chapter 4, prompts are your "homepage CTAs" (calls-to-action) for agents.

| Website Element | MCP Prompt | Purpose |
| --- | --- | --- |
| "Get Started" button | Simple prompt | Quick access to common workflow |
| Multi-step wizard | Workflow prompt | Guided multi-tool orchestration |
| Template forms | Prompt with arguments | Pre-filled workflows with user input |
| Help tooltips | Prompt descriptions | Explain what the prompt does |
| Form validation | Argument validation | Ensure user provides correct inputs |

**Key insight**: Prompts are **user-triggered**, not auto-executed by the LLM. They appear in the client UI for users to select explicitly.

## Why Prompts Matter for LLMs

LLMs driving MCP clients benefit from prompts in several ways:

1. **User Intent Clarity**: User selects "Generate weekly report" → LLM knows exact workflow
2. **Tool Orchestration**: Prompt defines sequence (fetch data → calculate → format → save)
3. **Context Pre-loading**: Prompt includes system instructions and resource references
4. **Argument Guidance**: User provides structured inputs (date range, format, recipients)
5. **Consistent Results**: Same prompt + same inputs = predictable workflow execution

**Example workflow:**
```
User action: Selects "Generate weekly report" prompt in Claude Desktop
           ↓
Client calls: prompts/get with arguments {start_date, end_date, format}
           ↓
Server returns: Structured messages with:
  - System instructions (how to generate report)
  - Resource references (templates, previous reports)
  - Tool orchestration (which tools to call in what order)
           ↓
LLM executes: Follows instructions, calls tools, produces report
```

Without prompts, users would need to manually describe the entire workflow every time.

## Prompt Anatomy: Step-by-Step

Every prompt follows this anatomy:
1. **Name + Description** → What the prompt does
2. **Arguments** → User inputs (required vs optional)
3. **Messages** → Structured conversation (System, User, Assistant)
4. **Message Content Types** → Text, Image, or Resource references
5. **Add to Server** → Register and test

Let's build a comprehensive blog post generator following this pattern.

### Step 1: Name + Description

```rust
/// Prompt name: "blog-post"
/// Description: "Generate a complete blog post on any topic.
///              Includes title, introduction, main sections, and conclusion.
///              Supports different writing styles and lengths."
```

**Naming best practices:**
- Use kebab-case: `blog-post`, `weekly-report`, `code-review`
- Descriptive and action-oriented: `generate-blog-post` not `blog`
- User-facing: Users see these names in UI dropdowns

### Step 2: Arguments (with Defaults)

Define required and optional arguments:

```rust
use std::collections::HashMap;

/// Arguments for blog post prompt
///
/// The function receives args as HashMap<String, String>
fn get_arguments(args: &HashMap<String, String>) -> pmcp::Result<(String, String, String)> {
    // Required argument
    let topic = args.get("topic")
        .ok_or_else(|| pmcp::Error::validation(
            "Missing required argument 'topic'. \
             Example: {topic: 'Rust async programming'}"
        ))?;

    // Optional arguments with defaults
    let style = args.get("style")
        .map(|s| s.as_str())
        .unwrap_or("professional");

    let length = args.get("length")
        .map(|s| s.as_str())
        .unwrap_or("medium");

    // Validate style
    if !matches!(style, "professional" | "casual" | "technical") {
        return Err(pmcp::Error::validation(format!(
            "Invalid style '{}'. Must be: professional, casual, or technical",
            style
        )));
    }

    // Validate length
    if !matches!(length, "short" | "medium" | "long") {
        return Err(pmcp::Error::validation(format!(
            "Invalid length '{}'. Must be: short (500w), medium (1000w), or long (2000w)",
            length
        )));
    }

    Ok((topic.to_string(), style.to_string(), length.to_string()))
}
```

**Argument patterns:**
- ✅ **Required**: Must provide (e.g., topic, customer_id)
- ✅ **Optional with defaults**: Fallback if not provided (e.g., style, length)
- ✅ **Validation**: Check values before use
- ✅ **Clear errors**: Tell user exactly what's wrong

### Step 3: Messages (Structured Conversation)

Create a structured conversation with different roles:

```rust
use pmcp::types::{PromptMessage, Role, MessageContent};

fn build_messages(topic: &str, style: &str, length: &str) -> Vec<PromptMessage> {
    vec![
        // System message: Instructions to the LLM
        PromptMessage {
            role: Role::System,
            content: MessageContent::Text {
                text: format!(
                    "You are a professional blog post writer.\n\
                     \n\
                     TASK: Write a {} {} blog post about: {}\n\
                     \n\
                     WORKFLOW:\n\
                     1. Create an engaging title\n\
                     2. Write a compelling introduction\n\
                     3. Develop 3-5 main sections with examples\n\
                     4. Conclude with key takeaways\n\
                     \n\
                     STYLE GUIDE:\n\
                     - Professional: Formal tone, industry terminology\n\
                     - Casual: Conversational, relatable examples\n\
                     - Technical: Deep dives, code examples, references\n\
                     \n\
                     LENGTH TARGETS:\n\
                     - Short: ~500 words (quick overview)\n\
                     - Medium: ~1000 words (balanced coverage)\n\
                     - Long: ~2000 words (comprehensive guide)\n\
                     \n\
                     FORMAT: Use Markdown with proper headings (# ## ###)",
                    length, style, topic
                ),
            },
        },

        // Assistant message: Provide context or resources
        PromptMessage {
            role: Role::Assistant,
            content: MessageContent::Resource {
                uri: format!("resource://blog/style-guide/{}", style),
                text: None,
                mime_type: Some("text/markdown".to_string()),
            },
        },

        // User message: The actual request
        PromptMessage {
            role: Role::User,
            content: MessageContent::Text {
                text: format!(
                    "Please write a {} {} blog post about: {}",
                    length, style, topic
                ),
            },
        },
    ]
}
```

**Message roles explained:**
- **System**: Instructions for LLM behavior (tone, format, workflow)
- **Assistant**: Context, resources, or examples
- **User**: The user's actual request (with argument placeholders)

**Why this structure works:**
1. **Clear expectations**: System message defines workflow steps
2. **Resource integration**: Assistant provides style guides
3. **User intent**: User message is concise and clear

### Step 4: Message Content Types

PMCP supports three content types for messages:

#### Text Content (Most Common)

```rust
MessageContent::Text {
    text: "Your text here".to_string(),
}
```

Use for: Instructions, user requests, explanations

#### Image Content

```rust
MessageContent::Image {
    data: base64_encoded_image, // Vec<u8> base64-encoded
    mime_type: "image/png".to_string(),
}
```

Use for: Visual references, diagrams, screenshots, design mockups

**Example:**
```rust
let logo_bytes = include_bytes!("../assets/logo.png");
let logo_base64 = base64::encode(logo_bytes);

PromptMessage {
    role: Role::Assistant,
    content: MessageContent::Image {
        data: logo_base64,
        mime_type: "image/png".to_string(),
    },
}
```

#### Resource References

```rust
MessageContent::Resource {
    uri: "resource://app/documentation".to_string(),
    text: None, // Optional inline preview
    mime_type: Some("text/markdown".to_string()),
}
```

Use for: Documentation, configuration files, templates, policies

**Why resource references are powerful:**
- ❌ **Bad**: Embed 5000 lines of API docs in prompt text
- ✅ **Good**: Reference `resource://api/documentation` — LLM fetches only if needed
- **Benefit**: Smaller prompts, on-demand context loading

### Step 5: Complete Prompt Implementation

Putting it all together with `SyncPrompt`:

```rust
use pmcp::{SyncPrompt, types::{GetPromptResult, PromptMessage, Role, MessageContent}};
use std::collections::HashMap;

fn create_blog_post_prompt() -> SyncPrompt<
    impl Fn(HashMap<String, String>) -> pmcp::Result<GetPromptResult> + Send + Sync
> {
    SyncPrompt::new("blog-post", |args| {
        // Step 1: Parse and validate arguments
        let topic = args.get("topic")
            .ok_or_else(|| pmcp::Error::validation("'topic' required"))?;
        let style = args.get("style").map(|s| s.as_str()).unwrap_or("professional");
        let length = args.get("length").map(|s| s.as_str()).unwrap_or("medium");

        // Validate values
        if !matches!(style, "professional" | "casual" | "technical") {
            return Err(pmcp::Error::validation(format!(
                "Invalid style '{}'. Use: professional, casual, or technical",
                style
            )));
        }

        // Step 2: Build messages
        let messages = vec![
            // System: Workflow instructions
            PromptMessage {
                role: Role::System,
                content: MessageContent::Text {
                    text: format!(
                        "You are a {} blog post writer. Write a {} post about: {}\n\
                         \n\
                         STRUCTURE:\n\
                         1. Title (# heading)\n\
                         2. Introduction (hook + overview)\n\
                         3. Main sections (## headings)\n\
                         4. Conclusion (key takeaways)\n\
                         \n\
                         LENGTH: {} (~{} words)",
                        style,
                        style,
                        topic,
                        length,
                        match length {
                            "short" => "500",
                            "long" => "2000",
                            _ => "1000",
                        }
                    ),
                },
            },

            // Assistant: Style guide resource
            PromptMessage {
                role: Role::Assistant,
                content: MessageContent::Resource {
                    uri: format!("resource://blog/style-guide/{}", style),
                    text: None,
                    mime_type: Some("text/markdown".to_string()),
                },
            },

            // User: The request
            PromptMessage {
                role: Role::User,
                content: MessageContent::Text {
                    text: format!("Write a {} {} blog post about: {}", length, style, topic),
                },
            },
        ];

        Ok(GetPromptResult::new(
            messages,
            Some(format!("Generate {} blog post about {}", style, topic)),
        })
    })
    .with_description("Generate a complete blog post on any topic")
    .with_argument("topic", "The topic to write about", true)
    .with_argument("style", "Writing style: professional, casual, technical", false)
    .with_argument("length", "Post length: short, medium, long", false)
}
```

**Key components:**
1. **Closure captures arguments**: `|args| { ... }`
2. **Validation**: Check required fields and values
3. **Message construction**: System → Assistant → User
4. **Metadata**: Description helps users understand the prompt
5. **Argument definitions**: Shown in discovery (`prompts/list`)

### Step 6: Add to Server

```rust
use pmcp::Server;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let blog_prompt = create_blog_post_prompt();

    let server = Server::builder()
        .name("content-server")
        .version("1.0.0")
        .prompt("blog-post", blog_prompt)
        // Add the tools this prompt might use:
        // .tool("search_resources", /* ... */)
        // .tool("generate_outline", /* ... */)
        .build()?;

    // Test with: mcp-tester test stdio --get-prompt "blog-post" '{"topic":"Rust"}'

    server.run_stdio().await
}
```

## Simple Text Prompts: Common Patterns

For most use cases, simple text-based prompts with `SyncPrompt` are sufficient.

### Pattern 1: Code Review Prompt

```rust
use pmcp::{SyncPrompt, types::{GetPromptResult, PromptMessage, Role, MessageContent}};
use std::collections::HashMap;

fn create_code_review_prompt() -> SyncPrompt<
    impl Fn(HashMap<String, String>) -> pmcp::Result<GetPromptResult> + Send + Sync
> {
    SyncPrompt::new("code-review", |args| {
        let code = args.get("code")
            .ok_or_else(|| pmcp::Error::validation("'code' required"))?;
        let language = args.get("language")
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        let focus = args.get("focus")
            .map(|s| s.as_str())
            .unwrap_or("general");

        Ok(GetPromptResult::new(
            vec![
                PromptMessage {
                    role: Role::System,
                    content: MessageContent::Text {
                        text: format!(
                            "You are an expert {} code reviewer. Focus on {} aspects.\n\
                             Provide constructive feedback with specific suggestions.",
                            language, focus
                        ),
                    },
                },
                PromptMessage {
                    role: Role::User,
                    content: MessageContent::Text {
                        text: format!("Review this {} code:\n\n```{}\n{}\n```", language, language, code),
                    },
                },
            ],
            Some(format!("Code review for {} focusing on {}", language, focus)),
        })
    })
    .with_description("Generate a code review prompt")
    .with_argument("code", "Code to review", true)
    .with_argument("language", "Programming language", false)
    .with_argument("focus", "Focus area: performance, security, style", false)
}
```

### Pattern 2: Documentation Generator

```rust
fn create_docs_prompt() -> SyncPrompt<
    impl Fn(HashMap<String, String>) -> pmcp::Result<GetPromptResult> + Send + Sync
> {
    SyncPrompt::new("generate-docs", |args| {
        let code = args.get("code")
            .ok_or_else(|| pmcp::Error::validation("'code' required"))?;
        let format = args.get("format")
            .map(|s| s.as_str())
            .unwrap_or("markdown");

        if !matches!(format, "markdown" | "html" | "plaintext") {
            return Err(pmcp::Error::validation(format!(
                "Invalid format '{}'. Use: markdown, html, or plaintext",
                format
            )));
        }

        Ok(GetPromptResult::new(
            vec![
                PromptMessage {
                    role: Role::System,
                    content: MessageContent::Text {
                        text: format!(
                            "Generate comprehensive documentation in {} format.\n\
                             \n\
                             Include:\n\
                             - Function/class descriptions\n\
                             - Parameter documentation\n\
                             - Return value descriptions\n\
                             - Usage examples\n\
                             - Edge cases and error handling",
                            format
                        ),
                    },
                },
                PromptMessage {
                    role: Role::User,
                    content: MessageContent::Text {
                        text: format!("Document this code:\n\n```\n{}\n```", code),
                    },
                },
            ],
            Some("Generate code documentation".to_string()),
        })
    })
    .with_description("Generate documentation for code")
    .with_argument("code", "Code to document", true)
    .with_argument("format", "Output format: markdown, html, plaintext", false)
}
```

### Pattern 3: Task Creation Prompt

```rust
fn create_task_prompt() -> SyncPrompt<
    impl Fn(HashMap<String, String>) -> pmcp::Result<GetPromptResult> + Send + Sync
> {
    SyncPrompt::new("create-task", |args| {
        let title = args.get("title")
            .ok_or_else(|| pmcp::Error::validation("'title' required"))?;
        let project = args.get("project")
            .map(|s| s.as_str())
            .unwrap_or("default");
        let priority = args.get("priority")
            .map(|s| s.as_str())
            .unwrap_or("normal");

        Ok(GetPromptResult::new(
            vec![
                PromptMessage {
                    role: Role::System,
                    content: MessageContent::Text {
                        text: format!(
                            "Create a task in project '{}' with priority '{}'.\n\
                             \n\
                             Task format:\n\
                             - Title: Brief, actionable\n\
                             - Description: Clear context and requirements\n\
                             - Acceptance criteria: Measurable completion conditions\n\
                             - Labels: Relevant tags for categorization",
                            project, priority
                        ),
                    },
                },
                // Reference project documentation
                PromptMessage {
                    role: Role::Assistant,
                    content: MessageContent::Resource {
                        uri: format!("resource://projects/{}/guidelines", project),
                        text: None,
                        mime_type: Some("text/markdown".to_string()),
                    },
                },
                PromptMessage {
                    role: Role::User,
                    content: MessageContent::Text {
                        text: format!("Create a task: {}", title),
                    },
                },
            ],
            Some(format!("Create task in {}", project)),
        })
    })
    .with_description("Create a new task in a project")
    .with_argument("title", "Task title", true)
    .with_argument("project", "Project name", false)
    .with_argument("priority", "Priority: low, normal, high", false)
}
```

## Best Practices for Simple Prompts

### 1. Argument Validation: Fail Fast

```rust
// ❌ Bad: Silent defaults for invalid values
let priority = args.get("priority")
    .map(|s| s.as_str())
    .unwrap_or("normal"); // Silently accepts "urgnet" typo

// ✅ Good: Validate and provide clear error
let priority = args.get("priority")
    .map(|s| s.as_str())
    .unwrap_or("normal");

if !matches!(priority, "low" | "normal" | "high") {
    return Err(pmcp::Error::validation(format!(
        "Invalid priority '{}'. Must be: low, normal, or high",
        priority
    )));
}
```

### 2. System Messages: Be Specific

```rust
// ❌ Too vague
"You are a helpful assistant."

// ✅ Specific role and instructions
"You are a senior software engineer specializing in code review.\n\
 Focus on: security vulnerabilities, performance issues, and maintainability.\n\
 Provide actionable feedback with specific file/line references.\n\
 Use a constructive, educational tone."
```

### 3. Resource References: Keep Prompts Lightweight

```rust
// ❌ Bad: Embed large policy doc in prompt
PromptMessage {
    role: Role::Assistant,
    content: MessageContent::Text {
        text: five_thousand_line_policy_document, // Huge prompt!
    },
}

// ✅ Good: Reference resource (LLM fetches if needed)
PromptMessage {
    role: Role::Assistant,
    content: MessageContent::Resource {
        uri: "resource://policies/refund-policy".to_string(),
        text: None,
        mime_type: Some("text/markdown".to_string()),
    },
}
```

### 4. Argument Descriptions: Guide Users

```rust
// ❌ Vague descriptions
.with_argument("style", "The style", false)

// ✅ Clear descriptions with examples
.with_argument(
    "style",
    "Writing style (professional, casual, technical). Default: professional",
    false
)
```

### 5. Optional Arguments: Document Defaults

```rust
fn create_prompt() -> SyncPrompt<
    impl Fn(HashMap<String, String>) -> pmcp::Result<GetPromptResult> + Send + Sync
> {
    SyncPrompt::new("example", |args| {
        // Document defaults in code
        let format = args.get("format")
            .map(|s| s.as_str())
            .unwrap_or("markdown"); // Default: markdown

        let verbosity = args.get("verbosity")
            .map(|s| s.as_str())
            .unwrap_or("normal"); // Default: normal

        // ... use format and verbosity
        Ok(GetPromptResult::new(vec![], None))
    })
    // Document defaults in argument descriptions
    .with_argument("format", "Output format (markdown, html). Default: markdown", false)
    .with_argument("verbosity", "Detail level (brief, normal, verbose). Default: normal", false)
}
```

## AsyncPrompt vs SyncPrompt

Choose based on your handler's needs:

### SyncPrompt (Recommended for Most Cases)

For simple, CPU-bound prompt generation:

```rust
use pmcp::SyncPrompt;

let prompt = SyncPrompt::new("simple", |args| {
    // Synchronous logic only
    let topic = args.get("topic").unwrap_or(&"default".to_string());

    Ok(GetPromptResult::new(
        vec![
            PromptMessage {
                role: Role::System,
                content: MessageContent::Text {
                    text: format!("Talk about {}", topic),
                },
            },
        ],
        None,
    })
});
```

### SimplePrompt (Async)

For prompts that need async operations (database queries, API calls):

```rust
use pmcp::SimplePrompt;
use std::pin::Pin;
use std::future::Future;

let prompt = SimplePrompt::new("async-example", Box::new(
    |args: HashMap<String, String>, _extra: pmcp::RequestHandlerExtra| {
        Box::pin(async move {
            // Can await async operations
            let data = fetch_from_database(&args["id"]).await?;
            let template = generate_messages(&data).await?;

            Ok(GetPromptResult::new(
                template,
                Some("Generated from database".to_string()),
            })
        }) as Pin<Box<dyn Future<Output = pmcp::Result<GetPromptResult>> + Send>>
    }
));
```

**When to use which:**
- ✅ **SyncPrompt**: 95% of cases (simple message construction)
- ✅ **SimplePrompt**: Database lookups, API calls, file I/O

## Listing Prompts

Users discover prompts via `prompts/list`:

```json
{
  "method": "prompts/list"
}
```

Response:
```json
{
  "prompts": [
    {
      "name": "code-review",
      "description": "Generate a code review prompt",
      "arguments": [
        {"name": "code", "description": "Code to review", "required": true},
        {"name": "language", "description": "Programming language", "required": false},
        {"name": "focus", "description": "Focus area", "required": false}
      ]
    },
    {
      "name": "blog-post",
      "description": "Generate a complete blog post",
      "arguments": [
        {"name": "topic", "description": "Topic to write about", "required": true},
        {"name": "style", "description": "Writing style", "required": false},
        {"name": "length", "description": "Post length", "required": false}
      ]
    }
  ]
}
```

## When to Use Prompts

Use prompts when:

✅ **Users need quick access to common workflows**
- "Generate weekly report"
- "Create pull request description"
- "Review code focusing on security"

✅ **Multiple tools must be orchestrated in a specific order**
- Data analysis pipelines
- Content generation workflows
- Multi-step validation processes

✅ **You want to guide LLM behavior for specific tasks**
- "Write in executive summary style"
- "Focus on security vulnerabilities"
- "Generate tests for this function"

Don't use prompts when:

❌ **It's just a single tool call**
- Use tools directly instead

❌ **The workflow is user-specific and can't be templated**
- Let the LLM figure it out from available tools

❌ **The task changes based on dynamic runtime conditions**
- Use tools with conditional logic instead

---

## Advanced: Workflow-Based Prompts

For complex multi-tool orchestration with data flow between steps, PMCP provides a powerful workflow system. This advanced section demonstrates building sophisticated prompts that compose multiple tools.

**When to use workflows:**
- ✅ Multi-step processes with data dependencies
- ✅ Complex tool orchestration (step 2 uses output from step 1)
- ✅ Validated workflows with compile-time checks
- ✅ Reusable tool compositions

**When NOT to use workflows:**
- ❌ Simple single-message prompts (use `SyncPrompt`)
- ❌ One-off custom requests
- ❌ Highly dynamic workflows that can't be templated

### Workflow Anatomy: Quadratic Formula Solver

Let's build a workflow that solves quadratic equations (ax² + bx + c = 0) step by step.

From `examples/50_workflow_minimal.rs`:

```rust
use pmcp::server::workflow::{
    dsl::{constant, field, from_step, prompt_arg},
    InternalPromptMessage, SequentialWorkflow, ToolHandle, WorkflowStep,
};
use serde_json::json;

fn create_quadratic_solver_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "quadratic_solver",
        "Solve quadratic equations using the quadratic formula"
    )
    // Define required prompt arguments
    .argument("a", "Coefficient a (x² term)", true)
    .argument("b", "Coefficient b (x term)", true)
    .argument("c", "Coefficient c (constant term)", true)

    // Add instruction messages
    .instruction(InternalPromptMessage::system(
        "Solve the quadratic equation ax² + bx + c = 0"
    ))

    // Step 1: Calculate discriminant (b² - 4ac)
    .step(
        WorkflowStep::new("calc_discriminant", ToolHandle::new("calculator"))
            .arg("operation", constant(json!("discriminant")))
            .arg("a", prompt_arg("a"))
            .arg("b", prompt_arg("b"))
            .arg("c", prompt_arg("c"))
            .bind("discriminant") // ← Bind output as "discriminant"
    )

    // Step 2: Calculate first root
    .step(
        WorkflowStep::new("calc_root1", ToolHandle::new("calculator"))
            .arg("operation", constant(json!("quadratic_root")))
            .arg("a", prompt_arg("a"))
            .arg("b", prompt_arg("b"))
            .arg("discriminant_value", field("discriminant", "value")) // ← Reference binding
            .arg("sign", constant(json!("+")))
            .bind("root1") // ← Bind output as "root1"
    )

    // Step 3: Calculate second root
    .step(
        WorkflowStep::new("calc_root2", ToolHandle::new("calculator"))
            .arg("operation", constant(json!("quadratic_root")))
            .arg("a", prompt_arg("a"))
            .arg("b", prompt_arg("b"))
            .arg("discriminant_value", field("discriminant", "value"))
            .arg("sign", constant(json!("-")))
            .bind("root2")
    )

    // Step 4: Format the solution
    .step(
        WorkflowStep::new("format_solution", ToolHandle::new("formatter"))
            .arg("discriminant_result", from_step("discriminant")) // ← Entire output
            .arg("root1_result", from_step("root1"))
            .arg("root2_result", from_step("root2"))
            .arg("format_template", constant(json!("Solution: x = {root1} or x = {root2}")))
            .bind("formatted_solution")
    )
}
```

**Key concepts:**

1. **SequentialWorkflow**: Defines a multi-step workflow
2. **WorkflowStep**: Individual steps that call tools
3. **Bindings**: `.bind("name")` creates named outputs
4. **DSL helpers**:
   - `prompt_arg("a")` - Reference workflow argument
   - `from_step("discriminant")` - Use entire output from previous step
   - `field("discriminant", "value")` - Extract specific field from output
   - `constant(json!("value"))` - Provide constant value

5. **Data flow**: Step 2 uses output from Step 1 via bindings

### Workflow DSL: The Four Mapping Helpers

From `examples/52_workflow_dsl_cookbook.rs`:

```rust
WorkflowStep::new("step_name", ToolHandle::new("tool"))
    // 1. prompt_arg("arg_name") - Get value from workflow arguments
    .arg("input", prompt_arg("user_input"))

    // 2. constant(json!(...)) - Provide a constant value
    .arg("mode", constant(json!("auto")))
    .arg("count", constant(json!(42)))

    // 3. from_step("binding") - Get entire output from previous step
    .arg("data", from_step("result1"))

    // 4. field("binding", "field") - Get specific field from output
    .arg("style", field("result1", "recommended_style"))

    .bind("result2") // ← Create binding for this step's output
```

**Important distinction:**
- **Step name** (first arg): Identifies the step internally
- **Binding name** (via `.bind()`): How other steps reference the output
- ✅ Use **binding names** in `from_step()` and `field()`
- ❌ Don't use step names to reference outputs

### Chaining Steps with Bindings

From `examples/52_workflow_dsl_cookbook.rs`:

```rust
SequentialWorkflow::new("content-pipeline", "Multi-step content creation")
    .argument("topic", "Topic to write about", true)

    .step(
        // Step 1: Create draft
        WorkflowStep::new("create_draft", ToolHandle::new("writer"))
            .arg("topic", prompt_arg("topic"))
            .arg("format", constant(json!("markdown")))
            .bind("draft") // ← Bind as "draft"
    )

    .step(
        // Step 2: Review draft (uses output from step 1)
        WorkflowStep::new("review_draft", ToolHandle::new("reviewer"))
            .arg("content", from_step("draft")) // ← Reference "draft" binding
            .arg("criteria", constant(json!(["grammar", "clarity"])))
            .bind("review") // ← Bind as "review"
    )

    .step(
        // Step 3: Revise (uses outputs from steps 1 & 2)
        WorkflowStep::new("revise_draft", ToolHandle::new("editor"))
            .arg("original", from_step("draft")) // ← Reference "draft"
            .arg("feedback", field("review", "suggestions")) // ← Extract field from "review"
            .bind("final") // ← Bind as "final"
    )
```

**Pattern**: Each step binds its output, allowing later steps to reference it.

### Validation and Error Messages

Workflows are validated at build time. From `examples/51_workflow_error_messages.rs`:

**Common errors:**

1. **Unknown binding** - Referencing a binding that doesn't exist:
```rust
.step(
    WorkflowStep::new("create", ToolHandle::new("creator"))
        .bind("content") // ← Binds as "content"
)
.step(
    WorkflowStep::new("review", ToolHandle::new("reviewer"))
        .arg("text", from_step("draft")) // ❌ ERROR: "draft" doesn't exist
)

// Error: Unknown binding 'draft'. Available bindings: content
// Fix: Change to from_step("content")
```

2. **Undefined prompt argument** - Using an undeclared argument:
```rust
SequentialWorkflow::new("workflow", "...")
    .argument("topic", "The topic", true)
    // Missing: .argument("style", ...)
    .step(
        WorkflowStep::new("create", ToolHandle::new("creator"))
            .arg("topic", prompt_arg("topic"))
            .arg("style", prompt_arg("writing_style")) // ❌ ERROR: not declared
    )

// Error: Undefined prompt argument 'writing_style'
// Fix: Add .argument("writing_style", "Writing style", false)
```

3. **Step without binding cannot be referenced:**
```rust
.step(
    WorkflowStep::new("create", ToolHandle::new("creator"))
        .arg("topic", prompt_arg("topic"))
        // ❌ Missing: .bind("content")
)
.step(
    WorkflowStep::new("review", ToolHandle::new("reviewer"))
        .arg("text", from_step("create")) // ❌ ERROR: "create" has no binding
)

// Error: Step 'create' has no binding. Add .bind("name") to reference it.
// Fix: Add .bind("content") to first step
```

**Best practice**: Call `.validate()` early to catch errors:

```rust
let workflow = create_my_workflow();

match workflow.validate() {
    Ok(()) => println!("✅ Workflow is valid"),
    Err(e) => {
        eprintln!("❌ Validation failed: {}", e);
        // Error messages are actionable - they tell you exactly what's wrong
    }
}
```

### Understanding MCP Client Autonomy

**Critical insight**: MCP clients (LLMs like Claude) are **autonomous agents** that make their own decisions. When you return a prompt with instructions, the LLM is free to:

- ✅ Follow your instructions exactly
- ❌ Ignore your instructions entirely
- 🔀 Modify the workflow to suit its understanding
- 🌐 Call tools on **other MCP servers** instead of yours
- 🤔 Decide your workflow isn't appropriate and do something else

**This is not a bug—it's the design of MCP.** Clients have agency.

**Example: Instruction-Only Prompt (Low Compliance)**

```rust
// Traditional approach: Just return instructions
PromptMessage {
    role: Role::System,
    content: MessageContent::Text {
        text: "Follow these steps:
                1. Call list_pages to get all pages
                2. Find the best matching page for the project name
                3. Call add_journal_task with the formatted task"
    }
}
```

**What actually happens:**
- LLM might call different tools
- LLM might skip steps it thinks are unnecessary
- LLM might use tools from other MCP servers
- LLM might reorder steps based on its reasoning
- **Compliance probability: ~60-70%** (LLM decides independently)

### Server-Side Execution: Improving Workflow Compliance

PMCP's hybrid execution model **dramatically improves the probability** that clients complete your workflow as designed by:

1. **Executing deterministic steps server-side** (can't be skipped)
2. **Providing complete context** (tool results + resources)
3. **Offering clear guidance** for remaining steps
4. **Reducing client decision space** (fewer choices = higher compliance)

**From `examples/54_hybrid_workflow_execution.rs`:**

#### The Hybrid Execution Model

When a workflow prompt is invoked via `prompts/get`, the server:

1. **Executes tools server-side** for steps with resolved parameters
2. **Fetches and embeds resources** to provide context
3. **Returns conversation trace** showing what was done
4. **Hands off to client** with guidance for remaining steps

**Result**: Server has already completed deterministic steps. Client receives:
- ✅ Actual tool results (not instructions to call tools)
- ✅ Resource content (documentation, schemas, examples)
- ✅ Clear guidance for what remains
- ✅ Reduced decision space (fewer ways to go wrong)

**Compliance improvement: ~85-95%** (server did the work, client just continues)

#### Hybrid Execution Example: Logseq Task Creation

From `examples/54_hybrid_workflow_execution.rs`:

```rust
use pmcp::server::workflow::{
    SequentialWorkflow, WorkflowStep, ToolHandle, DataSource,
};

fn create_task_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "add_project_task",
        "add a task to a Logseq project with intelligent page matching"
    )
    .argument("project", "Project name (can be fuzzy match)", true)
    .argument("task", "Task description", true)

    // Step 1: Server executes (deterministic - no parameters needed)
    .step(
        WorkflowStep::new("list_pages", ToolHandle::new("list_pages"))
            .with_guidance("I'll first get all available page names from Logseq")
            .bind("pages")
    )

    // Step 2: Client continues (needs LLM reasoning for fuzzy matching)
    .step(
        WorkflowStep::new("add_task", ToolHandle::new("add_journal_task"))
            .with_guidance(
                "I'll now:\n\
                 1. Find the page name from the list above that best matches '{project}'\n\
                 2. Format the task as: [[matched-page-name]] {task}\n\
                 3. Call add_journal_task with the formatted task"
            )
            .with_resource("docs://logseq/task-format")
            .expect("Valid resource URI")
            // No .arg() mappings - server can't resolve params (needs fuzzy match)
            .bind("result")
    )
}
```

**Server execution flow:**

```
User invokes: prompts/get with {project: "MCP Tester", task: "Fix bug"}
      ↓
Server: Creates user intent message
Server: Creates assistant plan message
      ↓
Server: Executes Step 1 (list_pages)
  → Guidance: "I'll first get all available page names"
  → Calls list_pages tool
  → Result: {"page_names": ["mcp-tester", "MCP Rust SDK", "Test Page"]}
  → Stores in binding "pages"
      ↓
Server: Attempts Step 2 (add_task)
  → Guidance: "Find the page name... that matches 'MCP Tester'"
  → Fetches resource: docs://logseq/task-format
  → Embeds content: "Task Format Guide: Use [[page-name]]..."
  → Checks params: Missing (needs fuzzy match - can't resolve deterministically)
  → STOPS (graceful handoff)
      ↓
Server: Returns conversation trace to client
```

**Conversation trace returned to client:**

```
Message 1 (User):
  "I want to add a task to a Logseq project with intelligent page matching.
   Parameters:
     - project: "MCP Tester"
     - task: "Fix bug"

Message 2 (Assistant):
  "Here's my plan:
   1. list_pages - List all available pages
   2. add_journal_task - Add a task to a journal"

Message 3 (Assistant):  [Guidance for step 1]
  "I'll first get all available page names from Logseq"

Message 4 (Assistant):  [Tool call announcement]
  "Calling tool 'list_pages' with parameters: {}"

Message 5 (User):  [Tool result - ACTUAL DATA]
  "Tool result:
   {"page_names": ["mcp-tester", "MCP Rust SDK", "Test Page"]}"

Message 6 (Assistant):  [Guidance for step 2 - with argument substitution]
  "I'll now:
   1. Find the page name from the list above that best matches 'MCP Tester'
   2. Format the task as: [[matched-page-name]] Fix bug
   3. Call add_journal_task with the formatted task"

Message 7 (User):  [Resource content - DOCUMENTATION]
  "Resource content from docs://logseq/task-format:
   Task Format Guide:
   - Use [[page-name]] for links
   - Add TASK prefix for action items
   - Use TODAY for current date"

[Server stops - hands off to client with complete context]
```

**Client LLM receives:**
- ✅ Page list (actual data, not instruction to fetch it)
- ✅ Clear 3-step guidance (what to do next)
- ✅ Task format documentation (how to format)
- ✅ User's original intent (project + task)

**Probability client completes correctly: ~90%**

The client:
- Can't skip step 1 (server already did it)
- Has exact data to work with (page list)
- Has clear instructions (3 steps)
- Has documentation (format guide)
- Has fewer decisions to make (just fuzzy match + format + call)

#### Workflow Methods for Hybrid Execution

**`.with_guidance(text)`** - Assistant message explaining what this step should do

```rust
.step(
    WorkflowStep::new("match", ToolHandle::new("add_task"))
        .with_guidance(
            "Find the page matching '{project}' in the list above. \
             If no exact match, use fuzzy matching for the closest name."
        )
        .bind("result")
)
```

**Features:**
- Rendered as assistant message in conversation trace
- Supports `{arg_name}` substitution (replaced with actual argument values)
- Shown even if server successfully executes the step
- Critical for graceful handoff when server can't resolve parameters

**`.with_resource(uri)`** - Fetches resource and embeds content as user message

```rust
.step(
    WorkflowStep::new("add_task", ToolHandle::new("add_journal_task"))
        .with_guidance("Format the task according to the guide")
        .with_resource("docs://logseq/task-format")
        .expect("Valid resource URI")
        .with_resource("docs://logseq/examples")
        .expect("Valid resource URI")
        .arg("task", DataSource::prompt_arg("task"))
)
```

**Features:**
- Server fetches resource during workflow execution
- Content embedded as user message before step execution
- Multiple resources supported (call `.with_resource()` multiple times)
- Provides context for client LLM decision-making
- Reduces hallucination (client has actual docs, not assumptions)

#### When Server Executes vs Hands Off

**Server executes step completely if:**
- ✅ All required tool parameters can be resolved from:
  - Prompt arguments (via `prompt_arg("name")`)
  - Previous step bindings (via `from_step("binding")` or `field("binding", "field")`)
  - Constants (via `constant(json!(...))`)
- ✅ Tool schema's required fields are satisfied
- ✅ No errors during tool execution

**Server stops gracefully (hands off to client) if:**
- ❌ Tool requires parameters not available deterministically
- ❌ LLM reasoning needed (fuzzy matching, context interpretation, decisions)
- ❌ Parameters can't be resolved from available sources

**On graceful handoff, server includes:**
- All guidance messages (what to do next)
- All resource content (documentation, schemas, examples)
- All previous tool results (via bindings in conversation trace)
- Clear state of what was completed vs what remains

#### Why This Improves Compliance

**Traditional prompt-only approach:**

```
Prompt: "1. Call list_pages, 2. Match project, 3. Call add_task"
        ↓
Client decides: Should I follow this? Let me think...
  - Maybe I should search first?
  - Maybe the user wants something else?
  - What if I use a different tool?
  - Should I call another server?
        ↓
Compliance: ~60-70% (high variance)
```

**Hybrid execution approach:**

```
Prompt execution returns:
  - Step 1 DONE (here's the actual page list)
  - Step 2 guidance (match from THIS list)
  - Resource content (here's the format docs)
        ↓
Client sees: Half the work is done, I just need to:
  1. Match "MCP Tester" to one of: ["mcp-tester", "MCP Rust SDK", "Test Page"]
  2. Format using the provided guide
  3. Call add_journal_task
        ↓
Compliance: ~85-95% (low variance)
```

**Key improvements:**
- ✅ **Reduced decision space**: Client has fewer choices
- ✅ **Concrete data**: Actual tool results, not instructions
- ✅ **Clear next steps**: Guidance is specific to current state
- ✅ **Documentation provided**: No need to guess formatting
- ✅ **Partial completion**: Can't skip server-executed steps
- ✅ **Lower cognitive load**: Less for LLM to figure out

#### Argument Substitution in Guidance

Guidance supports `{arg_name}` placeholders that are replaced with actual argument values:

```rust
.step(
    WorkflowStep::new("process", ToolHandle::new("processor"))
        .with_guidance(
            "Process the user's request for '{topic}' in '{style}' style. \
             Use the examples from the resource to match the tone."
        )
        .with_resource("docs://style-guides/{style}")
        .expect("Valid URI")
)
```

**At runtime** with `{topic: "Rust async", style: "casual"}`:

```
Guidance rendered as:
  "Process the user's request for 'Rust async' in 'casual' style.
   Use the examples from the resource to match the tone."

Resource URI becomes:
  "docs://style-guides/casual"
```

**Benefits:**
- Guidance is specific to user's input
- Client sees exact values it should work with
- Reduces ambiguity (not "the topic" but "Rust async")

### Registering Workflows as Prompts

Use `.prompt_workflow()` to register and validate workflows. When invoked via `prompts/get`, the workflow executes server-side and returns a conversation trace:

```rust
use pmcp::Server;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let workflow = create_task_workflow();

    let server = Server::builder()
        .name("logseq-server")
        .version("1.0.0")

        // Register tools that the workflow uses
        .tool("list_pages", list_pages_tool)
        .tool("add_journal_task", add_task_tool)

        // Register resources for .with_resource() to fetch
        .resources(LogseqDocsHandler)

        // Register workflow as prompt (validates automatically)
        .prompt_workflow(workflow)?

        .build()?;

    server.run_stdio().await
}
```

**What happens when user invokes the prompt:**

1. **Registration time** (`.prompt_workflow()`):
   - Validates workflow (bindings, arguments, tool references exist)
   - Registers as prompt (discoverable via `prompts/list`)
   - Returns error if validation fails

2. **Invocation time** (`prompts/get`):
   - User calls with arguments: `{project: "MCP Tester", task: "Fix bug"}`
   - Server executes workflow steps with resolved parameters
   - Server calls tools, fetches resources, builds conversation trace
   - Server stops when parameters can't be resolved (graceful handoff)
   - Server returns **conversation trace** (not just instructions)

3. **Client receives:**
   - User intent message (what user wants)
   - Assistant plan message (workflow steps)
   - Tool execution results (actual data from server-side calls)
   - Resource content (embedded documentation)
   - Guidance messages (what to do next)
   - Complete context to continue or review

**Key insight**: The workflow is **executed**, not just described. Client receives results, not instructions.

### Integration with Typed Tools

From `examples/53_typed_tools_workflow_integration.rs`:

Workflows integrate seamlessly with typed tools:

```rust
use pmcp::Server;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Typed tool input
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct AnalyzeCodeInput {
    code: String,
    language: String,
    depth: u8,
}

async fn analyze_code(input: AnalyzeCodeInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Implementation
    Ok(json!({
        "analysis": "...",
        "issues_found": 3
    }))
}

// Workflow that uses the typed tool
fn create_code_review_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new("code_review", "Review code comprehensively")
        .argument("code", "Source code", true)
        .argument("language", "Programming language", false)

        .step(
            WorkflowStep::new("analyze", ToolHandle::new("analyze_code"))
                .arg("code", prompt_arg("code"))
                .arg("language", prompt_arg("language"))
                .arg("depth", constant(json!(2)))
                .bind("analysis")
        )
        // ... more steps
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    Server::builder()
        .name("code-server")
        .version("1.0.0")
        // Register typed tool (automatic schema generation)
        .tool_typed("analyze_code", analyze_code)
        // Register workflow that references the tool
        .prompt_workflow(create_code_review_workflow())?
        .build()?
        .run_stdio()
        .await
}
```

**Benefits:**
- ✅ Type-safe tool inputs (compile-time checked)
- ✅ Automatic JSON schema generation
- ✅ Workflow validates tool references exist
- ✅ Single source of truth for tool definitions

### Workflow Best Practices

1. **Use descriptive binding names**:
```rust
// ❌ Bad: Unclear
.bind("r1")
.bind("out")

// ✅ Good: Clear purpose
.bind("analysis_result")
.bind("formatted_output")
```

2. **Declare all arguments before using**:
```rust
SequentialWorkflow::new("workflow", "...")
    // ✅ Declare all arguments first
    .argument("topic", "Topic", true)
    .argument("style", "Style", false)
    .argument("length", "Length", false)
    // Then use them in steps
    .step(...)
```

3. **Add `.bind()` only when output is needed**:
```rust
.step(
    WorkflowStep::new("log", ToolHandle::new("logger"))
        .arg("message", from_step("result"))
        // No .bind() - logging is a side-effect, output not needed
)
```

4. **Use `field()` to extract specific data**:
```rust
// ❌ Bad: Pass entire large object
.arg("data", from_step("analysis")) // Entire analysis result

// ✅ Good: Extract only what's needed
.arg("summary", field("analysis", "summary"))
.arg("score", field("analysis", "confidence_score"))
```

5. **Validate workflows early**:
```rust
let workflow = create_my_workflow();
workflow.validate()?; // ← Catch errors before registration
```

6. **Use guidance for steps requiring LLM reasoning**:
```rust
// ✅ Good: Clear guidance for non-deterministic steps
.step(
    WorkflowStep::new("match", ToolHandle::new("add_task"))
        .with_guidance(
            "Find the best matching page from the list above. \
             Consider: exact matches > fuzzy matches > semantic similarity."
        )
        // No .arg() mappings - server will hand off to client
)

// ❌ Bad: No guidance for complex reasoning step
.step(
    WorkflowStep::new("match", ToolHandle::new("add_task"))
        // Client has to guess what to do
)
```

7. **Embed resources for context-heavy steps**:
```rust
// ✅ Good: Provide documentation for formatting/styling
.step(
    WorkflowStep::new("format", ToolHandle::new("formatter"))
        .with_guidance("Format according to the style guide")
        .with_resource("docs://formatting/style-guide")
        .expect("Valid URI")
        .with_resource("docs://formatting/examples")
        .expect("Valid URI")
)

// ❌ Bad: Expect LLM to know complex formatting rules
.step(
    WorkflowStep::new("format", ToolHandle::new("formatter"))
        .with_guidance("Format the output properly")
        // No resources - LLM will hallucinate formatting rules
)
```

8. **Design for hybrid execution - maximize server-side work**:
```rust
// ✅ Good: Server does deterministic work, client does reasoning
.step(
    WorkflowStep::new("fetch_data", ToolHandle::new("database_query"))
        .arg("query", constant(json!("SELECT * FROM pages")))
        .bind("all_pages") // ← Server executes this
)
.step(
    WorkflowStep::new("select_page", ToolHandle::new("update_page"))
        .with_guidance("Choose the most relevant page from the list")
        // ← Client does reasoning with server-provided data
)

// ❌ Bad: Client has to do all the work
.step(
    WorkflowStep::new("do_everything", ToolHandle::new("complex_tool"))
        .with_guidance(
            "1. Query the database for pages\n\
             2. Filter by relevance\n\
             3. Select the best match\n\
             4. Update the page"
        )
        // Server does nothing - just instructions
)
```

### When to Use Workflows vs Simple Prompts

| Feature | Simple Prompt (`SyncPrompt`) | Workflow (`SequentialWorkflow`) |
|---------|----------------------------|--------------------------------|
| **Use case** | Single-message prompts | Multi-step tool orchestration |
| **Execution** | Returns instructions only | Executes tools server-side |
| **Complexity** | Simple | Moderate to complex |
| **Tool composition** | LLM decides | Pre-defined sequence |
| **Data flow** | None | Explicit bindings |
| **Validation** | Argument checks | Full workflow validation |
| **Compliance** | ~60-70% (LLM decides) | ~85-95% (server guides) |
| **Resource embedding** | Manual references | Automatic fetch & embed |
| **Examples** | Code review, blog post generation | Logseq task creation, data pipelines |

**Decision guide:**
- ✅ Use **simple prompts** for: One-shot requests, LLM-driven tool selection, no tool execution needed
- ✅ Use **workflows** for: Multi-step processes, high compliance requirements, data dependencies, hybrid execution

---

## Testing Prompts

### Unit Testing Simple Prompts

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_code_review_prompt() {
        let prompt = create_code_review_prompt();

        let mut args = HashMap::new();
        args.insert("code".to_string(), "fn test() {}".to_string());
        args.insert("language".to_string(), "rust".to_string());

        let result = prompt.handle(args, RequestHandlerExtra::default()).await;

        assert!(result.is_ok());
        let prompt_result = result.unwrap();
        assert_eq!(prompt_result.messages.len(), 2);
        assert!(matches!(prompt_result.messages[0].role, Role::System));
        assert!(matches!(prompt_result.messages[1].role, Role::User));
    }

    #[tokio::test]
    async fn test_missing_required_argument() {
        let prompt = create_code_review_prompt();

        let args = HashMap::new(); // Missing "code"

        let result = prompt.handle(args, RequestHandlerExtra::default()).await;
        assert!(result.is_err());
    }
}
```

### Testing Workflows

```rust
#[test]
fn test_workflow_validation() {
    let workflow = create_quadratic_solver_workflow();

    // Workflow should validate successfully
    assert!(workflow.validate().is_ok());

    // Check arguments
    assert_eq!(workflow.arguments().len(), 3);
    assert!(workflow.arguments().contains_key(&"a".into()));

    // Check steps
    assert_eq!(workflow.steps().len(), 4);

    // Check bindings
    let bindings = workflow.output_bindings();
    assert!(bindings.contains(&"discriminant".into()));
    assert!(bindings.contains(&"root1".into()));
}
```

### Integration Testing with mcp-tester

```bash
# List all prompts
mcp-tester test stdio --list-prompts

# Get specific prompt
mcp-tester test stdio --get-prompt "code-review" '{"code": "fn main() {}", "language": "rust"}'

# Get workflow prompt
mcp-tester test stdio --get-prompt "quadratic_solver" '{"a": 1, "b": -3, "c": 2}'
```

## Summary

Prompts are user-triggered workflows that orchestrate tools and resources. PMCP provides two approaches:

**Simple Prompts (`SyncPrompt`):**
- ✅ Quick message templates with arguments
- ✅ Minimal boilerplate
- ✅ Perfect for single-message prompts
- ✅ Returns instructions for LLM to follow (~60-70% compliance)
- ✅ User provides inputs, LLM decides tool usage and execution order

**Workflow Prompts (`SequentialWorkflow`):**
- ✅ Multi-step tool orchestration with server-side execution
- ✅ Executes deterministic steps during `prompts/get`
- ✅ Returns conversation trace (tool results + resources + guidance)
- ✅ Hybrid execution: server does work, client continues with context
- ✅ Explicit data flow with bindings
- ✅ Compile-time validation
- ✅ High compliance (~85-95% - server guides client)
- ✅ Automatic resource fetching and embedding

**Understanding MCP Client Autonomy:**
- MCP clients (LLMs) are autonomous agents - they can follow, ignore, or modify your instructions
- They can call tools on other MCP servers instead of yours
- Traditional instruction-only prompts have ~60-70% compliance
- Hybrid execution with server-side tool execution + resources + guidance improves compliance to ~85-95%
- Server does deterministic work, reducing client decision space and increasing predictability

**Key takeaways:**
1. Start with `SyncPrompt` for simple instruction-only prompts
2. Use workflows when you need high compliance and multi-step orchestration
3. Design workflows for hybrid execution: server executes what it can, client continues with guidance
4. Use `.with_guidance()` for steps requiring LLM reasoning
5. Use `.with_resource()` to embed documentation and reduce hallucination
6. Validate arguments thoroughly and workflows early
7. Test with `mcp-tester` and unit tests
8. Remember: Higher server-side execution = higher client compliance

**Next chapters:**
- **Chapter 8**: Error Handling & Recovery
- **Chapter 9**: Integration Patterns

Prompts + Tools + Resources = complete MCP server. You now understand how to provide user-triggered workflows that make your server easy and efficient to use.
