//! Standard I/O transport implementation.
//!
//! This transport uses stdin/stdout for communication with newline-delimited
//! JSON-RPC messages as per the MCP specification.

use crate::error::{Result, TransportError};
use crate::shared::transport::{Transport, TransportMessage};
use async_trait::async_trait;
#[cfg(not(target_arch = "wasm32"))]
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::Mutex;

/// stdio transport for MCP communication.
///
/// Uses newline-delimited JSON-RPC messages as per the MCP specification.
/// Messages are written to stdout and read from stdin.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::shared::StdioTransport;
///
/// # async fn example() -> pmcp::Result<()> {
/// let transport = StdioTransport::new();
/// // Use with Client or Server
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct StdioTransport {
    stdin: Mutex<BufReader<tokio::io::Stdin>>,
    /// Persistent partial-line buffer for cancel-safe `receive()`.
    ///
    /// `AsyncBufReadExt::read_line` is NOT cancellation-safe: its future
    /// accumulates bytes in an INTERNAL scratch `Vec` and only flushes them to
    /// the destination string on completion, so a dropped receive future loses
    /// everything it consumed — corrupting the next JSON-RPC line. We instead
    /// use `read_until(b'\n', ..)`, which appends directly into THIS persistent
    /// buffer (behind the same lock as `stdin`). A dropped read therefore
    /// retains its consumed bytes and the next call resumes appending until a
    /// full newline-delimited line is available.
    partial: Mutex<Vec<u8>>,
    stdout: Mutex<tokio::io::Stdout>,
    /// Set on stdin EOF, or by `close()`. Gates `receive()`.
    read_closed: std::sync::atomic::AtomicBool,
    /// Set ONLY by `close()` or a real stdout write error. Gates `send()`.
    ///
    /// These were one flag, and stdin EOF set it — so `send()` then refused
    /// every stdout write. A client that pipes a batch of requests and closes
    /// stdin, which is the normal shape of a one-shot MCP session and the only
    /// way this transport can signal end-of-input, never received the responses
    /// to requests the server had already accepted and answered (#316).
    ///
    /// stdin and stdout are separate pipes. "I have no further requests" is not
    /// "I have stopped reading your replies".
    write_closed: std::sync::atomic::AtomicBool,
}

impl StdioTransport {
    /// Create a new stdio transport.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::StdioTransport;
    ///
    /// let transport = StdioTransport::new();
    /// // Transport is ready to use
    /// ```
    pub fn new() -> Self {
        Self {
            stdin: Mutex::new(BufReader::new(tokio::io::stdin())),
            partial: Mutex::new(Vec::new()),
            stdout: Mutex::new(tokio::io::stdout()),
            read_closed: std::sync::atomic::AtomicBool::new(false),
            write_closed: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        contract_pre_transport_abstraction!();
        // `write_closed`, not the read side: stdin EOF must not stop us
        // answering what we already accepted (#316).
        if self.write_closed.load(std::sync::atomic::Ordering::Acquire) {
            return Err(TransportError::ConnectionClosed.into());
        }

        let json_bytes = Self::serialize_message(&message)?;
        self.write_message(&json_bytes).await
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        contract_pre_transport_abstraction!();
        if self.read_closed.load(std::sync::atomic::Ordering::Acquire) {
            return Err(TransportError::ConnectionClosed.into());
        }

        let buffer = self.read_line().await?;
        Self::parse_message(&buffer)
    }

    async fn close(&mut self) -> Result<()> {
        contract_pre_transport_abstraction!();
        // close() still stops BOTH directions — that guarantee is unchanged.
        self.read_closed
            .store(true, std::sync::atomic::Ordering::Release);
        self.write_closed
            .store(true, std::sync::atomic::Ordering::Release);

        // Flush any pending output
        let mut stdout = self.stdout.lock().await;
        stdout.flush().await.map_err(TransportError::from)?;
        drop(stdout);

        // Note: To send EOF to the server, the spawning process should drop
        // the child process handle or close the pipe. This is handled at the
        // process/spawn level, not here. The server will see EOF on its stdin
        // when the client process terminates or closes its end of the pipe.

        Ok(())
    }

    fn is_connected(&self) -> bool {
        // Still connected while we can answer. A server that has read EOF but
        // has responses to write is not disconnected.
        !self.write_closed.load(std::sync::atomic::Ordering::Acquire)
    }

    fn transport_type(&self) -> &'static str {
        "stdio"
    }
}

impl StdioTransport {
    /// Serialize transport message to JSON bytes.
    ///
    /// Delegates to [`crate::shared::transport::serialize_message`] — the single
    /// source of truth for the JSON-RPC wire encoding shared by all transports.
    pub fn serialize_message(message: &TransportMessage) -> Result<Vec<u8>> {
        crate::shared::transport::serialize_message(message)
    }

    /// Write message to stdout with newline delimiter.
    async fn write_message(&self, json_bytes: &[u8]) -> Result<()> {
        let mut stdout = self.stdout.lock().await;

        // A real stdout failure — a closed or broken pipe — DOES close the
        // write side. That is the other half of #316: `send()` no longer gives
        // up on stdin EOF, so the write side must still latch shut when stdout
        // itself is gone, or a dead pipe would be retried forever.
        let mut fail = |e: std::io::Error| {
            self.write_closed
                .store(true, std::sync::atomic::Ordering::Release);
            TransportError::from(e)
        };

        // Write message payload
        stdout.write_all(json_bytes).await.map_err(&mut fail)?;

        // Write newline delimiter (MCP spec requirement)
        stdout.write_all(b"\n").await.map_err(&mut fail)?;

        // Always flush stdio
        stdout.flush().await.map_err(&mut fail)?;
        drop(stdout);

        Ok(())
    }

    /// Read a line from stdin (newline-delimited JSON per MCP spec).
    ///
    /// Cancel-safe: reads into the persistent [`Self::partial`] buffer via
    /// [`Self::read_cancel_safe_line`], so a dropped receive future never loses
    /// consumed bytes.
    async fn read_line(&self) -> Result<Vec<u8>> {
        // Hold both guards for the whole read so the persistent buffer and the
        // buffered reader advance atomically. A future dropped while awaiting
        // more input releases these guards WITHOUT discarding `partial`.
        let mut stdin = self.stdin.lock().await;
        let mut partial = self.partial.lock().await;

        if let Some(bytes) = Self::read_cancel_safe_line(&mut stdin, &mut partial).await? {
            Ok(bytes)
        } else {
            // EOF on stdin closes the READ side only. stdout stays writable so
            // in-flight responses can still be delivered (#316).
            self.read_closed
                .store(true, std::sync::atomic::Ordering::Release);
            Err(TransportError::ConnectionClosed.into())
        }
    }

    /// Cancel-safe newline-delimited line reader over any async buffered reader.
    ///
    /// Appends into the caller-owned persistent `partial` buffer via
    /// `read_until(b'\n', ..)` (which writes directly into the buffer, unlike
    /// `read_line`), so a dropped future retains already-consumed bytes; the
    /// next call resumes appending until a complete `\n`-delimited line is
    /// available. Returns:
    /// - `Ok(Some(bytes))` — one complete, non-empty line (trailing `\r`/`\n`
    ///   stripped);
    /// - `Ok(None)` — EOF with nothing left buffered;
    /// - `Err(InvalidMessage)` — an empty line (skipped per the MCP spec).
    ///
    /// # EOF with an unterminated tail
    ///
    /// A peer that writes a complete JSON-RPC frame and closes WITHOUT a
    /// trailing newline leaves those bytes in `partial`. `read_line` — which
    /// this replaced — returned them as the final message, so discarding them
    /// here would silently drop the last frame of every such peer. The tail is
    /// therefore delivered exactly once, and the next call sees an empty buffer
    /// plus a `0`-byte read and reports EOF.
    ///
    /// Generic over the reader so the drop-mid-read cancel-safety property can
    /// be exercised in tests against an in-memory duplex pipe.
    async fn read_cancel_safe_line<R>(
        reader: &mut BufReader<R>,
        partial: &mut Vec<u8>,
    ) -> Result<Option<Vec<u8>>>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        loop {
            // A complete line may already be buffered from a prior (possibly
            // cancelled) call — extract it before reading more.
            if let Some(idx) = partial.iter().position(|&b| b == b'\n') {
                let mut line: Vec<u8> = partial.drain(..=idx).collect();
                // Strip the trailing '\n' and an optional '\r'.
                line.pop();
                return Self::finish_line(line);
            }

            // No complete line yet — append more bytes into the PERSISTENT
            // buffer. `read_until` writes straight into `partial`, so if this
            // await is cancelled the consumed bytes are retained.
            let bytes_read = reader
                .read_until(b'\n', partial)
                .await
                .map_err(TransportError::from)?;
            if bytes_read == 0 {
                // EOF. Anything still buffered is a final, newline-less frame:
                // deliver it rather than dropping it (see the doc note above).
                if partial.is_empty() {
                    return Ok(None);
                }
                return Self::finish_line(std::mem::take(partial));
            }
        }
    }

    /// Trim an optional trailing `\r` off an already-`\n`-stripped line and
    /// classify it — the ONE place both the newline-terminated and the
    /// EOF-terminated exits above agree on what a line is.
    fn finish_line(mut line: Vec<u8>) -> Result<Option<Vec<u8>>> {
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        if line.is_empty() {
            // Skip empty lines (per MCP spec: newline-delimited frames).
            return Err(TransportError::InvalidMessage("Empty line received".to_string()).into());
        }
        Ok(Some(line))
    }

    /// Parse JSON message and determine its type.
    ///
    /// Delegates to [`crate::shared::transport::parse_message`] — the single
    /// source of truth for JSON-RPC frame classification shared by all transports.
    pub fn parse_message(buffer: &[u8]) -> Result<TransportMessage> {
        crate::shared::transport::parse_message(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn transport_properties() {
        let transport = StdioTransport::new();
        assert!(transport.is_connected());
        assert_eq!(transport.transport_type(), "stdio");
    }

    #[tokio::test]
    async fn test_close() {
        let mut transport = StdioTransport::new();
        assert!(transport.is_connected());

        transport.close().await.unwrap();
        assert!(!transport.is_connected());
    }

    #[test]
    fn test_newline_delimited_format() {
        // Test that serialization produces valid JSON without Content-Length
        let request = TransportMessage::Request {
            id: crate::types::RequestId::Number(1),
            request: crate::types::Request::Client(Box::new(
                crate::types::ClientRequest::Initialize(crate::types::InitializeRequest {
                    protocol_version: "2025-06-18".to_string(),
                    capabilities: crate::types::ClientCapabilities::default(),
                    client_info: crate::types::Implementation::new("test", "1.0.0"),
                }),
            )),
        };

        let json_bytes = StdioTransport::serialize_message(&request).unwrap();
        let json_str = String::from_utf8(json_bytes).unwrap();

        // Should be valid JSON without Content-Length header
        assert!(json_str.starts_with('{'));
        assert!(json_str.contains("jsonrpc\":\"2.0\""));
        assert!(!json_str.contains("Content-Length"));
        assert!(!json_str.contains("\r\n"));
    }

    /// Cancel-safety proof (Phase 108): a `receive()` future dropped mid-read
    /// (when the transport actor's `select!` send branch wins) must lose NO
    /// already-consumed bytes — the next read returns the WHOLE first line.
    #[tokio::test]
    async fn read_cancel_safe_line_retains_partial_across_drop() {
        use tokio::io::{AsyncWriteExt, BufReader};

        let (mut writer, reader) = tokio::io::duplex(64);
        let mut reader = BufReader::new(reader);
        let mut partial: Vec<u8> = Vec::new();

        // Feed a partial line (no newline yet).
        writer.write_all(b"hello wo").await.unwrap();

        // Simulate the actor dropping the in-flight receive: race the read
        // against a timer that wins because no newline ever arrives.
        tokio::select! {
            _ = StdioTransport::read_cancel_safe_line(&mut reader, &mut partial) => {
                panic!("read must not complete without a newline");
            }
            () = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
        }

        // The consumed bytes were retained in the persistent buffer.
        assert_eq!(
            partial, b"hello wo",
            "a dropped read must retain the bytes it already consumed"
        );

        // Feed the rest plus a second, fully-buffered line. The next read
        // resumes and returns the WHOLE first line — no bytes lost.
        writer.write_all(b"rld\nsecond\n").await.unwrap();
        let line = StdioTransport::read_cancel_safe_line(&mut reader, &mut partial)
            .await
            .unwrap()
            .expect("a complete first line");
        assert_eq!(
            line, b"hello world",
            "no bytes lost across the dropped read"
        );

        // The pipelined second line is intact too.
        let line2 = StdioTransport::read_cancel_safe_line(&mut reader, &mut partial)
            .await
            .unwrap()
            .expect("a complete second line");
        assert_eq!(line2, b"second");
    }

    /// EOF surfaces as `Ok(None)` from the cancel-safe reader.
    #[tokio::test]
    async fn read_cancel_safe_line_reports_eof() {
        use tokio::io::BufReader;

        let (writer, reader) = tokio::io::duplex(8);
        drop(writer); // close the write half -> EOF on the read half
        let mut reader = BufReader::new(reader);
        let mut partial: Vec<u8> = Vec::new();

        let eof = StdioTransport::read_cancel_safe_line(&mut reader, &mut partial)
            .await
            .unwrap();
        assert!(eof.is_none(), "closed pipe must yield EOF (Ok(None))");
    }

    /// A peer that writes a complete frame and closes WITHOUT a trailing
    /// newline must still have that frame delivered — `read_line`, which the
    /// cancel-safe reader replaced, returned it, so dropping it would silently
    /// lose the last message of every such peer.
    #[tokio::test]
    async fn read_cancel_safe_line_delivers_an_unterminated_tail_at_eof() {
        use tokio::io::{AsyncWriteExt, BufReader};

        let (mut writer, reader) = tokio::io::duplex(64);
        let mut reader = BufReader::new(reader);
        let mut partial: Vec<u8> = Vec::new();

        writer.write_all(b"first\nlast-no-newline").await.unwrap();
        drop(writer); // EOF with an unterminated tail still buffered.

        let first = StdioTransport::read_cancel_safe_line(&mut reader, &mut partial)
            .await
            .unwrap()
            .expect("the terminated line");
        assert_eq!(first, b"first");

        let last = StdioTransport::read_cancel_safe_line(&mut reader, &mut partial)
            .await
            .unwrap()
            .expect("the unterminated tail is still a frame");
        assert_eq!(last, b"last-no-newline");

        // And it is delivered exactly once.
        let eof = StdioTransport::read_cancel_safe_line(&mut reader, &mut partial)
            .await
            .unwrap();
        assert!(eof.is_none(), "the tail must not be re-delivered");
    }
    // ================= #316: stdin EOF must not disable stdout =================

    use crate::shared::Transport;
    use std::sync::atomic::Ordering;

    /// REGRESSION (#316): one `closed` flag served two independent pipes.
    ///
    /// Reaching EOF on **stdin** set it, and `send()` then refused every
    /// **stdout** write — so a client that piped a batch of requests and closed
    /// stdin never received the responses to requests the server had already
    /// accepted and answered. That is the normal shape of a one-shot MCP
    /// session, and closing stdin is the only way this transport can signal
    /// end-of-input. Measured downstream: 40/40 sessions answered on pmcp 2.11,
    /// 10/40 on 2.17.
    ///
    /// This assertion is over the SOURCE of `read_line`, deliberately.
    /// `StdioTransport` reads the process's real stdin, so the EOF branch
    /// cannot be driven from a unit test — and the first version of this test
    /// set `read_closed` by hand instead, which meant it passed just as happily
    /// with the bug reinstated. A test that cannot fail on the defect it names
    /// is worse than none, so this reads the branch itself.
    #[test]
    fn stdin_eof_closes_the_read_side_only() {
        // Only the PRODUCTION half of the file. `include_str!` pulls in this
        // test module too, and the first version of this assertion matched its
        // own string literals — the same self-reference that makes a drift
        // guard silently compare the wrong thing.
        let src = include_str!("stdio.rs");
        let production = src
            .split("mod tests {")
            .next()
            .expect("the file has a test module");
        let body = production
            .split("// EOF on stdin closes the READ side only")
            .nth(1)
            .expect("the EOF branch must carry its marker comment");
        let branch = &body[..body.find("\n    }").unwrap_or_else(|| body.len().min(400))];
        assert!(
            branch.contains("read_closed"),
            "the EOF branch must close the read side: {branch}"
        );
        assert!(
            !branch.contains("write_closed"),
            "stdin EOF must NOT close the write side — the server still owes \
             responses to requests it already accepted (#316). Found:\n{branch}"
        );
    }

    /// The state really is two independent flags, and a fresh transport has
    /// both open.
    #[test]
    fn read_and_write_state_are_independent() {
        let t = StdioTransport::new();
        assert!(!t.read_closed.load(Ordering::Acquire));
        assert!(!t.write_closed.load(Ordering::Acquire));

        t.read_closed.store(true, Ordering::Release);
        assert!(
            !t.write_closed.load(Ordering::Acquire),
            "closing reads must leave writes open"
        );
        assert!(t.is_connected(), "still connected while it can answer");
    }

    /// `close()` must still stop BOTH directions — the guarantee that existed
    /// before the split, and the reason the split is not simply "drop the flag".
    #[tokio::test]
    async fn close_still_stops_both_directions() {
        let mut t = StdioTransport::new();
        t.close().await.expect("close");
        assert!(
            t.read_closed.load(Ordering::Acquire),
            "close() closes reads"
        );
        assert!(
            t.write_closed.load(Ordering::Acquire),
            "close() closes writes"
        );
        assert!(!t.is_connected());
    }

    /// And a `send()` after `close()` is still refused, so the split did not
    /// widen what the transport accepts.
    #[tokio::test]
    async fn send_after_close_is_still_refused() {
        let mut t = StdioTransport::new();
        t.close().await.expect("close");
        let sent = t
            .send(TransportMessage::Notification(
                crate::types::Notification::Progress(crate::types::ProgressNotification {
                    progress_token: crate::types::ProgressToken::String("t".into()),
                    progress: 1.0,
                    message: None,
                    total: None,
                }),
            ))
            .await;
        assert!(sent.is_err(), "send() after close() must fail");
    }
}
