# Type-Safe Tool Annotations

MCP tool annotations provide metadata beyond schemas—hints about behavior, safety, and usage that help AI clients make better decisions. Combined with Rust's type system, annotations create a powerful safety net.

## What Are Tool Annotations?

Annotations are structured metadata attached to tools that describe characteristics the AI should consider:

```rust
use pmcp::types::ToolAnnotations;

let annotations = ToolAnnotations::new()
    .with_read_only(false)
    .with_destructive(true)
    .with_idempotent(false)
    .with_open_world(false);
```

These annotations tell the AI:
- This tool modifies data (not read-only)
- It can be destructive (data loss possible)
- It's not idempotent (calling twice has different effects)
- It operates on a closed world (internal database)

## Standard MCP Annotations

The MCP specification defines several standard annotation hints:

### `readOnlyHint`

Indicates whether the tool only reads data or can modify state:

```rust
// Read-only tool - safe to call speculatively
let annotations = ToolAnnotations::new()
    .with_read_only(true);

// Modifying tool - AI should confirm before calling
let annotations = ToolAnnotations::new()
    .with_read_only(false);
```

AI clients may call read-only tools more freely, while being cautious with modifying tools.

### `destructiveHint`

Indicates whether the operation can cause irreversible changes:

```rust
// Non-destructive: data can be recovered
let annotations = ToolAnnotations::new()
    .with_destructive(false);

// Destructive: data is permanently lost
let annotations = ToolAnnotations::new()
    .with_read_only(false)
    .with_destructive(true);
```

Some AI clients will refuse to call destructive tools without explicit user confirmation.

### `idempotentHint`

Indicates whether calling the tool multiple times has the same effect as calling once:

```rust
// Idempotent: safe to retry
let annotations = ToolAnnotations::new()
    .with_idempotent(true);

// Not idempotent: each call has cumulative effect
let annotations = ToolAnnotations::new()
    .with_idempotent(false);
```

AI clients can safely retry idempotent operations on failure.

### `openWorldHint`

Indicates whether the tool interacts with external systems:

```rust
// Closed world: internal database only
let annotations = ToolAnnotations::new()
    .with_open_world(false);

// Open world: calls external APIs
let annotations = ToolAnnotations::new()
    .with_open_world(true);
```

Open world tools may have rate limits, costs, or unpredictable latency.

## PMCP SDK: ToolAnnotations Builder

The PMCP SDK provides a fluent builder for creating type-safe annotations:

```rust
use pmcp::types::ToolAnnotations;
use serde_json::json;

// Build annotations with the fluent API
let annotations = ToolAnnotations::new()
    .with_read_only(true)
    .with_idempotent(true)
    .with_open_world(false);

// Create a tool with annotations
use pmcp::types::ToolInfo;

let tool = ToolInfo::with_annotations(
    "sales_query",
    Some("Query sales data from PostgreSQL 15".to_string()),
    json!({
        "type": "object",
        "properties": {
            "sql": { "type": "string" }
        }
    }),
    annotations,
);
```

### Combining with Output Schema

For tools that need both behavioral hints and output schemas, set the output schema
on `ToolInfo` (top-level per MCP spec 2025-06-18) and the type name in annotations:

```rust
let tool = ToolInfo::new("query", Some("Execute SQL".into()), input_schema)
    .with_output_schema(json!({
        "type": "object",
        "properties": {
            "rows": { "type": "array" },
            "count": { "type": "integer" }
        }
    }))
    .with_annotations(
        ToolAnnotations::new()
            .with_read_only(true)
            .with_output_type_name("QueryResult")
    );
```

## TypedTool and Annotations

The PMCP SDK provides full annotation support directly on `TypedTool`, `TypedSyncTool`, and `TypedToolWithOutput`. You can add annotations using either the `.with_annotations()` method or convenience methods like `.read_only()` and `.destructive()`.

### TypedTool with Annotations

```rust
use pmcp::server::typed_tool::TypedTool;
use pmcp::types::ToolAnnotations;
use schemars::JsonSchema;
use serde::Deserialize;

/// Input parameters for the delete tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteCustomerInput {
    /// Customer ID to permanently delete
    pub customer_id: String,

    /// Reason for deletion (required for audit log)
    pub reason: String,
}

// Full annotation support with TypedTool
let tool = TypedTool::new("delete_customer", |args: DeleteCustomerInput, _extra| {
    Box::pin(async move {
        if args.reason.len() < 10 {
            return Err(pmcp::Error::Validation(
                "Deletion reason must be at least 10 characters".into()
            ));
        }
        // Execute deletion...
        Ok(serde_json::json!({ "deleted": true, "customer_id": args.customer_id }))
    })
})
.with_description("Permanently delete a customer and all associated data")
.with_annotations(
    ToolAnnotations::new()
        .with_read_only(false)
        .with_destructive(true)     // Permanent deletion
        .with_idempotent(true)      // Deleting twice = same result
        .with_open_world(false)     // Internal database
);
```

### Convenience Methods

For common annotation patterns, use the convenience methods directly on the tool:

```rust
// Read-only query tool
let query_tool = TypedTool::new("sales_query", |args: QueryInput, _| {
    Box::pin(async move {
        // Execute read-only query...
        Ok(serde_json::json!({ "rows": [] }))
    })
})
.with_description("Query sales data from PostgreSQL 15")
.read_only()      // Sets readOnlyHint: true
.idempotent();    // Sets idempotentHint: true

// Destructive delete tool
let delete_tool = TypedTool::new("delete_record", |args: DeleteInput, _| {
    Box::pin(async move {
        // Execute deletion...
        Ok(serde_json::json!({ "deleted": true }))
    })
})
.with_description("Permanently delete a record")
.destructive()    // Sets readOnlyHint: false, destructiveHint: true
.idempotent();    // Safe to retry

// External API tool
let api_tool = TypedTool::new("fetch_stock_price", |args: StockInput, _| {
    Box::pin(async move {
        // Call external API...
        Ok(serde_json::json!({ "price": 150.25 }))
    })
})
.with_description("Fetch current stock price from market data API")
.read_only()
.open_world();    // Sets openWorldHint: true (external system)
```

### TypedToolWithOutput: Merged Annotations

When using `TypedToolWithOutput`, user-provided annotations are automatically merged with the auto-generated output schema annotation:

```rust
use pmcp::server::typed_tool::TypedToolWithOutput;
use pmcp::types::ToolAnnotations;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryInput {
    pub sql: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct QueryOutput {
    pub rows: Vec<serde_json::Value>,
    pub count: i64,
}

let tool = TypedToolWithOutput::new("query", |args: QueryInput, _| {
    Box::pin(async move {
        // Execute query...
        Ok(QueryOutput { rows: vec![], count: 0 })
    })
})
.with_description("Execute SQL query")
.read_only()      // User-provided: readOnlyHint: true
.idempotent();    // User-provided: idempotentHint: true

// The tool now has BOTH:
// - User annotations: readOnlyHint, idempotentHint, pmcp:outputTypeName
// - Auto-generated top-level: outputSchema on ToolInfo
```

### TypedSyncTool for Synchronous Handlers

For tools that don't need async, use `TypedSyncTool` with the same annotation support:

```rust
use pmcp::server::typed_tool::TypedSyncTool;

let tool = TypedSyncTool::new("calculate", |args: CalcInput, _extra| {
    // Synchronous computation
    Ok(serde_json::json!({ "result": args.a + args.b }))
})
.with_description("Perform calculation")
.read_only()
.idempotent();
```

## Annotation Patterns by Tool Type

### Query Tools (Read-Only)

```rust
let annotations = ToolAnnotations::new()
    .with_read_only(true)
    .with_idempotent(true)    // Same query = same results
    .with_open_world(false);  // Internal database
```

### External API Tools

```rust
let annotations = ToolAnnotations::new()
    .with_read_only(true)     // Just fetching data
    .with_open_world(true)    // Calls external API
    .with_idempotent(false);  // External state may change
```

### Update Tools (Modifying)

```rust
let annotations = ToolAnnotations::new()
    .with_read_only(false)
    .with_destructive(false)  // Updates are recoverable
    .with_idempotent(true);   // SET status='active' is idempotent
```

### Delete Tools (Destructive)

```rust
let annotations = ToolAnnotations::new()
    .with_read_only(false)
    .with_destructive(true)   // Permanent deletion
    .with_idempotent(true);   // Deleting twice = same result
```

### Insert Tools (Non-Idempotent)

```rust
let annotations = ToolAnnotations::new()
    .with_read_only(false)
    .with_destructive(false)
    .with_idempotent(false);  // Each insert creates new record
```

## Custom Annotations

Beyond standard hints, define custom annotations for your domain using the raw JSON approach:

### Using ToolInfo with Custom Fields

```rust
use pmcp::types::ToolInfo;

// Start with standard annotations
let mut annotations = ToolAnnotations::new()
    .with_read_only(false)
    .with_destructive(true);

// Create tool info
let mut tool = ToolInfo::with_annotations(
    "admin_reset",
    Some("Reset user password".into()),
    input_schema,
    annotations,
);

// Access the underlying _meta for custom fields if needed
// (Custom annotations beyond MCP standard hints)
```

### Domain-Specific Annotation Structs

For complex annotation needs, define your own structures:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainAnnotations {
    // MCP standard hints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,

    // Custom domain annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_role: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_log: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub requests_per_hour: u32,
}

impl DomainAnnotations {
    pub fn admin_only() -> Self {
        Self {
            read_only_hint: Some(false),
            destructive_hint: Some(true),
            idempotent_hint: None,
            requires_role: Some("admin".into()),
            audit_log: Some(true),
            rate_limit: None,
        }
    }

    pub fn external_api(rpm: u32) -> Self {
        Self {
            read_only_hint: Some(true),
            destructive_hint: Some(false),
            idempotent_hint: Some(false),
            requires_role: None,
            audit_log: None,
            rate_limit: Some(RateLimitConfig {
                requests_per_minute: rpm,
                requests_per_hour: rpm * 60,
            }),
        }
    }
}
```

## Runtime Behavior Based on Annotations

Use annotations to drive runtime behavior in your server:

```rust
pub async fn execute_tool(
    tool: &RegisteredTool,
    params: Value,
    context: &ExecutionContext,
) -> Result<Value> {
    if let Some(annotations) = &tool.annotations {
        // Check role requirements (custom annotation)
        if let Some(required_role) = annotations.requires_role.as_ref() {
            if !context.user_roles.contains(required_role) {
                return Err(Error::AccessDenied(format!(
                    "Tool '{}' requires role '{}'",
                    tool.name, required_role
                )));
            }
        }

        // Require confirmation for destructive operations
        if annotations.destructive_hint == Some(true) && !context.confirmed {
            return Err(Error::ConfirmationRequired(
                "This operation is destructive. Please confirm.".into()
            ));
        }

        // Log audit trail
        if annotations.audit_log == Some(true) {
            audit_log(&tool.name, &params, &context.user_id).await;
        }
    }

    // Execute the tool
    (tool.handler)(params, context).await
}
```

## Annotation-Driven Documentation

Generate documentation from annotations automatically:

```rust
pub fn generate_safety_docs(tool: &ToolInfo) -> String {
    let mut doc = String::new();

    if let Some(ann) = &tool.annotations {
        doc.push_str("### Safety Characteristics\n\n");

        if ann.read_only_hint == Some(true) {
            doc.push_str("- ✅ **Read-only**: Safe to call without modifying data\n");
        } else if ann.read_only_hint == Some(false) {
            doc.push_str("- ⚠️ **Modifies data**: This tool changes system state\n");
        }

        if ann.destructive_hint == Some(true) {
            doc.push_str("- ❌ **Destructive**: May cause irreversible changes\n");
        }

        if ann.idempotent_hint == Some(true) {
            doc.push_str("- 🔄 **Idempotent**: Safe to retry on failure\n");
        }

        if ann.open_world_hint == Some(true) {
            doc.push_str("- 🌐 **External**: Interacts with external systems\n");
        }
    }

    doc
}
```

## Summary

Tool annotations provide behavioral metadata that:

| Annotation | Purpose | AI Behavior |
|------------|---------|-------------|
| `readOnlyHint` | Read vs write | Controls speculation |
| `destructiveHint` | Irreversible changes | Requires confirmation |
| `idempotentHint` | Safe to retry | Retry on failure |
| `openWorldHint` | External systems | Expects latency/limits |
| `outputSchema` (top-level) | Output type | Enables composition |

### PMCP SDK Annotation Support

| Tool Type | Annotation Support |
|-----------|-------------------|
| `TypedTool` | Full: `.with_annotations()`, `.read_only()`, `.destructive()`, `.idempotent()`, `.open_world()` |
| `TypedSyncTool` | Full: Same methods as `TypedTool` |
| `TypedToolWithOutput` | Full: Same methods + auto-merges with output schema |
| `ToolInfo::with_annotations()` | Full: Direct construction with `ToolAnnotations` builder |
| Custom `ToolHandler` | Full control via `metadata()` method |

### Best Practices

1. **Always annotate destructive tools** - AI clients need this for user safety
2. **Mark read-only tools** - Enables faster AI exploration with `.read_only()`
3. **Indicate idempotency** - Helps with retry logic using `.idempotent()`
4. **Use TypedToolWithOutput** - Get output schema annotations automatically merged
5. **Chain convenience methods** - `.read_only().idempotent()` for common patterns

Annotations transform tools from opaque functions into self-describing components that AI clients can reason about safely.
