# Output Schema for Type-Safe Composition

## Overview

PMCP supports MCP's top-level `outputSchema` field on tools (per MCP spec 2025-06-18) to enable **full type safety** in server composition workflows. While MCP provides type-safe input schemas, tool outputs are typically `serde_json::Value` - losing type safety at composition boundaries.

PMCP provides:
- `outputSchema` - Top-level JSON Schema on `ToolInfo` describing the tool's return type (MCP spec 2025-06-18)
- `pmcp:outputTypeName` - Annotation extension naming the code-generated output struct

## Why This Matters

### The Problem: Composition Type Blindness

When one MCP server calls another, you lose type information:

```rust
// Without output schemas - what shape does result have?
let result: Value = composition_client
    .call_tool("sqlite-explorer", "query", json!({"sql": "SELECT * FROM orders"}))
    .await?;

// Must guess or parse manually - error prone!
let rows = result["rows"].as_array().ok_or("expected rows")?;
let columns = result["columns"].as_array().ok_or("expected columns")?;
```

### The Solution: Top-Level Output Schema

With output schemas on `ToolInfo`, code generators produce typed clients:

```rust
// Generated from schema - full type safety!
#[derive(Debug, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub row_count: i64,
}

// Type-safe composition
let result: QueryResult = sqlite_client
    .query(QueryArgs { sql: "SELECT * FROM orders".into(), ..Default::default() })
    .await?;

// Compiler-checked field access
println!("Found {} rows", result.row_count);
for col in &result.columns {
    println!("Column: {}", col);
}
```

## Using Output Schemas

### In Tool Definitions

Set output schema as a top-level field on `ToolInfo`:

```rust
use pmcp::types::{ToolInfo, ToolAnnotations};
use serde_json::json;

let tool = ToolInfo::new(
    "query",
    Some("Execute SQL query and return results".into()),
    json!({
        "type": "object",
        "properties": {
            "sql": { "type": "string", "description": "SQL query to execute" },
            "params": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["sql"]
    }),
)
.with_output_schema(json!({
    "type": "object",
    "properties": {
        "columns": { "type": "array", "items": { "type": "string" } },
        "rows": { "type": "array" },
        "row_count": { "type": "integer" }
    },
    "required": ["columns", "rows", "row_count"]
}))
.with_annotations(
    ToolAnnotations::new()
        .with_read_only(true)
        .with_output_type_name("QueryResult")  // PMCP codegen extension
);
```

### With TypedTool and schemars (Automatic Generation)

For full automation, define both input and output types with `schemars`:

```rust
use pmcp::server::typed_tool::TypedToolWithOutput;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Input arguments for the query tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryArgs {
    /// SQL query to execute
    pub sql: String,
    /// Optional query parameters
    #[serde(default)]
    pub params: Vec<String>,
}

/// Query execution result
#[derive(Debug, Serialize, JsonSchema)]
pub struct QueryResult {
    /// Column names from the result set
    pub columns: Vec<String>,
    /// Row data as arrays of values
    pub rows: Vec<Vec<serde_json::Value>>,
    /// Total number of rows returned
    pub row_count: i64,
}

// Both input AND output schemas are generated automatically
let tool = TypedToolWithOutput::new("query", |args: QueryArgs, _extra| {
    Box::pin(async move {
        let result = execute_query(&args.sql, &args.params).await?;
        Ok(QueryResult {
            columns: result.columns,
            rows: result.rows,
            row_count: result.rows.len() as i64,
        })
    })
})
.with_description("Execute SQL query and return results");
```

## Schema Export and Code Generation

### 1. Export Schema from Running Server

```bash
# Export all tool schemas including output schemas
cargo pmcp schema export --endpoint https://my-server.pmcp.run/mcp \
    --output my-server-schema.json
```

The exported schema includes output schema at the top level per MCP spec 2025-06-18:

```json
{
  "version": "1.0",
  "servers": [{
    "id": "sqlite-explorer",
    "tools": [{
      "name": "query",
      "description": "Execute SQL query",
      "inputSchema": { ... },
      "outputSchema": {
        "type": "object",
        "properties": {
          "columns": { "type": "array", "items": { "type": "string" } },
          "rows": { "type": "array" },
          "row_count": { "type": "integer" }
        }
      },
      "annotations": {
        "readOnlyHint": true,
        "pmcp:outputTypeName": "QueryResult"
      }
    }]
  }]
}
```

### 2. Generate Typed Client

```bash
# Generate Rust client with full type safety
cargo pmcp generate --schema my-server-schema.json \
    --output src/clients/sqlite_explorer.rs
```

### Generated Code

```rust
//! Auto-generated typed client for sqlite-explorer
//! Generated from my-server-schema.json

use pmcp_composition::{CompositionClient, CompositionError};
use serde::{Deserialize, Serialize};

// ============================================================================
// Input Types (from inputSchema)
// ============================================================================

/// Arguments for query tool
#[derive(Debug, Serialize)]
pub struct QueryArgs {
    /// SQL query to execute
    pub sql: String,
    /// Optional query parameters
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<String>,
}

// ============================================================================
// Output Types (from top-level outputSchema)
// ============================================================================

/// Result from query tool
#[derive(Debug, Deserialize)]
pub struct QueryResult {
    /// Column names from the result set
    pub columns: Vec<String>,
    /// Row data as arrays of values
    pub rows: Vec<Vec<serde_json::Value>>,
    /// Total number of rows returned
    pub row_count: i64,
}

// ============================================================================
// Typed Client
// ============================================================================

/// Typed client for sqlite-explorer server
pub struct SqliteExplorerClient<'a> {
    inner: &'a CompositionClient,
}

impl<'a> SqliteExplorerClient<'a> {
    pub fn new(client: &'a CompositionClient) -> Self {
        Self { inner: client }
    }

    /// Execute SQL query and return results
    pub async fn query(&self, args: QueryArgs) -> Result<QueryResult, CompositionError> {
        let result = self.inner
            .call_tool("sqlite-explorer", "query", serde_json::to_value(args)?)
            .await?;
        Ok(serde_json::from_value(result)?)
    }
}
```

## MCP Protocol Compatibility

### outputSchema Is a Top-Level Tool Field

Per MCP spec 2025-06-18, `outputSchema` is a top-level field on the Tool object, as a sibling to `inputSchema`:

```json
{
  "name": "query",
  "inputSchema": { ... },
  "outputSchema": { ... },
  "annotations": {
    "readOnlyHint": true,
    "pmcp:outputTypeName": "QueryResult"
  }
}
```

### PMCP Extension: pmcp:outputTypeName

The `pmcp:outputTypeName` annotation remains in `annotations` as a PMCP codegen extension. It provides the struct name for code generation. Standard MCP clients ignore `pmcp:*` annotations per the spec:

> "Clients SHOULD ignore annotations they don't understand." - MCP Specification

This follows the established pattern of vendor-prefixed extensions (like `x-*` in OpenAPI).

## Standard MCP Annotations

In addition to the top-level outputSchema and PMCP extensions, tools can use standard MCP annotations:

| Annotation | Type | Description |
|------------|------|-------------|
| `title` | string | Human-readable title |
| `readOnlyHint` | boolean | Tool doesn't modify state |
| `destructiveHint` | boolean | Tool may perform destructive operations |
| `idempotentHint` | boolean | Multiple calls with same args have same effect |
| `openWorldHint` | boolean | Tool interacts with external systems |

Example with annotations and top-level output schema:

```rust
let tool = ToolInfo::new("query", Some("Execute SQL".into()), input_schema)
    .with_output_schema(output_schema)
    .with_annotations(
        ToolAnnotations::new()
            .with_read_only(true)
            .with_output_type_name("QueryResult")
    );
```

Or directly in JSON:

```json
{
  "name": "delete_record",
  "inputSchema": { ... },
  "outputSchema": {
    "type": "object",
    "properties": { "deleted": { "type": "boolean" } }
  },
  "annotations": {
    "readOnlyHint": false,
    "destructiveHint": true,
    "idempotentHint": true,
    "pmcp:outputTypeName": "DeleteResult"
  }
}
```

## Best Practices

### 1. Always Include Output Schemas for Tools Used in Composition

If your server will be called by other servers, add output schemas:

```rust
// Good: Output schema enables type-safe composition
let tool = ToolInfo::new("query", Some("Execute SQL".into()), input_schema)
    .with_output_schema(result_schema);
```

### 2. Use Descriptive Type Names

The `pmcp:outputTypeName` becomes the generated struct name:

```rust
// Good: Clear, descriptive name
ToolAnnotations::new().with_output_type_name("OrderQueryResult")

// Bad: Generic name
ToolAnnotations::new().with_output_type_name("Result")
```

### 3. Document Schema Fields

Include descriptions in your JSON Schema:

```json
{
  "type": "object",
  "properties": {
    "count": {
      "type": "integer",
      "description": "Number of records matched"
    }
  }
}
```

These become doc comments in generated code.

### 4. Match Output Schema to Actual Return Values

Ensure your tool's return value matches the declared schema:

```rust
// Schema declares: { "count": integer, "items": array }
// Tool must return matching structure:
Ok(json!({
    "count": items.len(),
    "items": items
}))
```

### 5. Use TypedToolWithOutput for Automatic Schema Sync

When using `TypedToolWithOutput`, the output schema is generated from your Rust type, guaranteeing they match:

```rust
TypedToolWithOutput::new("my_tool", |args: Input, _| {
    Box::pin(async move {
        Ok(Output { ... })  // Schema generated from Output type
    })
})
```

## Workflow Summary

```
+--------------------------------------------------------------------+
|                     Type-Safe Composition Flow                      |
+--------------------------------------------------------------------+
|                                                                    |
|  1. Define Tool with Output Schema                                 |
|     +-------------------------------------+                       |
|     | TypedToolWithOutput<Input, Output>  |                       |
|     | - Auto-generates inputSchema        |                       |
|     | - Auto-generates outputSchema       |                       |
|     | - outputSchema on ToolInfo          |                       |
|     | - outputTypeName in annotations     |                       |
|     +-------------------------------------+                       |
|                         |                                          |
|                         v                                          |
|  2. Export Schema                                                  |
|     +-------------------------------------+                       |
|     | cargo pmcp schema export            |                       |
|     | --endpoint https://server/mcp       |                       |
|     | --output schema.json                |                       |
|     +-------------------------------------+                       |
|                         |                                          |
|                         v                                          |
|  3. Generate Typed Client                                          |
|     +-------------------------------------+                       |
|     | cargo pmcp generate                 |                       |
|     | --schema schema.json                |                       |
|     | --output src/clients/server.rs      |                       |
|     |                                     |                       |
|     | Produces:                           |                       |
|     | - InputArgs structs (from input)    |                       |
|     | - OutputResult structs (from output)|                       |
|     | - Typed client methods              |                       |
|     +-------------------------------------+                       |
|                         |                                          |
|                         v                                          |
|  4. Use in Domain Server                                           |
|     +-------------------------------------+                       |
|     | let result: QueryResult = client    |                       |
|     |     .query(QueryArgs { ... })       |                       |
|     |     .await?;                        |                       |
|     |                                     |                       |
|     | // Full type safety!                |                       |
|     | result.columns.len()                |                       |
|     | result.rows.iter()                  |                       |
|     +-------------------------------------+                       |
|                                                                    |
+--------------------------------------------------------------------+
```

## Related Documentation

- [TYPED_TOOLS_GUIDE.md](./TYPED_TOOLS_GUIDE.md) - Type-safe input schemas
- [MCP Protocol Spec](https://spec.modelcontextprotocol.io/) - MCP annotations specification
- [Composition Architecture](../.claude/plans/) - Server composition design
