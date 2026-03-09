// Allow doc_markdown since this module has many technical terms (ChatGPT, MCP-UI, etc.)
#![allow(clippy::doc_markdown)]

//! MCP Apps Extension - Interactive UI support for multiple MCP hosts.
//!
//! This module provides adapters that transform core UI types for specific MCP host platforms:
//!
//! - **ChatGPT Apps** - OpenAI Apps SDK with `window.openai` API
//! - **MCP Apps (SEP-1865)** - Standard MCP extension using postMessage
//! - **MCP-UI** - Community standard supporting multiple UI formats
//!
//! # Architecture
//!
//! The adapter pattern allows a single tool implementation to work across multiple hosts:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                       UIResource (Core)                          │
//! │                                                                  │
//! │   ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐ │
//! │   │ ChatGptAdapter│    │ McpAppsAdapter│    │   McpUiAdapter   │ │
//! │   └──────────────┘    └──────────────┘    └──────────────────┘ │
//! │           │                   │                    │            │
//! │           ▼                   ▼                    ▼            │
//! │   text/html;profile=   text/html+mcp        text/html           │
//! │     mcp-app                                                    │
//! │   window.openai         postMessage          postMessage        │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use pmcp::server::mcp_apps::{UIAdapter, ChatGptAdapter, MultiPlatformResource};
//! use pmcp::types::ui::UIResource;
//!
//! // Create a core UI resource
//! let resource = UIResource::new("ui://chess/board.html", "Chess Board");
//!
//! // Transform for ChatGPT Apps
//! let chatgpt_adapter = ChatGptAdapter::new();
//! let chatgpt_resource = chatgpt_adapter.transform(&resource);
//!
//! // Or create a multi-platform resource that works everywhere
//! let multi = MultiPlatformResource::new(resource)
//!     .with_adapter(ChatGptAdapter::new())
//!     .with_adapter(McpAppsAdapter::new());
//! ```
//!
//! # Feature Flag
//!
//! This module requires the `mcp-apps` feature:
//!
//! ```toml
//! [dependencies]
//! pmcp = { version = "1.9", features = ["mcp-apps"] }
//! ```

mod adapter;
mod builder;
mod widget_fs;

pub use adapter::{ChatGptAdapter, McpAppsAdapter, McpUiAdapter, UIAdapter};
pub use builder::{MultiPlatformResource, UIResourceBuilder};
pub use widget_fs::{WidgetDir, WidgetEntry};
