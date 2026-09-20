//! WASM-compatible server core implementation.
//!
//! This module provides a minimal server implementation that can compile
//! to WASM and be used in environments like Cloudflare Workers.

use crate::error::{ErrorCode, Result};
use crate::server::ProtocolHandler;
use crate::types::{JSONRPCError, JSONRPCResponse, Notification, Request, RequestId};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;

/// Tool handler function type for WASM.
type ToolHandler = Box<dyn Fn(Value) -> Result<Value>>;

/// A minimal WASM-compatible MCP server core.
///
/// This server provides basic MCP functionality that can run in WASM environments.
/// It supports tool registration and basic MCP protocol operations.
pub struct WasmServerCore {
    name: String,
    version: String,
    tools: HashMap<String, (String, ToolHandler)>, // name -> (description, handler)
    initialized: RefCell<bool>,
}

impl WasmServerCore {
    /// Create a new WASM server core.
    pub fn new(name: String, version: String) -> Self {
        Self {
            name,
            version,
            tools: HashMap::new(),
            initialized: RefCell::new(false),
        }
    }

    /// Add a tool to the server.
    pub fn add_tool<F>(&mut self, name: String, description: String, handler: F)
    where
        F: Fn(Value) -> Result<Value> + 'static,
    {
        self.tools.insert(name, (description, Box::new(handler)));
    }
}

#[async_trait(?Send)]
impl ProtocolHandler for WasmServerCore {
    async fn handle_request(&self, id: RequestId, request: Request) -> JSONRPCResponse {
        // Handle MCP protocol operations
        let result = match request {
            Request::Client(client_request) => {
                // We need to handle the boxed client request
                match serde_json::to_value(&*client_request) {
                    Ok(req_value) => {
                        // Extract the method from the request
                        if let Some(method) = req_value.get("method").and_then(|m| m.as_str()) {
                            match method {
                                "initialize" => {
                                    *self.initialized.borrow_mut() = true;
                                    Ok(json!({
                                        "protocolVersion": crate::LATEST_PROTOCOL_VERSION,
                                        "serverInfo": {
                                            "name": self.name,
                                            "version": self.version,
                                        },
                                        "capabilities": {
                                            "tools": {}
                                        }
                                    }))
                                },
                                "tools/list" => {
                                    // For stateless environments, allow listing without initialization
                                    {
                                        let tools: Vec<_> = self
                                            .tools
                                            .iter()
                                            .map(|(name, (desc, _))| {
                                                json!({
                                                    "name": name,
                                                    "description": desc,
                                                })
                                            })
                                            .collect();
                                        Ok(json!({
                                            "tools": tools
                                        }))
                                    }
                                },
                                "tools/call" => {
                                    // For stateless environments, allow tool calls without initialization
                                    if let Some(params) = req_value.get("params") {
                                        if let Some(tool_name) =
                                            params.get("name").and_then(|n| n.as_str())
                                        {
                                            if let Some((_, handler)) = self.tools.get(tool_name) {
                                                let args = params
                                                    .get("arguments")
                                                    .unwrap_or(&Value::Null)
                                                    .clone();
                                                match handler(args) {
                                                    Ok(result) => Ok(json!({
                                                        "content": [{
                                                            "type": "text",
                                                            "text": serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
                                                        }],
                                                        "isError": false
                                                    })),
                                                    Err(e) => Ok(json!({
                                                        "content": [{
                                                            "type": "text",
                                                            "text": format!("Error: {}", e)
                                                        }],
                                                        "isError": true
                                                    })),
                                                }
                                            } else {
                                                Err(JSONRPCError {
                                                    code: ErrorCode::METHOD_NOT_FOUND.0,
                                                    message: format!(
                                                        "Tool '{}' not found",
                                                        tool_name
                                                    ),
                                                    data: None,
                                                })
                                            }
                                        } else {
                                            Err(JSONRPCError {
                                                code: ErrorCode::INVALID_PARAMS.0,
                                                message: "Tool name is required".to_string(),
                                                data: None,
                                            })
                                        }
                                    } else {
                                        Err(JSONRPCError {
                                            code: ErrorCode::INVALID_PARAMS.0,
                                            message: "Parameters required for tools/call"
                                                .to_string(),
                                            data: None,
                                        })
                                    }
                                },
                                _ => Err(JSONRPCError {
                                    code: ErrorCode::METHOD_NOT_FOUND.0,
                                    message: format!("Method '{}' not supported in WASM", method),
                                    data: None,
                                }),
                            }
                        } else {
                            Err(JSONRPCError {
                                code: ErrorCode::INVALID_REQUEST.0,
                                message: "Could not determine request method".to_string(),
                                data: None,
                            })
                        }
                    },
                    Err(_) => Err(JSONRPCError {
                        code: ErrorCode::PARSE_ERROR.0,
                        message: "Failed to parse request".to_string(),
                        data: None,
                    }),
                }
            },
            Request::Server(_) => Err(JSONRPCError {
                code: ErrorCode::INVALID_REQUEST.0,
                message: "Server requests not supported in WASM".to_string(),
                data: None,
            }),
        };

        // Create response with proper structure
        match result {
            Ok(mut value) => {
                // The pmcp-internal MRTR signal carries the handler's PLAINTEXT
                // continuation — the very state the AEAD `requestState` token
                // exists to seal. `server::core`'s `strip_mrtr_signal` /
                // `scrub_mrtr_signal` are both `cfg(not(target_arch = "wasm32"))`,
                // so this is the wasm half of the "removed on EVERY path before
                // serialization" guarantee that
                // `crate::types::mrtr::MRTR_SIGNAL_META_KEY` documents. The key
                // and `MrtrSignal` are `pub` on every target, so a handler can
                // write them here even though this core has no MRTR egress to
                // consume them.
                if crate::types::mrtr::remove_mrtr_signal(&mut value).is_some() {
                    tracing::warn!(
                        target: "mcp.mrtr",
                        field = crate::types::mrtr::MRTR_SIGNAL_META_KEY,
                        "removed the pmcp-internal MRTR signal from an outgoing result"
                    );
                }
                JSONRPCResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    payload: crate::types::jsonrpc::ResponsePayload::Result(value),
                }
            },
            Err(error) => JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id,
                payload: crate::types::jsonrpc::ResponsePayload::Error(error),
            },
        }
    }

    async fn handle_notification(&self, _notification: Notification) -> Result<()> {
        // In WASM, we'll just acknowledge notifications without processing
        Ok(())
    }
}

#[cfg(test)]
#[path = "wasm_core_tests.rs"]
mod wasm_core_tests;

impl std::fmt::Debug for WasmServerCore {
    /// Hand-written: the tool registry holds boxed handlers, which are not
    /// `Debug` and must not be forced to be.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmServerCore")
            .field("name", &self.name)
            .field("version", &self.version)
            .field("tools", &self.tools.len())
            .finish_non_exhaustive()
    }
}
