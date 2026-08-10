//! Data Visualization MCP Server with Chart.js Widget
//!
//! This example demonstrates a Chinook SQLite Explorer with an interactive
//! dashboard widget that renders bar, line, and pie charts from SQL query
//! results, plus a sortable data table.
//!
//! # Architecture
//!
//! - Each tool defines both **input** and **output** schemas via `TypedToolWithOutput`.
//! - The SDK automatically populates `structuredContent` in the tool result so the
//!   host (ChatGPT, Claude Desktop, etc.) can push data to the widget.
//! - The widget receives data through **two channels**:
//!   1. Host-pushed `ui/notifications/tool-result` with `structuredContent` (LLM-initiated)
//!   2. Widget-initiated `mcpBridge.callTool()` (user clicks in the UI)
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
use pmcp::server::mcp_apps::{McpAppsAdapter, UIAdapter, WidgetDir};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::typed_tool::TypedToolWithOutput;
use pmcp::server::ServerBuilder;
use pmcp::types::mcp_apps::{ExtendedUIMimeType, HostType};
use pmcp::types::Content;
use pmcp::types::{ListResourcesResult, ReadResourceResult, ResourceInfo};
use pmcp::{RequestHandlerExtra, ResourceHandler, Result};
use rusqlite::{types::Value as SqlValue, Connection};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
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
// Tool Output Types
// =============================================================================

/// Result of executing a SQL query.
#[derive(Debug, Serialize, JsonSchema)]
pub struct QueryResult {
    /// Column names from the result set.
    pub columns: Vec<String>,
    /// Rows as arrays of values (each row matches the columns order).
    pub rows: Vec<Vec<Value>>,
    /// Number of rows returned.
    pub row_count: usize,
}

/// List of tables in the database.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TableListResult {
    /// Table names in the database.
    pub tables: Vec<String>,
}

/// Column metadata for a single column.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ColumnInfo {
    /// Column name.
    pub name: String,
    /// SQL data type.
    #[serde(rename = "type")]
    pub col_type: String,
    /// Whether the column allows NULL values.
    pub nullable: bool,
    /// Whether this column is a primary key.
    pub primary_key: bool,
}

/// Schema description for a database table.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TableDescription {
    /// Name of the table.
    pub table_name: String,
    /// Column metadata.
    pub columns: Vec<ColumnInfo>,
}

// =============================================================================
// Database Helpers
// =============================================================================

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

fn execute_query_handler(input: ExecuteQueryInput, _extra: RequestHandlerExtra) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<QueryResult>> + Send>> {
    Box::pin(async move {
        let db = open_db().map_err(pmcp::Error::Internal)?;

        let mut stmt = db.prepare(&input.sql)
            .map_err(|e| pmcp::Error::Internal(format!("SQL error: {}", e)))?;

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
                Ok(QueryResult { columns, rows, row_count })
            }
            Err(e) => Err(pmcp::Error::Internal(format!("Query execution error: {}", e))),
        }
    })
}

fn list_tables_handler(_input: ListTablesInput, _extra: RequestHandlerExtra) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TableListResult>> + Send>> {
    Box::pin(async move {
        let db = open_db().map_err(pmcp::Error::Internal)?;

        let mut stmt = db.prepare(
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
        ).map_err(|e| pmcp::Error::Internal(format!("SQL error: {}", e)))?;

        let tables_result: std::result::Result<Vec<String>, _> = stmt
            .query_map([], |row| row.get(0))
            .and_then(|mapped| mapped.collect());

        match tables_result {
            Ok(tables) => Ok(TableListResult { tables }),
            Err(e) => Err(pmcp::Error::Internal(format!("Query error: {}", e))),
        }
    })
}

fn describe_table_handler(input: DescribeTableInput, _extra: RequestHandlerExtra) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TableDescription>> + Send>> {
    Box::pin(async move {
        if !input.table_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(pmcp::Error::Validation(
                "Invalid table name: only alphanumeric characters and underscores are allowed".to_string()
            ));
        }

        let db = open_db().map_err(pmcp::Error::Internal)?;

        let pragma_sql = format!("PRAGMA table_info({})", input.table_name);
        let mut stmt = db.prepare(&pragma_sql)
            .map_err(|e| pmcp::Error::Internal(format!("SQL error: {}", e)))?;

        let columns_result: std::result::Result<Vec<ColumnInfo>, _> = stmt
            .query_map([], |row| {
                let name: String = row.get(1)?;
                let col_type: String = row.get(2)?;
                let notnull: bool = row.get(3)?;
                let pk: bool = row.get(5)?;
                Ok(ColumnInfo { name, col_type, nullable: !notnull, primary_key: pk })
            })
            .and_then(|mapped| mapped.collect());

        match columns_result {
            Ok(columns) => Ok(TableDescription { table_name: input.table_name, columns }),
            Err(e) => Err(pmcp::Error::Internal(format!("Query error: {}", e))),
        }
    })
}

// =============================================================================
// Resource Handler
// =============================================================================

struct DataVizResources {
    adapter: McpAppsAdapter,
    widget_dir: WidgetDir,
}

impl DataVizResources {
    fn new(widgets_path: PathBuf) -> Self {
        Self {
            adapter: McpAppsAdapter::new(),
            widget_dir: WidgetDir::new(widgets_path),
        }
    }
}

#[async_trait]
impl ResourceHandler for DataVizResources {
    async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        let name = uri
            .strip_prefix("ui://app/")
            .or_else(|| uri.strip_prefix("ui://dataviz/"))
            .and_then(|s| s.strip_suffix(".html").or(Some(s)));

        if let Some(widget_name) = name {
            let html = self.widget_dir.read_widget(widget_name);
            let transformed = self.adapter.transform(uri, widget_name, &html);

            // `Content::Resource` is `#[non_exhaustive]` as of pmcp 2.19.0
            // (Phase 118.1, G-1/G-2), so it is built through the constructors.
            Ok(ReadResourceResult::new(vec![Content::resource_with_text(
                uri,
                transformed.content,
                ExtendedUIMimeType::HtmlMcpApp.to_string(),
            )]))
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
            .map(|entry| {
                ResourceInfo::new(&entry.uri, &entry.filename)
                    .with_description(format!("Interactive {} widget", entry.filename))
                    .with_mime_type(ExtendedUIMimeType::HtmlMcpApp.to_string())
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
    let widgets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("widgets");

    let server = ServerBuilder::new()
        .name("dataviz-server")
        .version("1.0.0")
        .tool(
            "execute_query",
            TypedToolWithOutput::new("execute_query", execute_query_handler)
                .with_description("Execute a SQL query against the Chinook database. Returns columns, rows, and row count as structured JSON.")
                .with_ui("ui://app/dashboard"),
        )
        .tool(
            "list_tables",
            TypedToolWithOutput::new("list_tables", list_tables_handler)
                .with_description("List all tables in the Chinook database.")
                .with_ui("ui://app/dashboard"),
        )
        .tool(
            "describe_table",
            TypedToolWithOutput::new("describe_table", describe_table_handler)
                .with_description("Get column metadata (name, type, nullable, primary key) for a specific table.")
                .with_ui("ui://app/dashboard"),
        )
        .resources(DataVizResources::new(widgets_path))
        .with_host_layer(HostType::ChatGpt)
        .build()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let server = Arc::new(Mutex::new(server));

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3002u16);
    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);

    let config = StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
        // PRE-EXISTING breakage, not introduced by Phase 118.1: this literal has
        // been missing `allowed_origins` / `max_request_bytes` since those fields
        // were added, and this crate is workspace-EXCLUDED so no gate caught it.
        // Filled from `Default` so the crate compiles and the Content fix above
        // can actually be proven.
        ..Default::default()
    };

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

    server_handle.await.map_err(|e| {
        Box::new(pmcp::Error::Internal(e.to_string())) as Box<dyn std::error::Error>
    })?;

    Ok(())
}
