//! Server-side logging from a tool handler: `extra.log(..)` and the level that
//! decides what a client actually sees (Phase 118.2, CONF-10).
//!
//! Run with: cargo run --example s55_handler_logging
//!
//! It finishes in well under a second: the sink is a local capture, so nothing
//! here opens a port, spawns a transport or touches the network. Contrast
//! `examples/s54_v2_dual_conformance.rs`, which drives the same `extra.log(..)`
//! surface over a real streamable-HTTP server for the official conformance
//! suite — same API, opposite end of the wiring.
//!
//! # What this shows
//!
//! A handler emits diagnostics through
//! [`RequestHandlerExtra::log`](pmcp::RequestHandlerExtra::log) and
//! [`log_with_data`](pmcp::RequestHandlerExtra::log_with_data). Every record it
//! emits at or above the request's effective level is handed to that request's
//! notification sink and reaches the client as a `notifications/message`
//! notification; everything below it is dropped at the emitter and never leaves
//! the process.
//!
//! In a real server you do **not** wire the sink or the level yourself — the
//! transport does both, per request:
//!
//! | Era | How the client asks for a level |
//! |-----|--------------------------------|
//! | 2025-11-25 (v1) | the `logging/setLevel` RPC, remembered for that SESSION |
//! | 2026-07-28 (v2) | `params._meta["io.modelcontextprotocol/logLevel"]`, for THAT REQUEST only (the RPC is retired) |
//!
//! When the client asked for neither, `DEFAULT_LOG_LEVEL` — `info` — applies.
//!
//! This example stands the sink up by hand so the whole story fits in one
//! process with no ports and no client: the sink here prints the exact JSON a
//! connected client would receive on the wire.
//!
//! # Three things worth knowing
//!
//! * **Levels order by SEVERITY, not alphabetically.** `error` is more severe
//!   than `debug`, which is what `LoggingLevel`'s `Ord` encodes — a plain string
//!   comparison would put `critical` below `debug` and invert every filter.
//! * **`Ok(())` is not delivery acknowledgement.** It means the record was handed
//!   to whatever sink this request has, or that there was none. A handler with no
//!   sink attached — `RequestHandlerExtra::default()`, as used in unit tests —
//!   logs successfully and emits nothing, which is what keeps a logging handler
//!   callable outside a server.
//! * **Emitting is synchronous.** No `.await`, so a handler can log from anywhere
//!   in its body without restructuring.
//! * **Every frame carries `data`, and a plain `log(..)` repeats the message
//!   there.** That is why the printed JSON below shows
//!   `"message":"query dispatched","data":"query dispatched"` — not a bug. The
//!   MCP schema declares `data` REQUIRED with no `message` member at all, and the
//!   official reference client's `z.unknown()` is non-optional under zod v4, so a
//!   frame without `data` is dropped on the floor. pmcp therefore defaults `data`
//!   to the message and keeps `message` alongside as an extension. A
//!   `log_with_data(..)` value is passed through verbatim and never overwritten
//!   — the `slow query` record below shows both halves at once.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::{Arc, Mutex};

use pmcp::types::{LoggingLevel, Notification};
use pmcp::RequestHandlerExtra;
use serde_json::json;

/// The per-request notification sink's exact type, spelled once.
///
/// This is the signature `RequestHandlerExtra::with_log_sink` takes, so it is
/// worth naming: it is **synchronous** and it returns `()`. A sink that cannot
/// report failure is why `log(..)`'s `Ok(())` is not a delivery acknowledgement.
type LogSink = Arc<dyn Fn(Notification) + Send + Sync>;

/// Stand in for the transport's per-request notification sink.
///
/// A real server never writes this: `Server` and `ServerCore` both attach the
/// request's sink at dispatch, from the transport's back-channel. Printing the
/// serialized notification is the point — this is byte-for-byte what a connected
/// client receives.
fn printing_sink() -> (LogSink, Arc<Mutex<usize>>) {
    let seen = Arc::new(Mutex::new(0usize));
    let counter = Arc::clone(&seen);
    let sink: LogSink = Arc::new(move |notification| {
        *counter
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        let wire = serde_json::to_string(&notification).expect("a notification serializes");
        println!("    -> client receives: {wire}");
    });
    (sink, seen)
}

/// The body of a tool handler. In a real server this is
/// `ToolHandler::handle(&self, args, extra)`.
fn run_the_tool(extra: &RequestHandlerExtra) -> pmcp::Result<()> {
    extra.log(LoggingLevel::Debug, "resolving the connection pool")?;
    extra.log(LoggingLevel::Info, "query dispatched")?;
    extra.log_with_data(
        LoggingLevel::Warning,
        "slow query",
        json!({ "elapsedMs": 1_840, "table": "orders" }),
    )?;
    Ok(())
}

fn main() -> pmcp::Result<()> {
    // ---------------------------------------------------------------------
    // 1. The default. The client asked for no level, so `info` applies.
    // ---------------------------------------------------------------------
    println!("\n[1] no level requested — the `info` default applies");
    let (sink, seen) = printing_sink();
    run_the_tool(&RequestHandlerExtra::default().with_log_sink(sink))?;
    println!(
        "    {} of 3 records left the process (the `debug` one was dropped at the emitter)",
        *seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    );

    // ---------------------------------------------------------------------
    // 2. The client asked for `debug`. Everything is delivered.
    // ---------------------------------------------------------------------
    println!("\n[2] the client asked for `debug` — nothing is filtered");
    let (sink, seen) = printing_sink();
    run_the_tool(
        &RequestHandlerExtra::default()
            .with_log_sink(sink)
            .with_log_level(LoggingLevel::Debug),
    )?;
    println!(
        "    {} of 3 records left the process",
        *seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    );

    // ---------------------------------------------------------------------
    // 3. The client asked for `error`. Only more-severe records survive —
    //    and `warning` is LESS severe than `error`, so nothing does.
    // ---------------------------------------------------------------------
    println!("\n[3] the client asked for `error` — severity, not alphabetical order");
    let (sink, seen) = printing_sink();
    run_the_tool(
        &RequestHandlerExtra::default()
            .with_log_sink(sink)
            .with_log_level(LoggingLevel::Error),
    )?;
    println!(
        "    {} of 3 records left the process",
        *seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    );

    // ---------------------------------------------------------------------
    // 4. No sink at all — the shape a unit test sees. Logging still succeeds.
    // ---------------------------------------------------------------------
    //
    //    This is the D-08 contract, and it is deliberate rather than an
    //    oversight: with no sink attached the same three calls return `Ok(())`
    //    and emit nothing at all — no panic, no error, no warning. The cost is
    //    that a MISPLUMBED transport looks exactly like a quiet handler; the
    //    benefit is that a logging handler stays unit-testable outside a
    //    server, which is why every `#[test]` in this repo can call one.
    println!("\n[4] no sink attached — `log(..)` is Ok(()) and emits nothing");
    run_the_tool(&RequestHandlerExtra::default())?;
    println!("    the handler is still callable outside a server (D-08)");

    println!();
    Ok(())
}
