//! Example: Resource-Only Workflow Steps
//!
//! Demonstrates how to use resource-only steps in workflows to fetch context
//! without executing tools. This is useful when you need to gather information
//! from resources based on previous step results.
//!
//! # Use Case
//!
//! This example shows an Interactive Fiction game hint system that:
//! 1. Gets the user's current game progress (tool execution)
//! 2. Fetches the walkthrough for their specific game (resource-only step)
//! 3. Fetches general help resources (resource-only step)
//! 4. Returns all context to the LLM for generating hints
//!
//! # Key Feature
//!
//! The resource-only steps eliminate the need for dummy/redundant tool calls,
//! making workflows cleaner and more semantic.
//!
//! # Running the Example
//!
//! ```bash
//! cargo run --example 60_resource_only_steps
//! ```

use async_trait::async_trait;
use pmcp::server::workflow::dsl::field;
use pmcp::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};
use pmcp::{
    Content, ListResourcesResult, MessageContent, ReadResourceResult, RequestHandlerExtra,
    ResourceHandler, ResourceInfo, Result, Server, SimpleTool,
};
use serde_json::json;

// Resource handler for game walkthroughs and help
struct GameResources;

#[async_trait]
impl ResourceHandler for GameResources {
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        match uri {
            "if://walkthrough/zork1" => Ok(ReadResourceResult::new(vec![Content::Text {
                text: r#"
# Zork I Walkthrough

## West of House
You are standing in an open field west of a white house, with a boarded front door.

### Hints for Your Current Location:
1. **Easy hint**: Look around carefully
2. **Medium hint**: Try going around the house
3. **Hard hint**: Go east, then open the window and enter

## Key Items
- **Mailbox**: Contains a leaflet (5 points)
- **Window**: Can be opened from behind the house (10 points)
- **Lamp**: Essential for exploring dark areas (10 points)

## Current Difficulty
Based on your progress, you're still in the early game. Focus on:
- Exploring the house thoroughly
- Collecting the brass lantern
- Reading all available text carefully
"#
                .to_string(),
            }])),
            "if://walkthrough/planetfall" => Ok(ReadResourceResult::new(vec![Content::Text {
                text: r#"
# Planetfall Walkthrough

## Starting Area
You wake up in your bunk on the spaceship.

### Initial Steps:
1. Take inventory
2. Examine your surroundings
3. Head to the mess hall
4. Complete your assigned tasks

Remember: Floyd is your friend!
"#
                .to_string(),
            }])),
            "if://help/general" => Ok(ReadResourceResult::new(vec![Content::Text {
                text: r#"
# Interactive Fiction General Help

## Basic Commands
- **LOOK**: Examine your surroundings
- **INVENTORY** (or I): Check what you're carrying
- **TAKE [object]**: Pick up an item
- **DROP [object]**: Put down an item
- **EXAMINE [object]**: Look at something closely

## Movement
- **NORTH, SOUTH, EAST, WEST** (or N, S, E, W)
- **UP, DOWN** (or U, D)
- **IN, OUT**

## Interaction
- **OPEN [object]**: Open containers or doors
- **CLOSE [object]**: Close containers or doors
- **READ [object]**: Read text
- **PUSH/PULL/TURN [object]**: Manipulate objects

## Game Tips
1. Save your game frequently
2. Read everything carefully
3. Try to map out locations
4. Experiment with different commands
5. If stuck, try examining everything in detail
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
            ResourceInfo {
                uri: "if://help/general".to_string(),
                name: "IF General Help".to_string(),
                description: Some("General interactive fiction commands and tips".to_string()),
                mime_type: Some("text/markdown".to_string()),
                meta: None,
            },
        ]))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🎮 Interactive Fiction Server - Resource-Only Steps Example\n");

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
                        "score": 15,
                        "inventory": ["brass lantern", "leaflet"]
                    }))
                })
            })
            .with_description("Get your current game progress"),
        )
        // Resources: Game walkthroughs and help
        .resources(GameResources)
        // Workflow Prompt: Get contextual hint for current game
        .prompt_workflow(
            SequentialWorkflow::new("get_hint", "Get a helpful hint for your current game")
                .argument(
                    "hint_level",
                    "How detailed should the hint be? (subtle, clear, explicit)",
                    false,
                )
                // Step 1: Tool execution - Get user's current game progress
                .step(
                    WorkflowStep::new("get_progress", ToolHandle::new("get_my_progress"))
                        .with_guidance(
                            "I'll first check which game you're currently playing and where you are..."
                        )
                        .bind("user_progress"),
                )
                // Step 2: Resource-only - Fetch walkthrough based on game_id
                //
                // KEY FEATURE: This is a resource-only step!
                // - No tool execution
                // - No redundant/dummy tool calls
                // - Clean, semantic API
                // - Dynamic resource URI using template bindings
                .step(
                    WorkflowStep::fetch_resources("fetch_walkthrough")
                        .with_resource("if://walkthrough/{game_id}")
                        .expect("Valid resource URI")
                        .with_template_binding("game_id", field("user_progress", "game_id"))
                        .with_guidance(
                            "Now I'll fetch the walkthrough guide specifically for your current game..."
                        ),
                )
                // Step 3: Resource-only - Fetch general help (static resource)
                .step(
                    WorkflowStep::fetch_resources("fetch_general_help")
                        .with_resource("if://help/general")
                        .expect("Valid resource URI")
                        .with_guidance("I'll also include general interactive fiction commands..."),
                ),
        )?
        .build()?;

    println!("Server capabilities:");
    println!("  ✓ Tool: get_my_progress");
    println!("  ✓ Resources: if://walkthrough/*, if://help/general");
    println!("  ✓ Workflow Prompt: get_hint (with resource-only steps)\n");

    println!("📋 Testing the Resource-Only Steps Workflow:\n");

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
                        let display_text = if text.len() > 400 {
                            format!(
                                "{}...\n[truncated {} chars]",
                                &text[..400],
                                text.len() - 400
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
            println!("\n✨ Key Observations:");
            println!("   1. Step 1 executed get_my_progress tool");
            println!("   2. Step 2 fetched if://walkthrough/zork1 (no tool call!)");
            println!("   3. Step 3 fetched if://help/general (no tool call!)");
            println!("   4. All resources embedded as user messages");
            println!("   5. Clean conversation flow without dummy tools\n");
        },
        Err(e) => {
            eprintln!("❌ Error executing workflow: {}", e);
            return Err(e);
        },
    }

    println!("🎓 Pattern Comparison:\n");
    println!("   BEFORE (redundant tool pattern):");
    println!("     WorkflowStep::new(\"fetch\", ToolHandle::new(\"dummy_tool\"))");
    println!("         .with_resource(\"if://walkthrough/{{game_id}}\")");
    println!("         .with_template_binding(\"game_id\", field(\"progress\", \"game_id\"))\n");
    println!("   AFTER (resource-only step):");
    println!("     WorkflowStep::fetch_resources(\"fetch_walkthrough\")");
    println!("         .with_resource(\"if://walkthrough/{{game_id}}\")");
    println!("         .with_template_binding(\"game_id\", field(\"progress\", \"game_id\"))\n");

    println!("📚 Benefits:");
    println!("   ✓ No redundant tool calls");
    println!("   ✓ Clean, semantic API");
    println!("   ✓ Clear intent in code");
    println!("   ✓ Consistent with MCP: tools and resources are equal primitives");
    println!("   ✓ Better developer experience\n");

    println!("🔬 Use Cases:");
    println!("   • Fetching documentation based on context");
    println!("   • Loading configuration files dynamically");
    println!("   • Retrieving dataset samples using IDs from previous steps");
    println!("   • Including reference materials for LLM reasoning");
    println!("   • Building multi-source context for complex prompts\n");

    Ok(())
}
