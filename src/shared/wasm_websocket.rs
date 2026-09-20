//! WebSocket transport implementation for WASM environments.
//!
//! This module provides a WebSocket transport that works in browser environments
//! using the Web API WebSocket interface.

#![cfg(target_arch = "wasm32")]

use crate::error::{Error, Result, TransportError};
use crate::shared::transport::{Transport, TransportMessage};
use async_trait::async_trait;
use futures::channel::mpsc;
use futures::StreamExt;
use serde_json::Value;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

/// WebSocket transport for WASM environments.
///
/// # Examples
///
/// ```rust,ignore
/// use pmcp::shared::WasmWebSocketTransport;
///
/// # async fn example() -> pmcp::Result<()> {
/// let transport = WasmWebSocketTransport::connect("ws://localhost:8080").await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct WasmWebSocketTransport {
    ws: WebSocket,
    rx: mpsc::UnboundedReceiver<TransportMessage>,
    // Held to keep the channel open for the `on_message` closure below, which
    // clones it; never read through this field directly.
    #[allow(dead_code)]
    tx: mpsc::UnboundedSender<TransportMessage>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(ErrorEvent)>,
    _on_close: Closure<dyn FnMut()>,
}

impl WasmWebSocketTransport {
    /// Connect to a WebSocket server.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use pmcp::shared::WasmWebSocketTransport;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// // Connect to local server
    /// let transport = WasmWebSocketTransport::connect("ws://localhost:8080").await?;
    ///
    /// // Connect to secure server
    /// let secure_transport = WasmWebSocketTransport::connect("wss://example.com/mcp").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(url: &str) -> Result<Self> {
        let ws = WebSocket::new(url).map_err(|e| {
            Error::Transport(TransportError::InvalidMessage(format!(
                "Failed to create WebSocket: {:?}",
                e
            )))
        })?;

        // Set binary type to arraybuffer for efficiency
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        // Create channel for receiving messages
        let (tx, rx) = mpsc::unbounded();
        let tx_clone = tx.clone();

        // Setup message handler
        let on_message = {
            let tx = tx.clone();
            Closure::wrap(Box::new(move |e: MessageEvent| {
                if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                    let text: String = text.into();
                    if let Ok(msg) = serde_json::from_str::<Value>(&text) {
                        // Parse the message into TransportMessage
                        if let Ok(transport_msg) = parse_transport_message(msg) {
                            let _ = tx.unbounded_send(transport_msg);
                        }
                    }
                }
            }) as Box<dyn FnMut(MessageEvent)>)
        };

        // Setup error handler
        let on_error = Closure::wrap(Box::new(move |e: ErrorEvent| {
            web_sys::console::error_1(&format!("WebSocket error: {:?}", e).into());
        }) as Box<dyn FnMut(ErrorEvent)>);

        // Setup close handler
        let on_close = {
            let _tx = tx_clone;
            Closure::wrap(Box::new(move || {
                web_sys::console::log_1(&"WebSocket closed".into());
                // Could send a close notification through the channel
            }) as Box<dyn FnMut()>)
        };

        // Register event handlers
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));

        // Wait for connection to open
        wait_for_open(&ws).await?;

        Ok(Self {
            ws,
            rx,
            tx,
            _on_message: on_message,
            _on_error: on_error,
            _on_close: on_close,
        })
    }

    /// Check if the WebSocket is connected.
    pub fn is_connected(&self) -> bool {
        self.ws.ready_state() == WebSocket::OPEN
    }
}

#[async_trait(?Send)]
impl Transport for WasmWebSocketTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        if !self.is_connected() {
            return Err(Error::Transport(TransportError::ConnectionClosed));
        }

        let json = serialize_transport_message(message)?;
        let text = serde_json::to_string(&json)?;

        self.ws.send_with_str(&text).map_err(|e| {
            Error::Transport(TransportError::Send(format!(
                "Failed to send message: {:?}",
                e
            )))
        })?;

        Ok(())
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        self.rx
            .next()
            .await
            .ok_or_else(|| Error::Transport(TransportError::ConnectionClosed))
    }

    async fn close(&mut self) -> Result<()> {
        self.ws.close().map_err(|e| {
            Error::Transport(TransportError::Io(format!(
                "Failed to close WebSocket: {:?}",
                e
            )))
        })?;
        Ok(())
    }
}

/// Wait for the WebSocket to open
async fn wait_for_open(ws: &WebSocket) -> Result<()> {
    let mut attempts = 0;
    const MAX_ATTEMPTS: u32 = 100; // 10 seconds with 100ms intervals

    while ws.ready_state() == WebSocket::CONNECTING {
        if attempts >= MAX_ATTEMPTS {
            return Err(Error::Transport(TransportError::Request(
                "WebSocket connection timeout".to_string(),
            )));
        }

        // Sleep for 100ms
        crate::shared::runtime::sleep(std::time::Duration::from_millis(100)).await;
        attempts += 1;
    }

    if ws.ready_state() != WebSocket::OPEN {
        return Err(Error::Transport(TransportError::Request(
            "WebSocket failed to connect".to_string(),
        )));
    }

    Ok(())
}

/// Parse a JSON value into a TransportMessage
fn parse_transport_message(value: Value) -> Result<TransportMessage> {
    // Check if it's a request
    if value.get("method").is_some() && value.get("id").is_some() {
        let id = serde_json::from_value(value["id"].clone())?;
        let request = serde_json::from_value(value)?;
        Ok(TransportMessage::Request { id, request })
    }
    // Check if it's a response
    else if value.get("result").is_some() || value.get("error").is_some() {
        let response = serde_json::from_value(value)?;
        Ok(TransportMessage::Response(response))
    }
    // Otherwise it's a notification
    else if value.get("method").is_some() {
        let notification = serde_json::from_value(value)?;
        Ok(TransportMessage::Notification(notification))
    } else {
        Err(Error::parse("Unknown message type"))
    }
}

/// Serialize a TransportMessage to JSON
fn serialize_transport_message(message: TransportMessage) -> Result<Value> {
    match message {
        TransportMessage::Request { id, request } => {
            let mut value = serde_json::to_value(request)?;
            value["id"] = serde_json::to_value(id)?;
            Ok(value)
        },
        TransportMessage::Response(response) => Ok(serde_json::to_value(response)?),
        TransportMessage::Notification(notification) => Ok(serde_json::to_value(notification)?),
    }
}

/// Configuration for WASM WebSocket connections
#[derive(Debug, Clone)]
pub struct WasmWebSocketConfig {
    /// URL to connect to
    pub url: String,
    /// Reconnect on disconnect
    pub auto_reconnect: bool,
    /// Maximum reconnection attempts
    pub max_reconnect_attempts: u32,
    /// Reconnection delay in milliseconds
    pub reconnect_delay_ms: u64,
}

impl Default for WasmWebSocketConfig {
    fn default() -> Self {
        Self {
            url: "ws://localhost:8080".to_string(),
            auto_reconnect: true,
            max_reconnect_attempts: 5,
            reconnect_delay_ms: 1000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_config_default() {
        let config = WasmWebSocketConfig::default();
        assert_eq!(config.url, "ws://localhost:8080");
        assert!(config.auto_reconnect);
        assert_eq!(config.max_reconnect_attempts, 5);
        assert_eq!(config.reconnect_delay_ms, 1000);
    }
}
