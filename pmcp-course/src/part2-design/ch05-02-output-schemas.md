# Output Schemas for Composition

Input validation prevents errors. Output schemas enable composition. When AI clients know what your tool returns, they can chain operations together confidently.

## The Composition Challenge

Consider an AI trying to use two tools together:

```
User: "Get our top customers and analyze their recent orders"

AI reasoning:
1. Use sales_top_customers to get customer list
2. For each customer, use order_history to get orders
3. Analyze patterns across all orders

But wait:
- What does sales_top_customers return?
- Is there a customer_id field? Or is it id? Or customer?
- What format is the response in?
- How do I iterate over the results?
```

Without knowing the output structure, the AI must guess—or execute the first tool and inspect results before continuing.

## The PMCP SDK Approach: TypedToolWithOutput

Just as `TypedTool` auto-generates input schemas from Rust structs, PMCP provides `TypedToolWithOutput` that generates **both input AND output schemas** automatically:

```rust
use pmcp::TypedToolWithOutput;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Input: Query parameters for top customers
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TopCustomersInput {
    /// Time period for revenue calculation
    period: Period,

    /// Maximum number of customers to return (1-100)
    #[serde(default = "default_limit")]
    limit: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Period {
    Month,
    Quarter,
    Year,
}

fn default_limit() -> u32 { 10 }

/// Output: List of top customers with revenue data
#[derive(Debug, Serialize, JsonSchema)]
pub struct TopCustomersOutput {
    /// List of customers sorted by revenue (highest first)
    pub customers: Vec<CustomerSummary>,

    /// The period that was queried
    pub period: String,

    /// When this report was generated (ISO 8601)
    pub generated_at: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CustomerSummary {
    /// Unique customer identifier - use with order_history, customer_details
    pub customer_id: String,

    /// Customer display name
    pub name: String,

    /// Total revenue in USD cents (divide by 100 for dollars)
    pub total_revenue: i64,

    /// Number of orders in the period
    pub order_count: u32,

    /// Most recent order date (ISO 8601)
    pub last_order_date: String,
}
```

Now create the tool with both schemas auto-generated:

```rust
let top_customers_tool = TypedToolWithOutput::new(
    "sales_top_customers",
    |args: TopCustomersInput, _extra| {
        Box::pin(async move {
            let customers = fetch_top_customers(&args.period, args.limit).await?;

            Ok(TopCustomersOutput {
                customers,
                period: format!("{:?}", args.period).to_lowercase(),
                generated_at: chrono::Utc::now().to_rfc3339(),
            })
        })
    }
)
.with_description(
    "Get top customers by revenue for a time period. \
    Returns customer_id values that work with order_history and customer_details tools."
);
```

The PMCP SDK automatically:
1. Generates `inputSchema` from `TopCustomersInput`
2. Generates `outputSchema` from `TopCustomersOutput` as a top-level field on `ToolInfo`
3. Sets `pmcp:outputTypeName` in annotations for code generation

### Doc Comments → Schema Descriptions

Just like input schemas, `///` doc comments become field descriptions:

```json
{
  "type": "object",
  "properties": {
    "customers": {
      "type": "array",
      "description": "List of customers sorted by revenue (highest first)",
      "items": {
        "type": "object",
        "properties": {
          "customer_id": {
            "type": "string",
            "description": "Unique customer identifier - use with order_history, customer_details"
          },
          "total_revenue": {
            "type": "integer",
            "description": "Total revenue in USD cents (divide by 100 for dollars)"
          }
        }
      }
    }
  }
}
```

The AI now knows:
- Results are in a `customers` array
- Each customer has `customer_id` (not `id` or `customer`)
- Revenue is in cents (needs division for dollars)
- `customer_id` works with other tools

## MCP Structured Content

MCP supports returning both human-readable text and structured data in tool responses. This enables AI clients to display friendly output while having typed data for processing:

```rust
use serde_json::json;

// Inside your tool handler
Ok(json!({
    "content": [{
        "type": "text",
        "text": format!("Found {} top customers for {}",
                       output.customers.len(), output.period)
    }],
    "structuredContent": output,  // The typed TopCustomersOutput
    "isError": false
}))
```

AI clients see:
- **content**: Human-readable summary for display
- **structuredContent**: Typed data matching your output schema

### The Structured Response Pattern

```rust
use pmcp::Error;
use serde::Serialize;

/// Helper to create MCP-compliant responses with structured content
pub fn structured_response<T: Serialize>(
    summary: &str,
    data: T,
) -> Result<serde_json::Value, Error> {
    Ok(json!({
        "content": [{
            "type": "text",
            "text": summary
        }],
        "structuredContent": data,
        "isError": false
    }))
}

/// Helper for structured error responses
pub fn structured_error<T: Serialize>(
    message: &str,
    error_data: T,
) -> Result<serde_json::Value, Error> {
    Ok(json!({
        "content": [{
            "type": "text",
            "text": message
        }],
        "structuredContent": error_data,
        "isError": true
    }))
}

// Usage in tool handler
let output = fetch_top_customers(&args).await?;
structured_response(
    &format!("Found {} top customers", output.customers.len()),
    output
)
```

## Consistent Response Envelopes

Design output schemas with consistent patterns across all tools:

### The Standard Envelope

```rust
#[derive(Debug, Serialize, JsonSchema)]
pub struct ToolResponse<T> {
    /// Whether the operation succeeded
    pub success: bool,

    /// Tool-specific response data (present when success=true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,

    /// Error details (present when success=false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorDetail>,

    /// Execution metadata
    pub metadata: ResponseMetadata,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ResponseMetadata {
    /// Query execution time in milliseconds
    pub execution_time_ms: u64,

    /// Data source identifier
    pub source: String,

    /// Whether results came from cache
    pub cached: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ErrorDetail {
    /// Machine-readable error code
    pub code: String,

    /// Human-readable error message
    pub message: String,

    /// Additional error context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}
```

With a consistent envelope, the AI learns one pattern for all your tools:

```
if response.success {
    process(response.data)
} else {
    handle_error(response.error)
}
```

### Implementation

```rust
impl<T: Serialize> ToolResponse<T> {
    pub fn success(data: T, metadata: ResponseMetadata) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            metadata,
        }
    }

    pub fn error(error: ErrorDetail, metadata: ResponseMetadata) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            metadata,
        }
    }
}
```

## Designing for Chaining

Structure outputs to support common chaining patterns:

### IDs for Follow-up Operations

When a tool returns entities, include IDs that work with other tools:

```rust
#[derive(Debug, Serialize, JsonSchema)]
pub struct CustomerSummary {
    /// Unique customer identifier - use with order_history, customer_details tools
    pub customer_id: String,

    /// Customer display name
    pub name: String,
    // ...
}

// Document the relationship in the tool receiving the ID
#[derive(Debug, Deserialize, JsonSchema)]
pub struct OrderHistoryInput {
    /// Customer ID from sales_top_customers or customer_search
    pub customer_id: String,

    /// Maximum orders to return
    #[serde(default = "default_order_limit")]
    pub limit: u32,
}
```

The AI sees `customer_id` in both schemas and understands how to chain them.

### Pagination Cursors

For paginated results, return consistent cursor information:

```rust
#[derive(Debug, Serialize, JsonSchema)]
pub struct PaginatedResponse<T> {
    /// The result items for this page
    pub results: Vec<T>,

    /// Pagination metadata
    pub pagination: PaginationInfo,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PaginationInfo {
    /// Total number of results available
    pub total_count: u64,

    /// Number of results per page
    pub page_size: u32,

    /// Whether more results are available
    pub has_more: bool,

    /// Pass to 'cursor' parameter to get next page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}
```

The AI learns: if `has_more` is true, call again with `cursor: next_cursor`.

### Aggregation-Ready Data

When data might be aggregated, use consistent numeric fields:

```rust
#[derive(Debug, Serialize, JsonSchema)]
pub struct SalesMetrics {
    /// Revenue in USD cents (divide by 100 for dollars)
    pub revenue_cents: i64,

    /// Number of units sold
    pub quantity: u32,

    /// Percentage as decimal (0.15 = 15%)
    pub growth_rate: f64,
}
```

## Type-Safe Server Composition

Output schemas become even more powerful when servers call other servers. PMCP enables **type-safe composition** through code generation.

### The Problem: Composition Type Blindness

When one MCP server calls another, you lose type information:

```rust
// Without output schemas - what shape does result have?
let result: Value = composition_client
    .call_tool("sqlite-explorer", "query", json!({"sql": "SELECT * FROM orders"}))
    .await?;

// Must guess or parse manually - error prone!
let rows = result["rows"].as_array().ok_or("expected rows")?;
```

### The Solution: Generated Typed Clients

PMCP can generate typed clients from servers with output schemas:

```bash
# Export schema from running server
cargo pmcp schema export --endpoint https://my-server.pmcp.run/mcp \
    --output my-server-schema.json

# Generate typed Rust client
cargo pmcp generate --schema my-server-schema.json \
    --output src/clients/my_server.rs
```

The generated code includes both input AND output types:

```rust
//! Auto-generated typed client for sqlite-explorer

/// Arguments for query tool
#[derive(Debug, Serialize)]
pub struct QueryArgs {
    /// SQL query to execute
    pub sql: String,
}

/// Result from query tool (from top-level outputSchema)
#[derive(Debug, Deserialize)]
pub struct QueryResult {
    /// Column names from the result set
    pub columns: Vec<String>,
    /// Row data as arrays of values
    pub rows: Vec<Vec<serde_json::Value>>,
    /// Total number of rows returned
    pub row_count: i64,
}

/// Typed client for sqlite-explorer server
impl SqliteExplorerClient {
    /// Execute SQL query and return results
    pub async fn query(&self, args: QueryArgs) -> Result<QueryResult, Error> {
        // Type-safe call with automatic serialization/deserialization
    }
}
```

Now your domain server has full type safety:

```rust
// In your domain server composing sqlite-explorer
let result: QueryResult = sqlite_client
    .query(QueryArgs { sql: "SELECT * FROM orders".into() })
    .await?;

// Compiler-checked field access!
println!("Found {} rows with {} columns",
         result.row_count, result.columns.len());

for row in &result.rows {
    // Process typed data
}
```

### Output Schema on ToolInfo

Per MCP spec 2025-06-18, output schemas are top-level fields on `ToolInfo`:

```rust
use pmcp::types::{ToolInfo, ToolAnnotations};

let tool = ToolInfo::new("query", Some("Execute SQL".into()), input_schema)
    .with_output_schema(schemars::schema_for!(QueryResult))
    .with_annotations(
        ToolAnnotations::new()
            .with_read_only(true)
            .with_output_type_name("QueryResult")  // PMCP codegen extension
    );
```

The exported tool metadata includes:

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

The `pmcp:outputTypeName` annotation is a PMCP codegen extension. Standard MCP clients ignore `pmcp:*` annotations per the spec.

## Schema Validation Best Practices

### 1. Validate Outputs Before Returning

Just as you validate inputs, validate outputs:

```rust
pub async fn generate_report(params: ReportInput) -> Result<ReportOutput, Error> {
    let report = build_report(&params).await?;

    // Validate output matches business rules
    if report.total_revenue < 0 {
        return Err(Error::Internal(
            "Generated report has negative revenue - data integrity issue".into()
        ));
    }

    if report.customer_id.is_empty() {
        return Err(Error::Internal(
            "Generated report missing customer_id".into()
        ));
    }

    Ok(report)
}
```

### 2. Match Output Schema to Actual Return Values

When using `TypedToolWithOutput`, this is enforced by the compiler:

```rust
// Compiler error if you return wrong type!
TypedToolWithOutput::new("my_tool", |args: Input, _| {
    Box::pin(async move {
        Ok(Output { ... })  // Must match Output type exactly
    })
})
```

### 3. Document Field Relationships in Comments

```rust
#[derive(Debug, Serialize, JsonSchema)]
pub struct CustomerSummary {
    /// Unique customer ID. Use with:
    /// - order_history: Get customer's order history
    /// - customer_details: Get full customer profile
    /// - customer_contacts: Get customer contact list
    pub customer_id: String,
}
```

### 4. Use Descriptive Type Names

The output type name becomes the generated struct name:

```rust
// Good: Clear, descriptive name
#[derive(Debug, Serialize, JsonSchema)]
pub struct OrderQueryResult { ... }

// Bad: Generic name causes conflicts
pub struct Result { ... }
```

## Summary

Output schemas enable composition by telling AI clients:

| What to Document | Why It Matters |
|------------------|----------------|
| **Field names and types** | AI constructs follow-up operations correctly |
| **ID relationships** | AI knows how to chain tools together |
| **Consistent envelopes** | AI learns one pattern for all your tools |
| **Error structures** | AI can handle failures gracefully |
| **Units and formats** | AI interprets values correctly |
| **Pagination patterns** | AI knows how to get more results |

### PMCP SDK Benefits

| Manual JSON Schema | TypedToolWithOutput |
|--------------------|---------------------|
| Schema and code can drift | Schema generated from code—always in sync |
| Manual JSON construction | Rust types with derive macros |
| No code generation | Generate typed clients for composition |
| Runtime type errors | Compile-time type safety |
| Verbose documentation | Doc comments become schema descriptions |

Remember: output schemas are a contract. The AI trusts that your tool returns what you declare. With `TypedToolWithOutput`, the Rust compiler ensures you keep that contract.
