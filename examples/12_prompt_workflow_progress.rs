//! Prompt Workflow with Progress Reporting
//!
//! This example demonstrates progress reporting in long-running prompt workflows.
//! A multi-step analysis prompt reports progress at each workflow stage.
//!
//! Features demonstrated:
//! - Progress reporting in prompts (not just tools)
//! - Multi-step workflow with clear progress tracking
//! - Cancellation support during workflow execution
//! - Best practice pattern for workflow prompts
//!
//! Run with:
//! ```bash
//! cargo run --example 12_prompt_workflow_progress
//! ```

use async_trait::async_trait;
use pmcp::error::Result;
use pmcp::server::cancellation::RequestHandlerExtra;
use pmcp::server::{PromptHandler, Server};
use pmcp::types::{
    Content, GetPromptRequest, GetPromptResult, ProgressToken, PromptMessage, RequestMeta, Role,
};
use std::collections::HashMap;
use std::time::Duration;

/// A workflow prompt that performs multi-step analysis with progress reporting.
///
/// This demonstrates the recommended pattern for long-running prompts:
/// - Break workflow into clear steps
/// - Report progress at each step
/// - Support cancellation between steps
/// - Provide meaningful status messages
struct AnalysisWorkflowPrompt;

#[async_trait]
impl PromptHandler for AnalysisWorkflowPrompt {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        extra: RequestHandlerExtra,
    ) -> Result<GetPromptResult> {
        let topic = args
            .get("topic")
            .cloned()
            .unwrap_or_else(|| "general analysis".to_string());

        tracing::info!("Starting analysis workflow for topic: {}", topic);

        // Define workflow steps
        let steps = [
            ("gather", "Gathering information and context"),
            ("analyze", "Analyzing data and patterns"),
            ("synthesize", "Synthesizing insights"),
            ("validate", "Validating conclusions"),
            ("format", "Formatting final report"),
        ];

        let mut results = Vec::new();

        // Execute each step with progress reporting
        for (i, (step_name, step_description)) in steps.iter().enumerate() {
            // Check for cancellation before each step
            if extra.is_cancelled() {
                tracing::warn!("Workflow cancelled at step: {}", step_name);
                return Err(pmcp::error::Error::internal(format!(
                    "Analysis workflow cancelled during {} step",
                    step_name
                )));
            }

            // Report progress for this step
            extra
                .report_count(
                    i + 1,
                    steps.len(),
                    Some(format!(
                        "Step {}/{}: {}",
                        i + 1,
                        steps.len(),
                        step_description
                    )),
                )
                .await?;

            tracing::info!(
                "Executing step {}/{}: {} - {}",
                i + 1,
                steps.len(),
                step_name,
                step_description
            );

            // Simulate work for this step
            tokio::time::sleep(Duration::from_secs(1)).await;

            // Record step completion
            results.push(format!(
                "✓ {} - {} (completed)",
                step_name, step_description
            ));
        }

        // Build the final prompt result
        let workflow_summary = results.join("\n");

        Ok(GetPromptResult::new(
            vec![PromptMessage {
                role: Role::User,
                content: Content::Text {
                    text: format!(
                        "Analysis Workflow Complete\n\nTopic: {}\n\nWorkflow Steps:\n{}\n\nAll {} steps completed successfully. Ready for review.",
                        topic, workflow_summary, steps.len()
                    ),
                },
            }],
            Some(format!(
                "Multi-step analysis workflow for: {}",
                topic
            )),
        ))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    println!("=== Prompt Workflow Progress Example ===\n");

    // Create server with workflow prompt
    let _server = Server::builder()
        .name("workflow-server")
        .version("1.0.0")
        .prompt("analysis_workflow", AnalysisWorkflowPrompt)
        .build()?;

    println!("Server created with 'analysis_workflow' prompt");
    println!("Prompt arguments:");
    println!("  topic (optional) - Topic to analyze (default: 'general analysis')\n");

    // Example 1: Normal workflow execution
    println!("--- Example 1: Complete workflow with progress tracking ---\n");

    let request = GetPromptRequest {
        name: "analysis_workflow".to_string(),
        arguments: HashMap::from([("topic".to_string(), "Machine Learning".to_string())]),
        _meta: Some(RequestMeta {
            progress_token: Some(ProgressToken::String("workflow-1".to_string())),
            _task_id: None,
        }),
    };

    println!("Executing workflow with progress token 'workflow-1'...\n");

    let prompt = AnalysisWorkflowPrompt;
    let extra = RequestHandlerExtra::new(
        "test-request-1".to_string(),
        tokio_util::sync::CancellationToken::new(),
    );

    let result = prompt.handle(request.arguments, extra).await?;

    println!("\n✅ Workflow completed!");
    println!("Description: {}", result.description.unwrap_or_default());
    println!("\nMessages generated: {}", result.messages.len());

    // Example 2: Workflow with cancellation
    println!("\n--- Example 2: Workflow with mid-execution cancellation ---\n");

    let request = GetPromptRequest {
        name: "analysis_workflow".to_string(),
        arguments: HashMap::from([("topic".to_string(), "Data Science".to_string())]),
        _meta: Some(RequestMeta {
            progress_token: Some(ProgressToken::String("workflow-2".to_string())),
            _task_id: None,
        }),
    };

    println!("Executing workflow with cancellation after 2.5 seconds...\n");

    let cancellation_token = tokio_util::sync::CancellationToken::new();
    let extra = RequestHandlerExtra::new("test-request-2".to_string(), cancellation_token.clone());

    // Cancel after 2.5 seconds (should interrupt at step 3)
    let cancel_handle = tokio::spawn({
        let token = cancellation_token.clone();
        async move {
            tokio::time::sleep(Duration::from_millis(2500)).await;
            println!("\n🛑 Cancelling workflow...\n");
            token.cancel();
        }
    });

    let result = prompt.handle(request.arguments, extra).await;

    match result {
        Ok(v) => println!("Unexpected success: {}", v.description.unwrap_or_default()),
        Err(e) => println!("❌ Workflow cancelled as expected: {}\n", e),
    }

    cancel_handle.await.unwrap();

    println!("--- Best Practices Demonstrated ---\n");
    println!("1. ✅ Multi-step workflow with clear progress tracking");
    println!("2. ✅ Progress reported at each workflow step");
    println!("3. ✅ Meaningful status messages for each step");
    println!("4. ✅ Cancellation checked between steps");
    println!("5. ✅ Same API as tools - extra.report_count()");
    println!("6. ✅ Automatic progress notification handling\n");

    println!("--- When to Use Progress in Prompts ---\n");
    println!("✓ Multi-step workflows (analysis → planning → execution)");
    println!("✓ Long-running data processing or generation");
    println!("✓ Prompts with multiple external API calls");
    println!("✓ Complex reasoning chains with distinct phases");
    println!("✗ Simple single-step prompts (no progress needed)\n");

    println!("=== Example Complete ===");

    Ok(())
}
