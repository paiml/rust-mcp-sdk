//! MCP Preview Server
//!
//! A browser-based development environment for testing MCP Apps widgets.
//! Simulates the ChatGPT Apps runtime environment with full bridge support.
//!
//! # Usage
//!
//! ```bash
//! cargo pmcp preview --url http://localhost:3000 --open
//! ```
//!
//! # Features
//!
//! - Widget rendering in isolated iframe
//! - Full `window.mcpBridge` / `window.openai` simulation
//! - Environment controls (theme, locale, display mode)
//! - DevTools panel (state, console, network, events)
//! - Live proxy to MCP server via HTTP

mod assets;
mod handlers;
mod proxy;
mod server;
pub mod wasm_builder;

pub use server::{PreviewConfig, PreviewMode, PreviewServer};
