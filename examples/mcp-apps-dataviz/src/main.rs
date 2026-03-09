//! Data Visualization MCP Server with Chart.js Widget
//!
//! This example demonstrates a Chinook SQLite Explorer with an interactive
//! dashboard widget that renders bar, line, and pie charts from SQL query
//! results, plus a sortable data table.
//!
//! # Architecture
//!
//! The server opens a local Chinook SQLite database and exposes three tools:
//! - `execute_query`: Run arbitrary SQL and get structured JSON results
//! - `list_tables`: Enumerate all tables in the database
//! - `describe_table`: Get column metadata for a specific table
//!
//! The widget uses Chart.js (loaded from CDN) for visualization and vanilla
//! JavaScript for a sortable data table.
//!
//! # Prerequisites
//!
//! Download the Chinook database before running:
//! ```bash
//! cd examples/mcp-apps-dataviz
//! curl -L -o Chinook.db https://github.com/lerocha/chinook-database/releases/download/v1.4.5/Chinook_Sqlite.sqlite
//! ```
//!
//! # Running
//!
//! ```bash
//! cd examples/mcp-apps-dataviz
//! cargo run
//! ```
//!
//! Then connect with `cargo pmcp connect` or via HTTP at http://localhost:3002

use async_trait::async_trait;
use pmcp::server::mcp_apps::{ChatGptAdapter, UIAdapter, WidgetDir};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::ServerBuilder;
use pmcp::types::mcp_apps::{ExtendedUIMimeType, WidgetMeta};
use pmcp::types::protocol::Content;
use pmcp::types::{ListResourcesResult, ReadResourceResult, ResourceInfo};
use pmcp::{RequestHandlerExtra, ResourceHandler, Result};
use rusqlite::{types::Value as SqlValue, Connection};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

// =============================================================================
// Tool Input Types
// =============================================================================

/// Input for the `execute_query` tool.
#[derive(Deserialize, JsonSchema)]
struct ExecuteQueryInput {
    /// The SQL query to execute against the Chinook database.
    sql: String,
}

/// Input for the `list_tables` tool (no parameters needed).
#[derive(Deserialize, JsonSchema)]
struct ListTablesInput {}

/// Input for the `describe_table` tool.
#[derive(Deserialize, JsonSchema)]
struct DescribeTableInput {
    /// The name of the table to describe.
    table_name: String,
}

// =============================================================================
// Database Helpers
// =============================================================================

/// Open the Chinook SQLite database from the example directory.
///
/// Returns a helpful error message if the database file is not found,
/// directing the user to download it.
fn open_db() -> std::result::Result<Connection, String> {
    let db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Chinook.db");
    if !db_path.exists() {
        return Err(format!(
            "Chinook.db not found at {}. Please download it:\n\
             cd examples/mcp-apps-dataviz\n\
             curl -L -o Chinook.db https://github.com/lerocha/chinook-database/releases/download/v1.4.5/Chinook_Sqlite.sqlite",
            db_path.display()
        ));
    }
    Connection::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))
}

/// Convert a rusqlite Value to a serde_json Value.
fn sql_value_to_json(val: SqlValue) -> Value {
    match val {
        SqlValue::Null => Value::Null,
        SqlValue::Integer(i) => json!(i),
        SqlValue::Real(f) => json!(f),
        SqlValue::Text(s) => json!(s),
        SqlValue::Blob(b) => json!(format!("<blob {} bytes>", b.len())),
    }
}

// =============================================================================
// Tool Handlers
// =============================================================================

/// Execute an arbitrary SQL query and return structured results.
fn execute_query_handler(input: ExecuteQueryInput, _extra: RequestHandlerExtra) -> Result<Value> {
    let db = match open_db() {
        Ok(db) => db,
        Err(msg) => return Ok(json!({ "error": msg })),
    };

    let mut stmt = match db.prepare(&input.sql) {
        Ok(s) => s,
        Err(e) => return Ok(json!({ "error": format!("SQL error: {}", e) })),
    };

    let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();

    let rows_result: std::result::Result<Vec<Vec<Value>>, _> = stmt
        .query_map([], |row| {
            let mut row_values = Vec::with_capacity(columns.len());
            for i in 0..columns.len() {
                let val: SqlValue = row.get_unwrap(i);
                row_values.push(sql_value_to_json(val));
            }
            Ok(row_values)
        })
        .and_then(|mapped| mapped.collect());

    match rows_result {
        Ok(rows) => {
            let row_count = rows.len();
            Ok(json!({
                "columns": columns,
                "rows": rows,
                "row_count": row_count
            }))
        }
        Err(e) => Ok(json!({ "error": format!("Query execution error: {}", e) })),
    }
}

/// List all tables in the Chinook database.
fn list_tables_handler(_input: ListTablesInput, _extra: RequestHandlerExtra) -> Result<Value> {
    let db = match open_db() {
        Ok(db) => db,
        Err(msg) => return Ok(json!({ "error": msg })),
    };

    let mut stmt = match db.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
    ) {
        Ok(s) => s,
        Err(e) => return Ok(json!({ "error": format!("SQL error: {}", e) })),
    };

    let tables_result: std::result::Result<Vec<String>, _> = stmt
        .query_map([], |row| row.get(0))
        .and_then(|mapped| mapped.collect());

    match tables_result {
        Ok(tables) => Ok(json!({ "tables": tables })),
        Err(e) => Ok(json!({ "error": format!("Query error: {}", e) })),
    }
}

/// Describe the columns of a specific table.
fn describe_table_handler(
    input: DescribeTableInput,
    _extra: RequestHandlerExtra,
) -> Result<Value> {
    // Validate table name: only allow alphanumeric characters and underscores
    // to prevent SQL injection via PRAGMA.
    if !input
        .table_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_')
    {
        return Ok(
            json!({ "error": "Invalid table name: only alphanumeric characters and underscores are allowed" }),
        );
    }

    let db = match open_db() {
        Ok(db) => db,
        Err(msg) => return Ok(json!({ "error": msg })),
    };

    let pragma_sql = format!("PRAGMA table_info({})", input.table_name);
    let mut stmt = match db.prepare(&pragma_sql) {
        Ok(s) => s,
        Err(e) => return Ok(json!({ "error": format!("SQL error: {}", e) })),
    };

    let columns_result: std::result::Result<Vec<Value>, _> = stmt
        .query_map([], |row| {
            let name: String = row.get(1)?;
            let col_type: String = row.get(2)?;
            let notnull: bool = row.get(3)?;
            let pk: bool = row.get(5)?;
            Ok(json!({
                "name": name,
                "type": col_type,
                "nullable": !notnull,
                "primary_key": pk
            }))
        })
        .and_then(|mapped| mapped.collect());

    match columns_result {
        Ok(columns) => Ok(json!({
            "table_name": input.table_name,
            "columns": columns
        })),
        Err(e) => Ok(json!({ "error": format!("Query error: {}", e) })),
    }
}

// =============================================================================
// Resource Handler
// =============================================================================

/// Data visualization resource handler that serves widgets from the `widgets/` directory.
///
/// Uses `WidgetDir` for file-based widget discovery and hot-reload: widget HTML
/// is read from disk on every request, so a browser refresh shows the latest
/// content without server restart.
struct DataVizResources {
    /// ChatGPT adapter for injecting the skybridge bridge.
    chatgpt_adapter: ChatGptAdapter,
    /// Widget directory scanner for file-based hot-reload.
    widget_dir: WidgetDir,
}

impl DataVizResources {
    fn new(widgets_path: PathBuf) -> Self {
        let widget_meta = WidgetMeta::new()
            .prefers_border(true)
            .description("Interactive data explorer - run SQL queries and visualize results as charts and tables");

        let chatgpt_adapter = ChatGptAdapter::new().with_widget_meta(widget_meta);
        let widget_dir = WidgetDir::new(widgets_path);

        Self {
            chatgpt_adapter,
            widget_dir,
        }
    }
}

#[async_trait]
impl ResourceHandler for DataVizResources {
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        // Extract widget name from URI (e.g., "ui://app/dashboard" -> "dashboard")
        let name = uri
            .strip_prefix("ui://app/")
            .or_else(|| uri.strip_prefix("ui://dataviz/"))
            .and_then(|s| s.strip_suffix(".html").or(Some(s)));

        if let Some(widget_name) = name {
            let html = self.widget_dir.read_widget(widget_name);
            let mut transformed = self.chatgpt_adapter.transform(uri, widget_name, &html);
            let meta = transformed.take_meta();

            Ok(ReadResourceResult::new(vec![Content::Resource {
                    uri: uri.to_string(),
                    text: Some(transformed.content),
                    mime_type: Some(ExtendedUIMimeType::HtmlMcpApp.to_string()),
                    meta,
                }]))
        } else {
            Err(pmcp::Error::protocol(
                pmcp::ErrorCode::METHOD_NOT_FOUND,
                format!("Resource not found: {}", uri),
            ))
        }
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        let entries = self.widget_dir.discover().unwrap_or_default();
        let resources = entries
            .into_iter()
            .map(|entry| ResourceInfo {
                uri: entry.uri,
                name: entry.filename.clone(),
                description: Some(format!("Interactive {} widget", entry.filename)),
                mime_type: Some(ExtendedUIMimeType::HtmlMcpApp.to_string()),
                meta: None,
            })
            .collect();

        Ok(ListResourcesResult::new(resources))
    }
}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Resolve widgets directory relative to the binary's source location
    let widgets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("widgets");

    // Build server
    let server = ServerBuilder::new()
        .name("dataviz-server")
        .version("1.0.0")
        .tool_typed_sync_with_description(
            "execute_query",
            "Execute a SQL query against the Chinook database. Returns columns, rows, and row count as structured JSON.",
            execute_query_handler,
        )
        .tool_typed_sync_with_description(
            "list_tables",
            "List all tables in the Chinook database.",
            list_tables_handler,
        )
        .tool_typed_sync_with_description(
            "describe_table",
            "Get column metadata (name, type, nullable, primary key) for a specific table.",
            describe_table_handler,
        )
        .resources(DataVizResources::new(widgets_path))
        .build()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // Wrap server in Arc<Mutex<>> for sharing
    let server = Arc::new(Mutex::new(server));

    // Configure HTTP server address
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3002u16);
    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);

    // Create stateless HTTP server configuration
    let config = StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
    };

    // Create and start the HTTP server
    let http_server = StreamableHttpServer::with_config(addr, server, config);
    let (bound_addr, server_handle) = http_server
        .start()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    println!("Data Viz MCP Server running at http://{}", bound_addr);
    println!();
    println!("Available tools:");
    println!("  - execute_query:   Run SQL queries against Chinook database");
    println!("  - list_tables:     List all tables in the database");
    println!("  - describe_table:  Get column metadata for a table");
    println!();
    println!(
        "Connect with: cargo pmcp connect --server dataviz --client claude-code --url http://{}",
        bound_addr
    );
    println!();
    println!("Press Ctrl+C to stop");

    // Keep the server running
    server_handle.await.map_err(|e| {
        Box::new(pmcp::Error::Internal(e.to_string())) as Box<dyn std::error::Error>
    })?;

    Ok(())
}
