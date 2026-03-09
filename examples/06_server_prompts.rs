//! Example: Server with prompt support
//!
//! This example demonstrates:
//! - Creating a server that provides prompts
//! - Implementing prompt handlers
//! - Dynamic prompt generation with arguments
//! - Prompt templates and formatting

use pmcp::{
    types::{
        capabilities::ServerCapabilities, GetPromptResult, MessageContent, PromptMessage, Role,
    },
    Server, SimplePrompt, SyncPrompt,
};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

// Type alias for async prompt handler
type AsyncPromptHandler = Box<
    dyn Fn(
            HashMap<String, String>,
            pmcp::RequestHandlerExtra,
        ) -> Pin<Box<dyn Future<Output = pmcp::Result<GetPromptResult>> + Send>>
        + Send
        + Sync,
>;

// Create code review prompt using SimplePrompt
fn create_code_review_prompt() -> SimplePrompt<AsyncPromptHandler> {
    SimplePrompt::new(
        "code_review",
        Box::new(|args: HashMap<String, String>, _extra: pmcp::RequestHandlerExtra| {
            Box::pin(async move {
                let language = args
                    .get("language")
                    .map(|s| s.as_str())
                    .unwrap_or("unknown");
                let code = args
                    .get("code")
                    .ok_or_else(|| pmcp::Error::validation("code argument is required"))?;
                let focus = args.get("focus").map(|s| s.as_str()).unwrap_or("general");

                let mut messages = vec![];

                // System message
                messages.push(PromptMessage {
                    role: Role::System,
                    content: MessageContent::Text {
                        text: format!(
                            "You are an expert {} code reviewer. Focus on {} aspects of the code. \
                             Provide constructive feedback with specific suggestions for improvement.",
                            language, focus
                        ),
                    },
                });

                // User message with the code
                messages.push(PromptMessage {
                    role: Role::User,
                    content: MessageContent::Text {
                        text: format!(
                            "Please review this {} code:\n\n```{}\n{}\n```",
                            language, language, code
                        ),
                    },
                });

                Ok(GetPromptResult::new(messages, Some(format!(
                        "Code review for {} code focusing on {}",
                        language, focus
                    ))))
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = pmcp::Result<GetPromptResult>> + Send>>
        }) as AsyncPromptHandler,
    )
    .with_description("Generate a code review prompt for the provided code")
    .with_argument("language", "Programming language of the code", false)
    .with_argument("code", "The code to review", true)
    .with_argument("focus", "Specific aspect to focus on (e.g., performance, security, style)", false)
}

// Create data analysis prompt using SyncPrompt
fn create_data_analysis_prompt(
) -> SyncPrompt<impl Fn(HashMap<String, String>) -> pmcp::Result<GetPromptResult> + Send + Sync> {
    SyncPrompt::new("data_analysis", |args| {
        let data_type = args.get("data_type").map(|s| s.as_str()).unwrap_or("CSV");
        let data = args
            .get("data")
            .ok_or_else(|| pmcp::Error::validation("data argument is required"))?;
        let question = args.get("question").map(|s| s.as_str());
        let output_format = args
            .get("output_format")
            .map(|s| s.as_str())
            .unwrap_or("summary");

        let mut messages = vec![];

        // System message
        messages.push(PromptMessage {
            role: Role::System,
            content: MessageContent::Text {
                text: format!(
                    "You are a data analyst expert. Analyze the provided {} data and \
                         provide insights in {} format. Be thorough and precise.",
                    data_type, output_format
                ),
            },
        });

        // User message with data
        let mut user_text = format!("Here is the {} data:\n\n{}\n\n", data_type, data);

        if let Some(q) = question {
            user_text.push_str(&format!("Please answer: {}", q));
        } else {
            user_text.push_str("Please provide a comprehensive analysis of this data.");
        }

        messages.push(PromptMessage {
            role: Role::User,
            content: MessageContent::Text { text: user_text },
        });

        Ok(GetPromptResult::new(
            messages,
            Some(format!("Data analysis for {} data", data_type)),
        ))
    })
    .with_description("Generate a data analysis prompt for the provided data")
    .with_argument("data_type", "Type of data (CSV, JSON, etc.)", false)
    .with_argument("data", "The data to analyze", true)
    .with_argument("question", "Specific question about the data", false)
    .with_argument(
        "output_format",
        "Desired output format (summary, detailed, visualization)",
        false,
    )
}

// Create writing assistant prompt using SyncPrompt
fn create_writing_assistant_prompt(
) -> SyncPrompt<impl Fn(HashMap<String, String>) -> pmcp::Result<GetPromptResult> + Send + Sync> {
    SyncPrompt::new(
        "writing_assistant",
        |args| {
            let style = args
                .get("style")
                .map(|s| s.as_str())
                .unwrap_or("professional");
            let topic = args
                .get("topic")
                .ok_or_else(|| pmcp::Error::validation("topic argument is required"))?;
            let length = args.get("length").map(|s| s.as_str()).unwrap_or("medium");
            let audience = args
                .get("audience")
                .map(|s| s.as_str())
                .unwrap_or("general");

            let mut messages = vec![];

            // System message
            messages.push(PromptMessage {
                role: Role::System,
                content: MessageContent::Text {
                    text: format!(
                        "You are a skilled writing assistant. Write in a {} style for a {} audience. \
                         The content should be {} in length. Ensure clarity, engagement, and appropriate tone.",
                        style, audience, length
                    ),
                },
            });

            // User message
            messages.push(PromptMessage {
                role: Role::User,
                content: MessageContent::Text {
                    text: format!("Write about: {}", topic),
                },
            });

            Ok(GetPromptResult::new(messages, Some(format!(
                    "Writing assistance for '{}' in {} style",
                    topic, style
                ))))
        },
    )
    .with_description("Generate written content on any topic")
    .with_argument("topic", "The topic to write about", true)
    .with_argument("style", "Writing style: professional, casual, academic, creative", false)
    .with_argument("length", "Content length: short, medium, long", false)
    .with_argument("audience", "Target audience: general, technical, children, experts", false)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    println!("=== MCP Server Prompts Example ===");
    println!("Starting server with prompt templates...\n");

    // Create prompts
    let code_review = create_code_review_prompt();
    let data_analysis = create_data_analysis_prompt();
    let writing_assistant = create_writing_assistant_prompt();

    // Build server with prompt support
    let server = Server::builder()
        .name("prompt-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::prompts_only())
        .prompt("code-review", code_review)
        .prompt("data-analysis", data_analysis)
        .prompt("writing-assistant", writing_assistant)
        .build()?;

    println!("Server ready! Prompts will be listed via the list_prompts protocol method.");
    println!("\nListening on stdio...");

    // Run server
    server.run_stdio().await?;

    Ok(())
}
