//! Example: Client resource access
//!
//! This example demonstrates:
//! - Listing available resources
//! - Reading resource contents
//! - Handling different content types
//! - Resource pagination

use pmcp::{Client, ClientCapabilities, StdioTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    println!("=== MCP Client Resources Example ===\n");

    // Create and initialize client
    let transport = StdioTransport::new();
    let mut client = Client::new(transport);

    // Use minimal capabilities - client doesn't need to advertise any special features
    // to access resources from a server
    let capabilities = ClientCapabilities::minimal();

    println!("Connecting to server...");
    let _server_info = client.initialize(capabilities).await?;
    println!("✅ Connected!\n");

    // List all resources with pagination
    println!("📋 Listing all available resources:\n");

    let mut cursor: Option<String> = None;
    let mut page = 1;

    loop {
        let result = client.list_resources(cursor).await?;

        if !result.resources.is_empty() {
            println!("📄 Page {}:", page);
            for resource in &result.resources {
                println!("\n  🔗 URI: {}", resource.uri);
                println!("     Name: {}", resource.name);
                if let Some(desc) = &resource.description {
                    println!("     Description: {}", desc);
                }
                if let Some(mime) = &resource.mime_type {
                    println!("     MIME type: {}", mime);
                }
            }
        }

        cursor = result.next_cursor;
        if cursor.is_none() {
            break;
        }

        page += 1;
        println!("\n--- More resources available ---");
    }

    // Read specific resources
    println!("\n\n📖 Reading specific resources:");

    // Example 1: Read a JSON configuration file
    println!("\n1️⃣ Reading JSON config:");
    match client
        .read_resource("file://config/app.json".to_string())
        .await
    {
        Ok(result) => {
            for content in result.contents {
                match content {
                    pmcp::types::Content::Resource {
                        uri,
                        text: Some(text),
                        ref mime_type,
                        ..
                    } => {
                        println!("   URI: {}", uri);
                        if let Some(mime) = mime_type {
                            println!("   Type: {}", mime);
                        }
                        println!("   Content:\n{}", text);

                        // Parse JSON if it's JSON
                        if mime_type.as_ref().map(|s| s.as_str()) == Some("application/json") {
                            match serde_json::from_str::<serde_json::Value>(&text) {
                                Ok(json) => {
                                    println!("   Parsed JSON: {:#?}", json);
                                },
                                Err(e) => {
                                    println!("   Failed to parse JSON: {}", e);
                                },
                            }
                        }
                    },
                    pmcp::types::Content::Resource {
                        uri,
                        text: None,
                        mime_type,
                        ..
                    } => {
                        println!("   URI: {} (no text content)", uri);
                        if let Some(mime) = mime_type {
                            println!("   Type: {}", mime);
                        }
                    },
                    pmcp::types::Content::Text { text } => {
                        println!("   Text content:\n{}", text);
                    },
                    pmcp::types::Content::Image { data, mime_type } => {
                        println!(
                            "   Image content: {} (data length: {})",
                            mime_type,
                            data.len()
                        );
                    },
                }
            }
        },
        Err(e) => {
            println!("   ❌ Error: {}", e);
        },
    }

    // Example 2: Read a CSV file
    println!("\n2️⃣ Reading CSV data:");
    match client
        .read_resource("file://data/users.csv".to_string())
        .await
    {
        Ok(result) => {
            for content in result.contents {
                if let pmcp::types::Content::Resource {
                    text: Some(text), ..
                } = content
                {
                    println!("   CSV Content:");
                    for line in text.lines() {
                        println!("   {}", line);
                    }
                }
            }
        },
        Err(e) => {
            println!("   ❌ Error: {}", e);
        },
    }

    // Example 3: Read a template resource with parameters
    println!("\n3️⃣ Reading template resource:");
    match client
        .read_resource("template://greeting/Alice".to_string())
        .await
    {
        Ok(result) => {
            for content in result.contents {
                match content {
                    pmcp::types::Content::Resource {
                        text: Some(text), ..
                    } => {
                        println!("   Message: {}", text);
                    },
                    pmcp::types::Content::Text { text } => {
                        println!("   Message: {}", text);
                    },
                    _ => {},
                }
            }
        },
        Err(e) => {
            println!("   ❌ Error: {}", e);
        },
    }

    // Example 4: Handle non-existent resource
    println!("\n4️⃣ Testing error handling:");
    match client
        .read_resource("file://nonexistent.txt".to_string())
        .await
    {
        Ok(_) => {
            println!("   Unexpected success!");
        },
        Err(e) => {
            println!("   ✅ Error caught: {}", e);
            match &e {
                pmcp::Error::Protocol { code, message, .. }
                    if code.as_i32() == pmcp::ErrorCode::METHOD_NOT_FOUND.as_i32() =>
                {
                    println!("   Resource not found: {}", message);
                },
                _ => {
                    println!("   Other error type: {:?}", e);
                },
            }
        },
    }

    Ok(())
}
