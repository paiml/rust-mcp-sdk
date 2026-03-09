//! Simple Calculator template - starting point for learning MCP
//!
//! Demonstrates the most basic MCP server:
//! - Single tool: add
//! - Simple input/output
//! - Foundation for understanding MCP concepts

pub const CALCULATOR_LIB: &str = r####"//! Simple Calculator MCP Server
//!
//! A minimal MCP server that demonstrates:
//! - Tool definition with TypedToolWithOutput (full input/output type safety)
//! - Automatic JSON schema generation for both input AND output
//! - Output schema annotations for type-safe server composition
//! - Basic arithmetic operations
//!
//! This is the starting point for learning MCP. From here, you can:
//! 1. Add more arithmetic operations (subtract, multiply, divide)
//! 2. Add resources (e.g., calculation history)
//! 3. Add workflow prompts for multi-step calculations
//!
//! ## Output Schema Feature
//!
//! This server uses `TypedToolWithOutput` which automatically generates
//! a top-level `outputSchema` on ToolInfo and `pmcp:outputTypeName` in annotations.
//! This enables code generators to create typed clients for server composition:
//!
//! ```rust,ignore
//! // Generated client code would look like:
//! let result: AddResult = calculator_client.add(AddArgs { a: 5.0, b: 3.0 }).await?;
//! println!("Sum: {}", result.result);  // Type-safe access!
//! ```

use pmcp::{Result, Server};
use pmcp::server::typed_tool::TypedToolWithOutput;
use pmcp::types::{ServerCapabilities, ToolCapabilities};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

/// Input arguments for the add operation
///
/// This demonstrates type safety with automatic schema generation and validation:
/// - `schemars::JsonSchema` automatically generates detailed JSON schema for MCP clients
/// - `validator::Validate` provides runtime validation with custom constraints
/// - `serde` handles JSON serialization/deserialization
/// - `deny_unknown_fields` rejects any extra fields for security
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AddInput {
    /// First number in the addition operation
    #[schemars(description = "First number in the addition operation", range(min = -1000000, max = 1000000))]
    pub a: f64,

    /// Second number in the addition operation
    #[schemars(description = "Second number in the addition operation", range(min = -1000000, max = 1000000))]
    pub b: f64,
}

/// Output result from the add operation
///
/// This type is used for:
/// 1. Type-safe return values from the tool handler
/// 2. Automatic output schema generation (top-level `outputSchema` on ToolInfo)
/// 3. Enabling typed client generation for server composition
///
/// When this server is composed by another MCP server, the code generator
/// can create typed Rust structs for the response, enabling compile-time
/// safety for inter-server communication.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddResult {
    /// The sum of the two input numbers
    #[schemars(description = "The sum of a + b")]
    pub result: f64,
}

// ============================================================================
// TOOL IMPLEMENTATIONS
// ============================================================================

async fn add_tool(input: AddInput, _extra: pmcp::RequestHandlerExtra) -> Result<AddResult> {
    Ok(AddResult {
        result: input.a + input.b,
    })
}

// ============================================================================
// SERVER BUILDER
// ============================================================================

/// Build the calculator server with output schema support
///
/// This server demonstrates the PMCP output schema feature:
/// - Tools are registered with `TypedToolWithOutput` instead of `TypedTool`
/// - Both input AND output types derive `JsonSchema`
/// - The tool's `ToolInfo` includes top-level `outputSchema` and `pmcp:outputTypeName` in annotations
///
/// When you run `cargo pmcp schema export`, the exported schema will include
/// the output type information, enabling `cargo pmcp generate` to create
/// fully typed client code.
pub fn build_calculator_server() -> Result<Server> {
    Server::builder()
        .name("calculator")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities {
                list_changed: Some(true)
            }),
            ..Default::default()
        })
        // Using TypedToolWithOutput for full input/output type safety
        // This automatically generates:
        // - Input schema from AddInput (as inputSchema)
        // - Output schema from AddResult (as top-level outputSchema on ToolInfo)
        .tool(
            "add",
            TypedToolWithOutput::new("add", |input: AddInput, extra| {
                Box::pin(add_tool(input, extra))
            })
            .with_description("Add two numbers together with range validation")
        )
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add() {
        let input = AddInput { a: 5.0, b: 3.0 };
        let result = add_tool(input, pmcp::RequestHandlerExtra::default())
            .await
            .unwrap();
        assert_eq!(result.result, 8.0);
    }

    #[tokio::test]
    async fn test_add_negative() {
        let input = AddInput { a: 10.0, b: -3.0 };
        let result = add_tool(input, pmcp::RequestHandlerExtra::default())
            .await
            .unwrap();
        assert_eq!(result.result, 7.0);
    }

    #[tokio::test]
    async fn test_server_builds() {
        let server = build_calculator_server();
        assert!(server.is_ok());
    }
}
"####;
