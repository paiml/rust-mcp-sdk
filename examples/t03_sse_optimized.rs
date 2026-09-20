// Why: this example demonstrates `OptimizedSseTransport`, which is deprecated
// on purpose (plan 113.1-03, D-01) but still ships in 2.x. A shipped example
// for a shipped transport is not wrong, and this is its only compile-check
// consumer. Rewriting it onto `StreamableHttpTransport` is a Phase 119 docs
// item, deliberately not done here.
#![allow(deprecated)]

//! Optimized SSE Transport Example
//!
//! PMCP-4002: Demonstrates optimized SSE transport with advanced features
//!
//! **DEPRECATED** — `OptimizedSseTransport` is retained for 2.x compatibility
//! only. For new code use `StreamableHttpTransport`, which bounds every
//! peer-controlled read and carries a configurable cap.
//!
//! Run with: cargo run --example t03_sse_optimized --features sse

use pmcp::shared::{OptimizedSseConfig, OptimizedSseTransport, Transport, TransportMessage};
use std::time::Duration;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("🚀 Starting Optimized SSE Transport Example");

    // Configure optimized SSE transport
    let config = OptimizedSseConfig {
        url: "http://localhost:8080/sse".to_string(),
        connection_timeout: Duration::from_secs(30),
        keepalive_interval: Duration::from_secs(15),
        max_reconnects: 5,
        reconnect_delay: Duration::from_secs(1),
        buffer_size: 100,
        flush_interval: Duration::from_millis(100),
        enable_pooling: true,
        max_connections: 10,
        enable_compression: false,
    };

    info!("✅ Configuration:");
    info!("  • URL: {}", config.url);
    info!("  • Connection pooling: {}", config.enable_pooling);
    info!("  • Max connections: {}", config.max_connections);
    info!("  • Buffer size: {}", config.buffer_size);
    info!("  • Flush interval: {:?}", config.flush_interval);
    info!("  • Keepalive: {:?}", config.keepalive_interval);
    info!("  • Compression: {}", config.enable_compression);

    // Create transport
    let mut transport = OptimizedSseTransport::new(config);

    info!(
        "📊 Transport created with type: {}",
        transport.transport_type()
    );

    // Demonstrate sending messages
    info!("📤 Sending test messages...");

    // Send a notification
    let notification = TransportMessage::Notification(pmcp::types::Notification::Progress(
        pmcp::types::ProgressNotification::new(
            pmcp::types::ProgressToken::String("task-001".to_string()),
            25.0,
            Some("Processing started".to_string()),
        ),
    ));

    if let Err(e) = transport.send(notification).await {
        info!("Failed to send notification: {}", e);
    } else {
        info!("✓ Notification sent");
    }

    // Send a request
    let request = TransportMessage::Request {
        id: pmcp::types::RequestId::from(1i64),
        request: pmcp::types::Request::Client(Box::new(pmcp::types::ClientRequest::Ping)),
    };

    if let Err(e) = transport.send(request).await {
        info!("Failed to send request: {}", e);
    } else {
        info!("✓ Request sent");
    }

    // Demonstrate batch sending (messages will be coalesced)
    info!("📦 Sending batch of messages...");

    for i in 0..10 {
        let progress_msg = TransportMessage::Notification(pmcp::types::Notification::Progress(
            pmcp::types::ProgressNotification::new(
                pmcp::types::ProgressToken::String(format!("batch-{}", i)),
                i as f64 * 10.0,
                Some(format!("Batch message {}", i)),
            ),
        ));

        if let Err(e) = transport.send(progress_msg).await {
            info!("Failed to send batch message {}: {}", i, e);
        }
    }

    info!("✓ Batch messages queued (will be coalesced and flushed)");

    // Check connection status
    info!(
        "🔌 Connection status: {}",
        if transport.is_connected() {
            "Connected"
        } else {
            "Disconnected"
        }
    );

    // Simulate receiving (would normally come from server)
    info!("📥 Attempting to receive messages...");

    match tokio::time::timeout(Duration::from_secs(2), transport.receive()).await {
        Ok(Ok(msg)) => {
            info!("Received message: {:?}", msg);
        },
        Ok(Err(e)) => {
            info!("Receive error: {}", e);
        },
        Err(_) => {
            info!("No messages received (timeout)");
        },
    }

    // Demonstrate connection pooling benefit
    info!("🔄 Connection pooling benefits:");
    info!("  • Reuses existing connections");
    info!("  • Reduces latency for subsequent requests");
    info!("  • Maintains TCP keepalive");
    info!("  • Automatic reconnection on failure");

    // Close transport
    transport.close().await?;
    info!("👋 Transport closed");

    Ok(())
}
