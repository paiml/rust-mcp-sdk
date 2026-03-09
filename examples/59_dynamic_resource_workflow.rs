//! Example: Dynamic Resource URI Interpolation in Workflows
//!
//! Demonstrates how to use template bindings to construct resource URIs dynamically
//! based on values from previous workflow steps or prompt arguments.
//!
//! # Use Case
//!
//! This example shows an Interactive Fiction game hint system that:
//! 1. Gets the user's current game progress
//! 2. Dynamically fetches the walkthrough for their specific game
//! 3. Returns both progress and walkthrough in a single prompt response
//!
//! # Running the Example
//!
//! ```bash
//! cargo run --example 59_dynamic_resource_workflow
//! ```

use async_trait::async_trait;
use pmcp::server::workflow::dsl::field;
use pmcp::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};
use pmcp::{
    Content, ListResourcesResult, MessageContent, ReadResourceResult, RequestHandlerExtra,
    ResourceHandler, ResourceInfo, Result, Server, SimpleTool,
};
use serde_json::json;

// Resource handler for game walkthroughs
struct WalkthroughResources;

#[async_trait]
impl ResourceHandler for WalkthroughResources {
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        match uri {
            "if://walkthrough/zork1" => Ok(ReadResourceResult::new(vec![Content::Text {
                text: r#"
# Zork I Walkthrough

## West of House
You are standing in an open field west of a white house, with a boarded front door.

### Hints:
1. Go around to the back of the house (type: "east")
2. There's a window you can open
3. Enter through the window to start your adventure
4. Don't forget to pick up useful items!

## Scoring:
- Opening the mailbox: +5 points
- Entering the house: +10 points
- Finding the lamp: +10 points
"#
                .to_string(),
            }])),
            "if://walkthrough/planetfall" => Ok(ReadResourceResult::new(vec![Content::Text {
                text: "# Planetfall Walkthrough\n\nYour adventure in space...".to_string(),
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
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        Ok(ListResourcesResult::new(vec![
            ResourceInfo {
                uri: "if://walkthrough/zork1".to_string(),
                name: "Zork I Walkthrough".to_string(),
                description: Some("Complete walkthrough for Zork I".to_string()),
                mime_type: Some("text/markdown".to_string()),
                meta: None,
            },
            ResourceInfo {
                uri: "if://walkthrough/planetfall".to_string(),
                name: "Planetfall Walkthrough".to_string(),
                description: Some("Complete walkthrough for Planetfall".to_string()),
                mime_type: Some("text/markdown".to_string()),
                meta: None,
            },
        ]))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🎮 Interactive Fiction Server - Dynamic Resource Example\n");

    // Build the server with tools, resources, and workflow prompts
    let server = Server::builder()
        .name("interactive-fiction")
        .version("1.0.0")
        // Tool: Get user's current game progress
        .tool(
            "get_my_progress",
            SimpleTool::new("get_my_progress", |_args, _extra| {
                Box::pin(async move {
                    // Simulate returning user's current game info
                    Ok(json!({
                        "game_id": "zork1",
                        "location": "West of House",
                        "moves": 42,
                        "score": 15
                    }))
                })
            })
            .with_description("Get your current game progress"),
        )
        // Resources: Game walkthroughs
        .resources(WalkthroughResources)
        // Workflow Prompt: Get contextual hint for current game
        .prompt_workflow(
            SequentialWorkflow::new("get_hint", "Get a helpful hint for your current game")
                .argument("hint_level", "How detailed should the hint be? (subtle, clear, explicit)", false)
                // Step 1: Get user's current game progress
                .step(
                    WorkflowStep::new("get_progress", ToolHandle::new("get_my_progress"))
                        .with_guidance(
                            "I'll first check which game you're currently playing and where you are..."
                        )
                        .bind("user_progress"),
                )
                // Step 2: Dynamically fetch walkthrough based on game_id using resource-only step
                //
                // This is the KEY feature being demonstrated:
                // - Resource URI contains template variable: {game_id}
                // - Template binding maps {game_id} to field from previous step
                // - At execution time, the actual game_id is interpolated into the URI
                // - Using WorkflowStep::fetch_resources() - no redundant tool calls!
                .step(
                    WorkflowStep::fetch_resources("fetch_walkthrough")
                        .with_resource("if://walkthrough/{game_id}")
                        .expect("Valid resource URI")
                        .with_template_binding("game_id", field("user_progress", "game_id"))
                        .with_guidance(
                            "Now I'll fetch the walkthrough guide specifically for your current game..."
                        ),
                ),
        )?
        .build()?;

    println!("Server capabilities:");
    println!("  ✓ Tool: get_my_progress");
    println!("  ✓ Resources: if://walkthrough/zork1, if://walkthrough/planetfall");
    println!("  ✓ Workflow Prompt: get_hint (with dynamic resource fetching)\n");

    println!("📋 Testing the Dynamic Resource Workflow:\n");

    // Simulate calling the get_hint prompt
    println!("Calling prompt: get_hint");
    println!("Arguments: hint_level=clear\n");

    let mut args = std::collections::HashMap::new();
    args.insert("hint_level".to_string(), "clear".to_string());

    let extra = RequestHandlerExtra::new("demo-request".to_string(), Default::default());

    // Get the prompt handler and execute it
    let prompt_handler = server
        .get_prompt("get_hint")
        .expect("Workflow should be registered");

    match prompt_handler.handle(args, extra).await {
        Ok(result) => {
            println!("✅ Workflow executed successfully!\n");
            println!("Messages returned:");
            println!("{}", "─".repeat(80));

            for (i, message) in result.messages.iter().enumerate() {
                println!("\n[Message {}] Role: {:?}", i + 1, message.role);
                match &message.content {
                    MessageContent::Text { text } => {
                        // Truncate very long text for display
                        let display_text = if text.len() > 300 {
                            format!(
                                "{}...\n[truncated {} chars]",
                                &text[..300],
                                text.len() - 300
                            )
                        } else {
                            text.clone()
                        };
                        println!("{}", display_text);
                    },
                    MessageContent::Image { .. } => {
                        println!("[Image content]");
                    },
                    MessageContent::Resource { .. } => {
                        println!("[Resource content]");
                    },
                }
            }

            println!("{}", "─".repeat(80));
            println!("\n✨ Key Observation:");
            println!("   The workflow automatically:");
            println!("   1. Called get_my_progress tool");
            println!("   2. Extracted game_id='zork1' from the result");
            println!("   3. Fetched resource at if://walkthrough/zork1 (interpolated!)");
            println!("   4. Embedded the walkthrough content in the conversation");
        },
        Err(e) => {
            eprintln!("❌ Error executing workflow: {}", e);
            return Err(e);
        },
    }

    println!("\n🎓 Pattern Summary:");
    println!("   This demonstrates the BEFORE/AFTER difference:\n");
    println!("   BEFORE (manual implementation):");
    println!("     → 100+ lines of custom PromptHandler code");
    println!("     → Manual tool calling, resource fetching, error handling");
    println!("     → Code duplication across similar prompts\n");
    println!("   AFTER (workflow with dynamic resources):");
    println!("     → 10 lines of declarative workflow DSL");
    println!("     → Automatic resource URI interpolation");
    println!("     → Reusable pattern for similar use cases\n");

    println!("📚 Template Binding Patterns:\n");
    println!("   Pattern 1: Field from previous step");
    println!("     .with_template_binding(\"game_id\", field(\"user_progress\", \"game_id\"))\n");
    println!("   Pattern 2: Prompt argument");
    println!("     .with_template_binding(\"doc_id\", prompt_arg(\"document_id\"))\n");
    println!("   Pattern 3: Multiple variables");
    println!("     .with_template_binding(\"org\", field(\"project\", \"organization\"))");
    println!("     .with_template_binding(\"repo\", field(\"project\", \"repository\"))\n");
    println!("   Pattern 4: Nested field access (dot notation)");
    println!("     .with_template_binding(\"user_id\", field(\"context\", \"user.profile.id\"))\n");

    Ok(())
}
