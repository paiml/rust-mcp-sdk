//! Hybrid Workflow Execution Example
//!
//! This example demonstrates the hybrid execution model where the server executes
//! deterministic steps and provides structured guidance for steps requiring LLM reasoning.
//!
//! # Use Case: Logseq Task Management with Fuzzy Matching
//!
//! The workflow adds a task to a Logseq project, but the user's input may not
//! match the exact project name. The server:
//! 1. Lists all available pages (deterministic API call)
//! 2. Fetches and embeds task formatting documentation (resource)
//! 3. Provides guidance for the client LLM to fuzzy-match the project name
//! 4. Client LLM continues by matching "MCP Tester" to "mcp-tester" and formatting the task
//!
//! ## Key Concepts
//!
//! - **Server-side execution**: Deterministic operations (data fetching)
//! - **Client-side reasoning**: Fuzzy matching, context-aware decisions
//! - **Graceful handoff**: Server stops when it can't proceed, returns partial trace
//! - **Guidance messages**: Help LLM understand what to do next
//! - **Argument substitution**: `{arg_name}` in guidance → actual values

use async_trait::async_trait;
use pmcp::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};
use pmcp::{
    Content, ListResourcesResult, ReadResourceResult, ResourceHandler, Result, Server, SimpleTool,
};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Hybrid Workflow Execution: Logseq Task Management ===\n");

    // Define workflow with hybrid execution
    let workflow = create_logseq_task_workflow();

    // Create mock Logseq tools
    let list_pages_tool = SimpleTool::new("list_pages", |_args, _extra| {
        Box::pin(async move {
            Ok(json!({
                "page_names": [
                    "mcp-tester",
                    "MCP Rust SDK",
                    "Test Page",
                    "Documentation",
                    "rust-projects"
                ]
            }))
        })
    })
    .with_description("List all pages in Logseq knowledge base")
    .with_schema(json!({
        "type": "object",
        "properties": {},
        "required": []
    }));

    let add_task_tool = SimpleTool::new("add_journal_task", |args, _extra| {
        Box::pin(async move {
            let task = args
                .get("formatted_task")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Ok(json!({
                "success": true,
                "task_id": "task-123",
                "task": task,
                "created_at": "2025-10-03T10:30:00Z"
            }))
        })
    })
    .with_description("Add a task to Logseq journal")
    .with_schema(json!({
        "type": "object",
        "properties": {
            "formatted_task": {"type": "string"}
        },
        "required": ["formatted_task"]
    }));

    // Create mock resource handler for task formatting documentation
    struct LogseqDocsHandler;

    #[async_trait]
    impl ResourceHandler for LogseqDocsHandler {
        async fn read(
            &self,
            uri: &str,
            _extra: pmcp::RequestHandlerExtra,
        ) -> Result<ReadResourceResult> {
            match uri {
                "docs://logseq/task-format" => Ok(ReadResourceResult::new(vec![Content::Text {
                    text: r#"Logseq Task Formatting Guide
================================

Task Format:
- Use [[page-name]] to link to pages
- Add TODO prefix for tasks
- Format: TODO [[page-name]] Task description

Examples:
- TODO [[mcp-tester]] Fix workflow execution bug
- TODO [[rust-projects]] Update documentation
- TODO [[Documentation]] Add examples

Best Practices:
- Always use lowercase, hyphenated page names
- Be specific in task descriptions
- Link to the relevant project page
"#
                    .to_string(),
                }])),
                _ => Err(pmcp::Error::validation(format!(
                    "Unknown resource: {}",
                    uri
                ))),
            }
        }

        async fn list(
            &self,
            _cursor: Option<String>,
            _extra: pmcp::RequestHandlerExtra,
        ) -> Result<ListResourcesResult> {
            Ok(ListResourcesResult::new(vec![pmcp::ResourceInfo {
                uri: "docs://logseq/task-format".to_string(),
                name: "Logseq Task Formatting Guide".to_string(),
                description: Some("Guide for formatting tasks in Logseq".to_string()),
                mime_type: Some("text/plain".to_string()),
                meta: None,
            }]))
        }
    }

    // Build server with workflow
    let server = Server::builder()
        .name("logseq-task-server")
        .version("1.0.0")
        .tool("list_pages", list_pages_tool)
        .tool("add_journal_task", add_task_tool)
        .resources(LogseqDocsHandler)
        .prompt_workflow(workflow)?
        .build()?;

    println!("✓ Server built successfully\n");

    // =================================================================
    // Demonstrate Hybrid Execution
    // =================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📝 User Invokes Workflow");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("User types:");
    println!("  /add_project_task \"MCP Tester\" \"Fix workflow execution bug\"\n");

    println!("Note: User typed \"MCP Tester\" (with capitals and space)");
    println!("      But the actual page name is \"mcp-tester\" (lowercase, hyphenated)\n");

    // Get the workflow prompt handler
    let prompt_handler = server
        .get_prompt("add_project_task")
        .expect("Workflow should be registered");

    let mut args = std::collections::HashMap::new();
    args.insert("project".to_string(), "MCP Tester".to_string());
    args.insert("task".to_string(), "Fix workflow execution bug".to_string());

    let extra = pmcp::server::cancellation::RequestHandlerExtra::new(
        "demo-request".to_string(),
        Default::default(),
    );

    // Execute workflow (hybrid execution)
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("⚙️  Server Executing Workflow (Hybrid Mode)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let result = prompt_handler
        .handle(args, extra)
        .await
        .expect("Workflow execution should succeed");

    println!(
        "Server executed what it could and returned {} messages\n",
        result.messages.len()
    );

    // =================================================================
    // Display Conversation Trace
    // =================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("💬 Conversation Trace (Returned to Client LLM)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    for (i, msg) in result.messages.iter().enumerate() {
        println!("Message {} [{:?}]:", i + 1, msg.role);
        if let pmcp::types::MessageContent::Text { text } = &msg.content {
            println!("{}\n", text);
        }
    }

    // =================================================================
    // Explain What Happens Next
    // =================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🤖 Client LLM Continues Execution");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("The client LLM receives the conversation trace above and:");
    println!("1. Sees the list of available pages");
    println!("2. Reads the guidance on what to do next");
    println!("3. Matches \"MCP Tester\" to \"mcp-tester\" (fuzzy matching)");
    println!("4. Formats the task as: [[mcp-tester]] Fix workflow execution bug");
    println!("5. Calls add_journal_task with the formatted task\n");

    println!("Expected client response:");
    println!("┌────────────────────────────────────────────────┐");
    println!("│ I can see \"mcp-tester\" in the page list,      │");
    println!("│ which matches \"MCP Tester\" (case-insensitive, │");
    println!("│ with hyphen instead of space).                 │");
    println!("│                                                 │");
    println!("│ I'll format the task with the correct link:    │");
    println!("│                                                 │");
    println!("│ <function_calls>                                │");
    println!("│   <invoke name=\"add_journal_task\">              │");
    println!("│     <parameter name=\"formatted_task\">           │");
    println!("│       [[mcp-tester]] Fix workflow execution bug│");
    println!("│     </parameter>                                │");
    println!("│   </invoke>                                     │");
    println!("│ </function_calls>                               │");
    println!("└────────────────────────────────────────────────┘\n");

    // =================================================================
    // Key Takeaways
    // =================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🎯 Key Takeaways: Hybrid Execution Model");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("✓ Server handles deterministic operations:");
    println!("  - Listing pages (API call)");
    println!("  - Fetching and embedding resource content");
    println!("  - Data fetching, validation");
    println!("  - Structured data operations\n");

    println!("✓ Client LLM handles reasoning:");
    println!("  - Fuzzy matching (\"MCP Tester\" → \"mcp-tester\")");
    println!("  - Context-aware decisions");
    println!("  - Natural language understanding\n");

    println!("✓ Guidance enables seamless handoff:");
    println!("  - with_guidance() provides instructions to LLM");
    println!("  - {{arg_name}} syntax substitutes actual values");
    println!("  - Clear separation of concerns\n");

    println!("✓ Benefits:");
    println!("  - Efficient: Server does what it can");
    println!("  - Intelligent: LLM handles complex reasoning");
    println!("  - Maintainable: Declarative workflow definition");
    println!("  - Flexible: Works for simple and complex cases\n");

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📚 When to Use Hybrid Execution");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("Use hybrid execution when:");
    println!("  ❌ Server can't match user input to structured data");
    println!("  ❌ Context-aware decisions are needed");
    println!("  ❌ Multiple valid options require LLM judgment");
    println!("  ❌ User might need to clarify intent\n");

    println!("Use full server-side execution when:");
    println!("  ✅ All parameters are deterministic");
    println!("  ✅ No fuzzy matching or reasoning needed");
    println!("  ✅ Single-shot execution is sufficient\n");

    Ok(())
}

fn create_logseq_task_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "add_project_task",
        "add a task to a Logseq project with intelligent project name matching",
    )
    .argument("project", "The project name (can be fuzzy, e.g., 'MCP Tester')", true)
    .argument("task", "The task description", true)
    // Step 1: Server executes this (deterministic)
    .step(
        WorkflowStep::new("list_pages", ToolHandle::new("list_pages"))
            .with_guidance("I'll first get all available page names from Logseq")
            .bind("pages"),
    )
    // Step 2: Client continues (needs LLM reasoning for fuzzy matching)
    // This step has guidance but no .arg() mappings
    // The tool schema requires 'formatted_task' parameter (see tool definition)
    // Server can't provide it -> automatic handoff to client
    // Guidance tells the client LLM what to do
    // Resource is fetched and embedded by server before handoff
    .step(
        WorkflowStep::new("add_task", ToolHandle::new("add_journal_task"))
            .with_guidance(
                "I'll now:\n\
                 1. Find the page name from the list above that best matches '{project}'\n\
                 2. Format the task according to the guide below\n\
                 3. Call add_journal_task with the formatted_task parameter",
            )
            .with_resource("docs://logseq/task-format")
            .expect("Valid resource URI")
            // No .arg() mappings - server will detect this doesn't satisfy schema
            // and gracefully hand off to client LLM
            .bind("result"),
    )
}
