//! End-to-end test for workflow prompt metadata
//!
//! This test mimics exactly what mcp-tester does: start a server, connect a client,
//! and verify the prompts/list response includes complete metadata.

#![cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]

use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::server::workflow::{InternalPromptMessage, SequentialWorkflow};
use pmcp::shared::streamable_http::{
    StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
};
use pmcp::types::Role;
use pmcp::{Client, ClientCapabilities, Result, Server};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use url::Url;

#[tokio::test]
async fn test_workflow_prompt_metadata_over_http() -> Result<()> {
    // Create the workflow exactly as in the bug report
    let workflow = SequentialWorkflow::new(
        "add_project_task",
        "Add a task to a Logseq project with proper task formatting and scheduling",
    )
    .argument(
        "project",
        "The project name (Logseq page) to add the task to",
        true,
    )
    .argument("task", "The task description", true)
    .argument("priority", "Task priority: A, B, or C (optional)", false)
    .argument(
        "state",
        "Task state: TODO, DOING, DONE, LATER, or NOW (default: TODO)",
        false,
    )
    .argument(
        "scheduled",
        "Scheduled date in ISO format (YYYY-MM-DD) or today/yesterday (optional)",
        false,
    )
    .argument(
        "deadline",
        "Deadline date in ISO format (YYYY-MM-DD) or today/yesterday (optional)",
        false,
    )
    .instruction(InternalPromptMessage::new(
        Role::System,
        "Add the task to the specified Logseq project with proper formatting",
    ));

    // Build server with workflow prompt
    let server = Server::builder()
        .name("test-logseq-server")
        .version("1.0.0")
        .capabilities(pmcp::types::ServerCapabilities::prompts_only())
        .prompt_workflow(workflow)?
        .build()?;

    // Wrap server in Arc<Mutex<>> for HTTP transport
    let server = Arc::new(Mutex::new(server));

    // Start server on a random port
    let addr: SocketAddr = "127.0.0.1:18765".parse().unwrap();

    // Create HTTP server wrapper
    let http_server = StreamableHttpServer::new(addr, server);

    // Spawn server in background
    tokio::spawn(async move {
        let _ = http_server.start().await;
    });

    println!("Server starting on {}", addr);

    // Give server time to start
    sleep(Duration::from_millis(500)).await;

    // Create client
    // Builder, not a struct literal: `session_id` / `on_resumption_token` are
    // `v1-compat`-only, so a literal naming them breaks the `full-v2` build.
    let config = StreamableHttpTransportConfigBuilder::new(
        Url::parse(&format!("http://{}", addr))
            .map_err(|e| pmcp::Error::Internal(e.to_string()))?,
    )
    .enable_json_response()
    .build();
    let transport = StreamableHttpTransport::new(config);
    let mut client = Client::new(transport);

    // Initialize client
    let init_result = client.initialize(ClientCapabilities::default()).await?;

    println!("Client initialized: {:?}", init_result.server_info);

    // List prompts - this is what mcp-tester does
    let prompts_result = client.list_prompts(None).await?;

    println!("\n=== Prompts/List Response ===");
    println!("Prompts count: {}", prompts_result.prompts.len());

    // Verify we have the prompt
    assert_eq!(prompts_result.prompts.len(), 1, "Should have 1 prompt");

    let prompt = &prompts_result.prompts[0];
    println!("\nPrompt:");
    println!("  Name: {}", prompt.name);
    println!("  Description present: {}", prompt.description.is_some());
    println!("  Description: {:?}", prompt.description);
    println!("  Arguments present: {}", prompt.arguments.is_some());
    if let Some(args) = &prompt.arguments {
        println!("  Arguments count: {}", args.len());
        for arg in args {
            println!(
                "    - {}: {} (required: {})",
                arg.name,
                arg.description.as_deref().unwrap_or("<no desc>"),
                arg.required
            );
        }
    }

    // These are the exact checks mcp-tester performs
    assert!(
        prompt.description.is_some(),
        "❌ FAIL: Description should be present (mcp-tester check)"
    );

    assert_eq!(
        prompt.description.as_ref().unwrap(),
        "Add a task to a Logseq project with proper task formatting and scheduling",
        "Description should match"
    );

    assert!(
        prompt.arguments.is_some(),
        "❌ FAIL: Arguments should be present (mcp-tester check)"
    );

    let args = prompt.arguments.as_ref().unwrap();
    assert!(
        !args.is_empty(),
        "❌ FAIL: Arguments should not be empty (mcp-tester check)"
    );

    assert_eq!(args.len(), 6, "Should have 6 arguments");

    // Verify first argument
    assert_eq!(args[0].name, "project");
    assert!(args[0].required);
    assert!(
        args[0].description.is_some(),
        "Argument should have description"
    );

    println!("\n✅ All mcp-tester checks passed!");
    println!("The workflow prompt metadata is properly exposed over HTTP.");

    Ok(())
}
