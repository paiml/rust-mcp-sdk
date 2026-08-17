//! Server-Sent Events (SSE) parser for MCP HTTP transport.
//!
//! This module provides a robust SSE parser compatible with the
//! `EventSource` specification, similar to eventsource-parser in TypeScript.

use std::collections::HashMap;
use std::fmt;

/// SSE event parsed from the stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// Event ID for resumption
    pub id: Option<String>,
    /// Event type/name
    pub event: Option<String>,
    /// Event data
    pub data: String,
    /// Retry interval in milliseconds
    pub retry: Option<u64>,
}

impl SseEvent {
    /// Create a new SSE event with data.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseEvent;
    ///
    /// let event = SseEvent::new("Hello, world!");
    /// assert_eq!(event.data, "Hello, world!");
    /// assert!(event.id.is_none());
    /// assert!(event.event.is_none());
    /// ```
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            id: None,
            event: None,
            data: data.into(),
            retry: None,
        }
    }

    /// Set the event ID.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseEvent;
    ///
    /// let event = SseEvent::new("data")
    ///     .with_id("msg-123");
    /// assert_eq!(event.id, Some("msg-123".to_string()));
    /// ```
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the event type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseEvent;
    ///
    /// let event = SseEvent::new("data")
    ///     .with_event("custom");
    /// assert_eq!(event.event, Some("custom".to_string()));
    /// ```
    pub fn with_event(mut self, event: impl Into<String>) -> Self {
        self.event = Some(event.into());
        self
    }

    /// Set the retry interval.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseEvent;
    ///
    /// let event = SseEvent::new("data")
    ///     .with_retry(3000);
    /// assert_eq!(event.retry, Some(3000));
    /// ```
    pub fn with_retry(mut self, retry: u64) -> Self {
        self.retry = Some(retry);
        self
    }
}

impl fmt::Display for SseEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(id) = &self.id {
            writeln!(f, "id: {}", id)?;
        }
        if let Some(event) = &self.event {
            writeln!(f, "event: {}", event)?;
        }
        if let Some(retry) = self.retry {
            writeln!(f, "retry: {}", retry)?;
        }

        // Split data by newlines and write each line
        for line in self.data.lines() {
            writeln!(f, "data: {}", line)?;
        }

        writeln!(f)?; // Empty line to end event
        Ok(())
    }
}

/// SSE parser state machine.
#[derive(Debug)]
pub struct SseParser {
    buffer: String,
    current_event: EventBuilder,
    last_event_id: Option<String>,
    /// Upper bound on the parser's total IN-FLIGHT bytes: the unterminated line
    /// still sitting in `buffer` PLUS the `data:` lines already accumulated into
    /// `current_event`, which only a BLANK line dispatches.
    ///
    /// The bytes fed to a parser are chosen by a REMOTE peer, and NEITHER
    /// accumulator drains on its own: `buffer` only drains as far as a `\n`, and
    /// `current_event.data` only drains when the peer sends the blank line that
    /// ends the event. Bounding `buffer` alone is therefore not a bound at all —
    /// a peer that streams ordinary newline-terminated `data:` lines forever
    /// grows this process's heap for as long as it holds the stream open, which
    /// is the realistic shape of the attack rather than the artificial one
    /// (verification gap item 3, review CR-01/CR-02, T-113-79).
    max_buffer_size: usize,
    /// Latched once oversized in-flight data has been discarded.
    overflowed: bool,
}

/// The default bound on a parser's total in-flight bytes (1 MiB).
///
/// Single source of truth for [`SseParser::new`] and [`SseConfig::default()`],
/// so the two can never disagree. Reading it through `SseConfig::default()`
/// would allocate that struct's `HashMap` and four `String`s just to fetch one
/// `usize`, on a path taken once per SSE response.
pub const DEFAULT_MAX_BUFFER_SIZE: usize = 1024 * 1024;

/// Split the longest decodable UTF-8 prefix off `buffer`, leaving the rest.
///
/// The companion every INCREMENTAL feeder of [`SseParser`] needs, and the reason
/// it lives beside the parser rather than beside one of them: a body chunk
/// boundary can fall in the MIDDLE of a multi-byte character, so a per-chunk
/// `String::from_utf8_lossy` corrupts any non-ASCII payload (a `file:///café.txt`
/// resource URI, a non-Latin tool argument) that happens to straddle two frames.
/// An INCOMPLETE tail is retained for the next chunk; genuinely INVALID bytes are
/// replaced with U+FFFD immediately, because retaining those forever would wedge
/// the stream on hostile input (T-113-67).
///
/// The two cases are handled INDEPENDENTLY rather than "any invalid byte means
/// decode the whole buffer lossily": a chunk that carries both an invalid byte
/// AND a trailing incomplete character would otherwise have that trailing
/// character replaced too, corrupting a legitimate multi-byte character that the
/// next chunk was about to complete.
///
/// The retained tail is at most 3 bytes, so this cannot grow without bound.
///
/// # Linear, not quadratic
///
/// The scan advances a `consumed` CURSOR and performs at most ONE `Vec::drain`
/// (or `clear`) for the whole call. The previous shape re-validated the buffer
/// from index 0 and `drain`-ed one invalid run per iteration, so a chunk of `n`
/// invalid bytes cost `O(n^2)` byte moves AND `O(n^2)` validation — a remote
/// CPU-exhaustion vector on a decoder that is fed directly from an untrusted
/// peer by both incremental feeders (`connect_sse`'s reader task and the
/// `subscriptions/listen` client), and one that no byte-count bound limits
/// because it runs BEFORE the parser sees the text (review CR-02).
pub(crate) fn take_utf8_prefix(buffer: &mut Vec<u8>) -> String {
    let mut text = String::new();
    // Bytes already appended to `text` (as themselves or as U+FFFD). Never
    // removed from `buffer` until the single exit-point drain below, so the
    // remaining slice is only ever validated once end-to-end.
    let mut consumed = 0usize;
    loop {
        let rest = &buffer[consumed..];
        let error = match std::str::from_utf8(rest) {
            Ok(valid) => {
                text.push_str(valid);
                buffer.clear();
                return text;
            },
            Err(error) => error,
        };
        let valid_up_to = error.valid_up_to();
        // `Utf8Error::valid_up_to()` is DEFINED as the length of the verified-valid
        // prefix, so this re-validation cannot fail; the `else` arm was unreachable.
        // The second pass itself is unavoidable under `#![deny(unsafe_code)]`.
        text.push_str(std::str::from_utf8(&rest[..valid_up_to]).unwrap_or_default());
        let Some(invalid_len) = error.error_len() else {
            // "Unexpected end of input": an incomplete character the next chunk
            // will finish. Keep exactly those bytes and yield what decoded.
            buffer.drain(..consumed + valid_up_to);
            return text;
        };
        // Never completable — emit the replacement character and skip past it.
        text.push('\u{FFFD}');
        consumed += valid_up_to + invalid_len;
    }
}

impl SseParser {
    /// Create a new SSE parser bounded by [`DEFAULT_MAX_BUFFER_SIZE`] (1 MiB),
    /// the same value [`SseConfig::default()`] carries.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseParser;
    ///
    /// let mut parser = SseParser::new();
    /// assert!(parser.last_event_id().is_none());
    /// assert!(!parser.overflowed());
    /// ```
    pub fn new() -> Self {
        Self::with_max_buffer_size(DEFAULT_MAX_BUFFER_SIZE)
    }

    /// Create a new SSE parser with an explicit in-flight bound.
    ///
    /// [`SseParser::new`] takes its bound from [`SseConfig::default()`]'s
    /// `max_buffer_size`. Use this constructor when a caller needs a TIGHTER
    /// one — a long-lived stream of small frames read from an untrusted remote
    /// peer, for example — or a looser one.
    ///
    /// The bound applies to everything the parser RETAINS across calls: the
    /// unterminated line in its buffer plus the `data:` lines already
    /// accumulated into the event still awaiting its blank line. A chunk that
    /// would push that total past the bound is DISCARDED, not
    /// truncated-and-emitted, and [`Self::overflowed`] latches: a silently
    /// truncated line would surface later as a misleading JSON parse failure,
    /// which is strictly worse for an operator than a named one.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseParser;
    ///
    /// let mut parser = SseParser::with_max_buffer_size(64);
    ///
    /// // A peer that never sends a newline cannot grow this parser's heap.
    /// assert!(parser.feed(&"x".repeat(1024)).is_empty());
    /// assert!(parser.overflowed());
    ///
    /// // The parser keeps working — but the flag stays latched.
    /// let events = parser.feed("data: ok\n\n");
    /// assert_eq!(events[0].data, "ok");
    /// assert!(parser.overflowed());
    /// ```
    ///
    /// Neither does a peer that sends perfectly ordinary NEWLINE-TERMINATED
    /// `data:` lines and simply never ends the event with a blank line — the
    /// realistic shape of the same attack, and the one a bound over the line
    /// buffer alone does not stop:
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseParser;
    ///
    /// let mut parser = SseParser::with_max_buffer_size(64);
    /// for _ in 0..1_000 {
    ///     // Every chunk is well-formed and ends in a newline.
    ///     assert!(parser.feed("data: AAAAAAAA\n").is_empty());
    /// }
    /// assert!(parser.overflowed());
    /// ```
    #[must_use]
    pub fn with_max_buffer_size(max_buffer_size: usize) -> Self {
        Self {
            buffer: String::new(),
            current_event: EventBuilder::new(),
            last_event_id: None,
            max_buffer_size,
            overflowed: false,
        }
    }

    /// Whether this parser has DISCARDED oversized IN-FLIGHT data.
    ///
    /// LATCHING: once set it stays set for the parser's lifetime — including
    /// across [`Self::reset`] — so a caller that polls once per chunk cannot
    /// miss the event. An overflowed parser has lost bytes a remote peer sent,
    /// so its stream should be considered CORRUPT: the recommended response is
    /// to end the stream with an error naming the limit, not to keep parsing.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseParser;
    ///
    /// let mut parser = SseParser::new();
    /// let _ = parser.feed("data: a well-formed event\n\n");
    /// assert!(!parser.overflowed());
    /// ```
    #[must_use]
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// The bound THIS parser was built with, in bytes.
    ///
    /// A caller reporting an overflow should name this rather than re-deriving
    /// a bound from config: parsers on different paths are deliberately built
    /// with different bounds, so a re-derived number can name a limit the
    /// parser never had.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseParser;
    ///
    /// assert_eq!(SseParser::with_max_buffer_size(64).max_buffer_size(), 64);
    /// ```
    #[must_use]
    pub fn max_buffer_size(&self) -> usize {
        self.max_buffer_size
    }

    /// The bytes this parser retains ACROSS lines, i.e. the two accumulators
    /// [`Self::max_buffer_size`] bounds: the unterminated line in the buffer plus
    /// the `data:` payload of the event still awaiting its blank line.
    ///
    /// NOT "total retained bytes", and deliberately not named that. It EXCLUDES
    /// `current_event.id`, `current_event.event` and `last_event_id`, and an
    /// `id:` value is in fact retained TWICE (once in each of the first and last
    /// of those). Those three fields are each ASSIGNED from a single line rather
    /// than APPENDED to, and a line can only reach `process_line` if the chunk
    /// carrying it satisfied the bound, so each is itself bounded by roughly
    /// `max_buffer_size` and none of them can grow with stream age. The true
    /// ceiling on a parser is therefore a small constant multiple of
    /// `max_buffer_size` rather than exactly it.
    ///
    /// What this function measures is the only pair of quantities that
    /// ACCUMULATE across lines — which is the property that matters for a
    /// long-lived stream, and the one the pre-113-17 bound violated.
    ///
    /// `pub(crate)` deliberately: its consumers are this module's tests,
    /// [`crate::client`]'s `subscriptions` module and (through that module) the
    /// fuzz seam. It is diagnostic detail, not shipped public API.
    pub(crate) fn buffered_bytes(&self) -> usize {
        self.buffer.len() + self.current_event.data.len()
    }

    /// Feed data to the parser and get parsed events.
    ///
    /// The parser retains at most [`Self::max_buffer_size`] bytes across its two
    /// accumulators on return; see `Self::buffered_bytes`. A chunk that would
    /// break that is refused whole and [`Self::overflowed`] latches.
    ///
    /// This is the ONLY entry point. The unbounded complete-body sibling that
    /// used to sit beside it was retired in Phase 118.2 once both of its callers
    /// became incremental readers — see the note where it lived.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseParser;
    ///
    /// let mut parser = SseParser::new();
    ///
    /// // Simple event
    /// let events = parser.feed("data: Hello\n\n");
    /// assert_eq!(events.len(), 1);
    /// assert_eq!(events[0].data, "Hello");
    ///
    /// // Event with ID
    /// let events = parser.feed("id: 123\ndata: World\n\n");
    /// assert_eq!(events[0].id, Some("123".to_string()));
    /// assert_eq!(events[0].data, "World");
    ///
    /// // Multi-line data
    /// let events = parser.feed("data: Line 1\ndata: Line 2\n\n");
    /// assert_eq!(events[0].data, "Line 1\nLine 2");
    ///
    /// // Custom event type
    /// let events = parser.feed("event: ping\ndata: pong\n\n");
    /// assert_eq!(events[0].event, Some("ping".to_string()));
    /// ```
    pub fn feed(&mut self, data: &str) -> Vec<SseEvent> {
        // A REMOTE peer decides every byte here, and BOTH of the parser's
        // accumulators are peer-drained: `buffer` empties only as far as a `\n`,
        // and `current_event.data` empties only when the peer sends the blank
        // line that dispatches the event. Bound their SUM, unconditionally: when
        // appending `data` would push the retained total past `max_buffer_size`,
        // DISCARD what is held and latch `overflowed` rather than grow.
        //
        // There is deliberately no "unless this chunk carries a newline" escape.
        // That escape used to be here, on the theory that a chunk containing a
        // newline completes a line and therefore cannot accumulate — but a
        // `data:` line accumulates into the EVENT, which no newline ends. Only a
        // BLANK line does. `feed("data: AAAAAAAA\n")` repeated 100 000 times
        // under a 64-byte bound reached 899 999 retained bytes with the flag
        // clear (review CR-01/CR-02, verification gap item 3).
        //
        // The bound is over RETAINED STATE PLUS THIS CHUNK, not over "one
        // in-progress event". A chunk carrying many individually small COMPLETE
        // events is refused whenever the chunk TOTAL exceeds the limit, so
        // behaviour depends partly on how the transport frames its reads. That
        // is accepted rather than fixed: evaluating the bound only over what
        // survives after splitting the complete events out of the chunk would
        // require parsing the whole unbounded chunk first — performing exactly
        // the allocation the bound exists to prevent (review MEDIUM-1,
        // T-113-86). A caller holding a COMPLETE body raises the BOUND for its
        // parser rather than reaching for an unbounded entry point; there is no
        // longer one to reach for.
        if self.buffered_bytes().saturating_add(data.len()) > self.max_buffer_size {
            self.overflowed = true;
            self.buffer.clear();
            // The in-progress event is now missing a line; anything built from
            // it would be a corrupted frame presented to the caller as genuine.
            self.current_event = EventBuilder::new();
            // `last_event_id` is deliberately untouched: it is stream-level
            // resumption state, not line state.
            return Vec::new();
        }

        let events = self.drain_complete_lines(data);

        // A SECOND, independently sufficient enforcement point over the RESIDUAL
        // this call actually leaves behind. Before the pre-check became
        // unconditional this caught the chunk that begins with `\n` and then
        // carries megabytes of newline-free bytes; it is kept because the two
        // checks fail independently, and a future change that weakens one must
        // still meet the other. It covers the event accumulator as well as the
        // line buffer, so the invariant that holds on RETURN from `feed` is
        // `buffered_bytes() <= max_buffer_size`.
        if self.buffered_bytes() > self.max_buffer_size {
            self.overflowed = true;
            self.buffer.clear();
            self.current_event = EventBuilder::new();
        }

        events
    }

    // ── Retired: the complete-body entry point (Phase 118.2, plan 03) ────────
    //
    // There used to be a `feed_complete_body` here — an UNBOUNDED sibling of
    // `feed` that parsed an already-collected body in one piece, stating its byte
    // cap as a precondition on the caller rather than enforcing one. It existed
    // to serve the two whole-body collects in `StreamableHttpTransport`: the GET
    // session stream and the POST response that answers `text/event-stream`.
    // Phase 118.2 turned BOTH of those into incremental reads under `feed`'s
    // bound (D-01/D-02), which left it with no caller at all.
    //
    // It was DELETED rather than kept, because a parser entry point that performs
    // no bound check and has nobody to serve is exactly the attractive nuisance
    // its own documentation warned about: the next caller inherits an obligation
    // stated only in prose, and `feed` is now correct for every case it covered.

    /// Append `data` and drain every COMPLETE line out of the buffer.
    ///
    /// The tokenizer behind [`Self::feed`], which bound-checks before calling
    /// it. Kept as a separate function so the bound and the tokenization stay
    /// visibly distinct concerns.
    fn drain_complete_lines(&mut self, data: &str) -> Vec<SseEvent> {
        // # Invariant — the retained buffer is NEWLINE-FREE on entry
        //
        // The drain loop below runs to exhaustion and `process_line` never
        // touches `buffer`, so every call RETURNS with a newline-free buffer and
        // it starts empty — an unterminated line is all it can ever hold on
        // entry.
        //
        // This is now the SOUNDNESS PRECONDITION for `scan_start` below, not
        // merely a comment: the search skips the retained prefix entirely, which
        // can only miss a line if no `'\n'` can hide in that prefix.
        //
        // It used to be a `debug_assert!(!self.buffer.contains('\n'))`. That
        // assertion was itself an O(retained-length) scan on EVERY call, so it
        // kept this function quadratic in debug builds — precisely the profile
        // `cargo test` and the 1-byte-chunk proptest run in, which would have
        // left the new guards RED after a correct fix. Deleted in plan 113.1-02
        // (D-15); the invariant is pinned instead by the named test
        // `drain_leaves_no_newline_in_the_retained_buffer`, which covers feeds of
        // every shape including a CRLF split across a chunk boundary.
        let scan_start = self.buffer.len();
        self.buffer.push_str(data);
        let mut events = Vec::new();

        // LINEAR, not quadratic — in TWO distinct senses, fixed in two rounds.
        //
        // Per-LINE (fixed in `0493d9fb`): a `drain(..=line_end)` per line memmoves
        // the whole remaining buffer once per line, so one chunk carrying `n`
        // lines cost `O(n * len)` byte moves. A `consumed` cursor advances instead
        // and the single drain below runs once for the whole call, the same shape
        // `take_utf8_prefix` above uses for the same reason.
        //
        // Per-CALL (D-113-R, fixed in plan 113.1-02): the search itself used to
        // restart at offset 0 on every call, re-scanning the whole retained prefix
        // that every earlier call had already scanned. A peer chooses the chunk
        // framing, so a peer chose how many times its bytes got re-scanned —
        // 1-byte chunks cost one full-buffer scan per byte. `search_from` starts
        // at `scan_start`, the length BEFORE this chunk was appended, so each call
        // scans only the bytes it just added. Sound because the retained prefix is
        // newline-free (see the invariant above).
        //
        // `consumed` MUST stay initialised to 0, not to `scan_start`: it drives
        // both the final `drain(..consumed)` and the CRLF lookback guard. When the
        // retained prefix ends with `\r` and this chunk begins with `\n`,
        // `line_end == scan_start` and `line_end - 1` points INTO the retained
        // prefix; the guard `line_end > consumed` is then `scan_start > 0`, which
        // is true, so the `\r` is correctly trimmed. Seeding `consumed` to
        // `scan_start` would make that guard false and leak the `\r` into the line.
        let mut consumed = 0usize;
        let mut search_from = scan_start;
        while let Some(offset) = self.buffer[search_from..].find('\n') {
            let line_end = search_from + offset;
            // `line_end` is a BYTE index (`str::find` returns one). The CRLF
            // check must therefore also be a BYTE check.
            //
            // It used to read `self.buffer.chars().nth(line_end - 1)`, which
            // indexes by CHARACTER. On any buffer containing a multi-byte
            // character the two disagree, so the check could report `'\r'` for
            // a position that is not byte `line_end - 1`, and the slice that
            // followed (`self.buffer[..line_end - 1]`) then cut INSIDE a
            // character and PANICKED — on bytes supplied by a remote server.
            // Found by `client::subscriptions`'s arbitrary-bytes property test
            // (Phase 113-13, T-113-67); `feed_never_panics_on_arbitrary_text`
            // below is the permanent guard.
            //
            // `\n` and `\r` are ASCII, so both `line_end` and `line_end - 1`
            // (taken only when that byte IS `\r`) are guaranteed char
            // boundaries. The `> consumed` guard — not `> 0` — keeps the lookback
            // inside THIS line, so an empty line never reads the previous line's
            // terminator.
            let line_start_len =
                if line_end > consumed && self.buffer.as_bytes()[line_end - 1] == b'\r' {
                    line_end - 1
                } else {
                    line_end
                };
            let line = self.buffer[consumed..line_start_len].to_string();

            if let Some(event) = self.process_line(&line) {
                events.push(event);
            }

            consumed = line_end + 1;
            search_from = consumed;
        }
        self.buffer.drain(..consumed);

        events
    }

    /// Process a single line and potentially emit an event.
    fn process_line(&mut self, line: &str) -> Option<SseEvent> {
        // Empty line dispatches the event
        if line.is_empty() {
            return self.dispatch_event();
        }

        // Comment line (starts with :)
        if line.starts_with(':') {
            return None;
        }

        // Parse field and value
        let (field, value) = if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let value = &line[colon_pos + 1..];
            // Remove leading space from value if present
            let value = value.strip_prefix(' ').unwrap_or(value);
            (field, value)
        } else {
            // Field without value
            (line, "")
        };

        // Process field
        match field {
            "event" => {
                self.current_event.event = Some(value.to_string());
            },
            "data" => {
                if self.current_event.data.is_empty() {
                    self.current_event.data = value.to_string();
                } else {
                    self.current_event.data.push('\n');
                    self.current_event.data.push_str(value);
                }
            },
            "id" if !value.contains('\0') => {
                self.current_event.id = Some(value.to_string());
                self.last_event_id = Some(value.to_string());
            },
            "retry" => {
                if let Ok(retry) = value.parse::<u64>() {
                    self.current_event.retry = Some(retry);
                }
            },
            _ => {
                // Unknown field, ignore
            },
        }

        None
    }

    /// Dispatch the current event if it has data.
    fn dispatch_event(&mut self) -> Option<SseEvent> {
        if self.current_event.data.is_empty() {
            // No data, don't dispatch
            self.current_event = EventBuilder::new();
            return None;
        }

        let event = SseEvent {
            id: self
                .current_event
                .id
                .clone()
                .or_else(|| self.last_event_id.clone()),
            event: self.current_event.event.clone(),
            data: self.current_event.data.clone(),
            retry: self.current_event.retry,
        };

        self.current_event = EventBuilder::new();
        Some(event)
    }

    /// Get the last event ID seen.
    pub fn last_event_id(&self) -> Option<&str> {
        self.last_event_id.as_deref()
    }

    /// Reset the parser state.
    ///
    /// Clears the line buffer and any in-progress event. It deliberately does
    /// NOT clear [`Self::overflowed`], which records that bytes a peer sent were
    /// already LOST — a fact resetting the line state cannot undo.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.current_event = EventBuilder::new();
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for SSE events during parsing.
#[derive(Debug, Clone)]
struct EventBuilder {
    id: Option<String>,
    event: Option<String>,
    data: String,
    retry: Option<u64>,
}

impl EventBuilder {
    fn new() -> Self {
        Self {
            id: None,
            event: None,
            data: String::new(),
            retry: None,
        }
    }
}

/// SSE stream builder for creating SSE responses.
#[derive(Debug)]
pub struct SseStream {
    events: Vec<SseEvent>,
}

impl SseStream {
    /// Create a new SSE stream builder.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Add an event to the stream.
    pub fn event(mut self, event: SseEvent) -> Self {
        self.events.push(event);
        self
    }

    /// Add a simple data event.
    pub fn data(self, data: impl Into<String>) -> Self {
        self.event(SseEvent::new(data))
    }

    /// Add a typed event with data.
    pub fn typed_event(self, event_type: impl Into<String>, data: impl Into<String>) -> Self {
        self.event(SseEvent::new(data).with_event(event_type))
    }

    /// Add a comment line.
    pub fn comment(self, _comment: impl Into<String>) -> Self {
        // Comments are not stored as events, they're just for keep-alive
        // In a real implementation, we'd write this directly to the stream
        self
    }

    /// Build the SSE stream as a string.
    pub fn build(self) -> String {
        self.events
            .into_iter()
            .map(|e| e.to_string())
            .collect::<String>()
    }
}

impl Default for SseStream {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for SSE connections.
#[derive(Debug, Clone)]
pub struct SseConfig {
    /// Reconnection retry interval in milliseconds
    pub retry: u64,
    /// Maximum in-flight bytes a parser may retain.
    ///
    /// [`SseParser::new`] takes its bound from this field's DEFAULT value, and
    /// [`SseParser::with_max_buffer_size`] overrides it per parser. It covers
    /// BOTH of the parser's accumulators — the unterminated line in its buffer
    /// and the `data:` payload of the event still awaiting its blank line. A
    /// chunk that would push that total past the bound is discarded whole and
    /// latches [`SseParser::overflowed`], so no peer can grow the process's heap
    /// without limit by holding a stream open, whether or not it sends newlines.
    pub max_buffer_size: usize,
    /// Enable compression
    pub compression: bool,
    /// Custom headers
    pub headers: HashMap<String, String>,
}

impl Default for SseConfig {
    fn default() -> Self {
        let mut headers = HashMap::new();
        headers.insert("Cache-Control".to_string(), "no-cache".to_string());
        headers.insert("Connection".to_string(), "keep-alive".to_string());

        Self {
            retry: 3000,
            max_buffer_size: DEFAULT_MAX_BUFFER_SIZE,
            compression: false,
            headers,
        }
    }
}

#[cfg(test)]
mod tests {
    //! # Which ALWAYS requirement each piece of plan 113-22 discharges
    //!
    //! CLAUDE.md asks every change for FUZZ, PROPERTY, UNIT and EXAMPLE. Stated
    //! here explicitly rather than left to a reviewer to reconstruct:
    //!
    //! - **PROPERTY** — `property_take_utf8_prefix_retains_at_most_a_three_byte_tail`,
    //!   which proves the ≤3-byte retained-tail bound over arbitrary bytes rather
    //!   than over the three fixtures it used to rest on.
    //! - **UNIT** — the two budget tests, the growth-ratio test, the retained-tail
    //!   companion and the pre-existing output test.
    //! - **FUZZ** — the EXISTING `fuzz_listen_frames` target (113-16), which already
    //!   drives this decoder through `decode_listen_chunks_for_fuzz`. No new target
    //!   is added, and the reason is recorded rather than assumed: D-113-G notes
    //!   that `make quality-gate`'s fuzz stage currently builds 0 of 17 targets and
    //!   swallows failures. That is a known, separately-owned defect; adding an
    //!   18th target that also would not build would be motion, not coverage, and
    //!   this plan does not adopt D-113-G.
    //! - **EXAMPLE** — none. This plan ships no new feature surface, only guards on
    //!   existing behaviour, so there is nothing for an example to demonstrate.
    //!   Said plainly instead of inventing one to satisfy a checklist.

    use super::*;
    use std::time::{Duration, Instant};

    /// Run `body` `runs` times and return the MINIMUM elapsed wall-clock time.
    ///
    /// The minimum, never the mean, and never a single sample: it is the
    /// statistic robust to scheduler preemption on a loaded machine, and — the
    /// property that makes it sound for a complexity guard — noise can only push
    /// a measurement UP. Every run pays the same asymptotic cost, so a quadratic
    /// shape cannot get lucky its way under a ceiling, while a linear one that
    /// happened to be descheduled gets a second chance.
    fn min_elapsed(runs: usize, mut body: impl FnMut()) -> Duration {
        assert!(runs > 0, "a minimum over zero runs is not a measurement");
        let mut best = Duration::MAX;
        for _ in 0..runs {
            let start = Instant::now();
            body();
            best = best.min(start.elapsed());
        }
        best
    }

    /// `take_utf8_prefix` agrees with `String::from_utf8_lossy` on a buffer
    /// that carries valid text, invalid runs and a trailing INCOMPLETE
    /// character — the three cases the cursor rewrite has to keep distinct.
    #[test]
    fn take_utf8_prefix_decodes_mixed_valid_and_invalid_runs() {
        // "a" | 0xff 0xfe (two independent invalid bytes) | "b☂" | 0xE2 0x98
        // (the first two bytes of a three-byte character).
        let mut buffer: Vec<u8> = Vec::new();
        buffer.push(b'a');
        buffer.extend_from_slice(&[0xff, 0xfe]);
        buffer.extend_from_slice("b\u{2602}".as_bytes());
        buffer.extend_from_slice(&[0xE2, 0x98]);

        let text = take_utf8_prefix(&mut buffer);
        assert_eq!(text, "a\u{FFFD}\u{FFFD}b\u{2602}");
        assert_eq!(
            buffer,
            vec![0xE2, 0x98],
            "only the incomplete trailing character is retained"
        );

        // The next chunk completes it, and nothing was lost across the split.
        buffer.push(0x82);
        assert_eq!(take_utf8_prefix(&mut buffer), "\u{2602}");
        assert!(buffer.is_empty());
    }

    /// The OUTPUT of a LARGE invalid-byte run (review CR-02).
    ///
    /// The bytes here are supplied by a remote peer on both incremental feeders,
    /// and this decoder runs BEFORE any of the parser's byte-count bounds apply,
    /// so a per-invalid-byte `Vec::drain` (plus a re-validation from index 0)
    /// was a remote CPU-exhaustion vector. This test pins the OUTPUT rather than
    /// a wall-clock number, so it is deterministic on any machine — and that
    /// output pin is worth having on its own: it is what a decoder that got
    /// "fast" by decoding LESS fails.
    ///
    /// # Correction — this test is NOT the falsifiable complexity guard
    ///
    /// It used to end "…while the input size is what makes a reintroduced
    /// quadratic shape hang the suite instead of passing it". That claim is
    /// arithmetically false, and it is recorded here rather than deleted so the
    /// next reader knows which guard is load-bearing. Quadratic cost scales with
    /// n², and review CR-02 measured the pre-`5f045086` shape at 1.17 s for
    /// 400 KiB, so the 256 KiB here costs about half a second — this test PASSES,
    /// in half a second, on the exact defect it named. That is not a deduction:
    /// plan 113-22 restored the quadratic shape and watched this test go green in
    /// **0.54 s** while the two budget tests below failed. Re-measured at that
    /// execution, at `opt-level = 0` (how `cargo nextest` builds this module):
    /// 256 KiB costs 9.2 ms on the committed cursor scan and 514 ms on the
    /// restored quadratic shape. Both are under any tolerable ceiling, so no
    /// timing assertion at THIS size could separate them.
    ///
    /// The falsifiable guards are `take_utf8_prefix_stays_within_its_linear_time_budget`
    /// and `take_utf8_prefix_cost_grows_linearly_not_quadratically` below, which
    /// move to 1 MiB — where the same two shapes cost 32 ms and 7.9 s (HTTP-09).
    #[test]
    fn take_utf8_prefix_is_linear_over_a_large_invalid_run() {
        const GARBAGE: usize = 256 * 1024;
        let mut buffer = vec![0xffu8; GARBAGE];
        buffer.extend_from_slice(b"data: ok\n\n");

        let text = take_utf8_prefix(&mut buffer);
        assert!(buffer.is_empty(), "nothing completable is retained");
        assert_eq!(
            text.chars().filter(|c| *c == '\u{FFFD}').count(),
            GARBAGE,
            "one replacement character per invalid byte, exactly as from_utf8_lossy"
        );
        assert!(text.ends_with("data: ok\n\n"), "the valid tail survives");
    }

    /// The LOAD-BEARING complexity guard for `take_utf8_prefix` (HTTP-09).
    ///
    /// HTTP-09 asks that no scan over peer-chosen input be worse than O(n). An
    /// output assertion cannot say that, and neither can a timing assertion at a
    /// size where both shapes finish quickly. This one is sized so that the two
    /// shapes land on OPPOSITE sides of the ceiling.
    ///
    /// # Why 1 MiB and why one second
    ///
    /// Measured at `opt-level = 0` (the profile `cargo nextest run` builds this
    /// module with — the crate sets no `[profile.dev]` override) on the plan
    /// 113-22 development machine, over 1 MiB of `0xFF`:
    ///
    /// | shape | cost | vs. this ceiling |
    /// |---|---|---|
    /// | committed single-pass cursor scan | 32 ms | 31x UNDER |
    /// | pre-`5f045086`: re-validate from 0, one `Vec::drain` per invalid run | 7.9 s | 7.9x OVER |
    ///
    /// The gap between those two is the whole test, and BOTH rows were measured,
    /// the second by restoring the old shape and watching this test fail at
    /// 9.39 s. A machine would have to be 31x slower than this one at a
    /// single-pass 1 MiB scan to fail here spuriously — and that is against the
    /// MINIMUM of three runs, so it would have to be 31x slower every time, not
    /// once. Conversely a machine 7.9x FASTER than this one would still fail on
    /// the quadratic shape. The margins are asymmetric and both are written down
    /// on purpose: this is a wall-clock assertion, and the honest defence of one
    /// is its measurement, not its round number.
    ///
    /// The 1 MiB is also where quadratic cost stops being survivable: at 256 KiB
    /// — the size `take_utf8_prefix_is_linear_over_a_large_invalid_run` uses —
    /// the quadratic shape costs 514 ms and passes everything.
    #[test]
    fn take_utf8_prefix_stays_within_its_linear_time_budget() {
        const GARBAGE: usize = 1024 * 1024;
        const CEILING: Duration = Duration::from_secs(1);

        let best = min_elapsed(3, || {
            // A fresh buffer per run: the call consumes it.
            let mut buffer = vec![0xffu8; GARBAGE];
            let text = take_utf8_prefix(&mut buffer);
            std::hint::black_box((&text, &buffer));
        });

        // Emit the measurement even on success (`--success-output=immediate` or
        // `--nocapture` to see it). A wall-clock guard whose margin is invisible
        // until the day it fails is a guard nobody can maintain.
        eprintln!("take_utf8_prefix: {GARBAGE} invalid bytes in {best:?} (ceiling {CEILING:?})");

        assert!(
            best < CEILING,
            "take_utf8_prefix took {best:?} (minimum of 3 runs) over {GARBAGE} bytes of \
             invalid input; the ceiling is {CEILING:?}.\n\
             \n\
             This excludes ONE shape: re-validating the buffer from index 0 each \
             iteration and performing one `Vec::drain` per invalid run, which is \
             O(n^2) byte moves. Measured at opt-level 0: that shape costs ~7.9 s here \
             (9.39 s when this very assertion was run against it) and the committed \
             single-pass cursor scan costs ~32 ms, so this ceiling sits ~31x above \
             linear and ~7.9x below quadratic.\n\
             \n\
             A measurement in the upper region means the quadratic shape is back, not \
             that the machine is slow — no machine that can run this suite needs a \
             second to scan 1 MiB once. Do NOT raise this number to make the test \
             pass; that converts the guard back into the unfalsifiable one this \
             replaced (review CR-02, plan 113-22)."
        );

        // A time budget alone would also be satisfied by a decoder that got fast
        // by decoding LESS. Pin the output at the budget size too, so "fast" has
        // to mean "fast AND still correct".
        let mut buffer = vec![0xffu8; GARBAGE];
        let text = take_utf8_prefix(&mut buffer);
        assert!(
            buffer.is_empty(),
            "nothing completable is retained at 1 MiB"
        );
        assert_eq!(
            text.chars().filter(|c| *c == '\u{FFFD}').count(),
            GARBAGE,
            "one replacement character per invalid byte, at the budget size too"
        );
    }

    /// The SECONDARY signal: cost must grow with the input, not with its square.
    ///
    /// A 4x input step predicts ~4x time under O(n) and ~16x under O(n^2), so a
    /// ceiling of 8x separates them with roughly equal headroom on both sides.
    /// This is machine-independent in a way the absolute budget is not — it
    /// cancels the machine's constant factor — but it is the WEAKER of the two,
    /// because a ratio measured on two short samples is a ratio of two noise
    /// floors. Hence the resolution guard below, and hence the absolute budget
    /// stays load-bearing.
    #[test]
    fn take_utf8_prefix_cost_grows_linearly_not_quadratically() {
        const SMALL: usize = 256 * 1024;
        const BIG: usize = 4 * SMALL;
        // Below this, the measurement is dominated by timer resolution and
        // allocator warm-up rather than by the scan.
        const RESOLUTION_FLOOR: Duration = Duration::from_micros(50);
        const MAX_RATIO: f64 = 8.0;

        let measure = |bytes: usize| {
            min_elapsed(5, || {
                let mut buffer = vec![0xffu8; bytes];
                let text = take_utf8_prefix(&mut buffer);
                std::hint::black_box((&text, &buffer));
            })
        };

        let small = measure(SMALL);
        let big = measure(BIG);

        if small < RESOLUTION_FLOOR {
            eprintln!(
                "take_utf8_prefix_cost_grows_linearly_not_quadratically: SKIPPING the \
                 ratio assertion. The {SMALL}-byte measurement is {small:?}, under the \
                 {RESOLUTION_FLOOR:?} floor, so the ratio would be asserted on noise. \
                 The absolute budget in \
                 take_utf8_prefix_stays_within_its_linear_time_budget is unaffected and \
                 remains load-bearing, so the guard does not degrade to nothing."
            );
            return;
        }

        let ratio = big.as_secs_f64() / small.as_secs_f64();
        eprintln!(
            "take_utf8_prefix growth: {SMALL} -> {small:?}, {BIG} -> {big:?}, \
             ratio {ratio:.2}x (ceiling {MAX_RATIO:.1}x)"
        );
        assert!(
            ratio <= MAX_RATIO,
            "take_utf8_prefix cost grew {ratio:.1}x for a 4x input step \
             ({SMALL} bytes -> {small:?}, {BIG} bytes -> {big:?}); the ceiling is \
             {MAX_RATIO:.1}x.\n\
             \n\
             O(n) predicts ~4x (measured 3.46x on the committed shape: 9.18 ms -> \
             31.8 ms). O(n^2) predicts ~16x (measured 15.38x on the restored \
             pre-`5f045086` shape: 514 ms -> 7.91 s). A ratio in \
             the upper region is the quadratic shape, not scheduling noise — this is a \
             minimum over 5 runs at each size, and noise raises a minimum only by \
             raising every sample."
        );
    }

    /// The same budget on the PUBLIC entry point, over its retained-state path.
    ///
    /// `SseParser::feed` is what a peer actually drives (review WR-04), and the
    /// path with an accumulation risk is the one where nothing completes: 256
    /// newline-free chunks, so every byte is retained and no event dispatches.
    ///
    /// # This is a CEILING, not a complexity proof — and D-113-R is why
    ///
    /// Stated plainly, because an undocumented margin is how the guard this plan
    /// replaced went wrong.
    ///
    /// `feed`'s retained-state path is ALREADY quadratic in total bytes, and
    /// plan 113-22 measured it rather than assuming otherwise. Each call runs
    /// `str::find('\n')` over the WHOLE retained buffer, including the prefix
    /// every earlier call already scanned; a peer chooses the chunking, so a peer
    /// chooses how many times its bytes get re-scanned. Measured in a RELEASE
    /// build, feeding single-byte chunks (one HTTP chunked frame per byte — both
    /// incremental feeders call this once per `hyper` body frame):
    ///
    /// | retained bytes | cost | vs. 16 KiB |
    /// |---|---|---|
    /// | 16 KiB | 5.6 ms | 1x |
    /// | 64 KiB | 59 ms | 10.6x (4x input) |
    /// | 256 KiB | 833 ms | 148x (16x input) |
    ///
    /// 256 KiB is exactly `MAX_LISTEN_LINE_BYTES`. That was recorded as
    /// **D-113-R**, and it is now **CLOSED in plan 113.1-02**: the scan starts at
    /// the offset where the new chunk begins instead of at 0. The table above is
    /// kept as the record of the defect that motivated the fix.
    ///
    /// The consequence for THIS test is unchanged and is why it stays: at 4 KiB
    /// chunks the per-chunk re-scan is a memchr over at most 1 MiB, single-digit
    /// milliseconds in total, so no absolute ceiling here can separate "linear"
    /// from "quadratic with a memchr-sized constant". Two negative controls
    /// confirm it, and BOTH are recorded rather than erased:
    ///
    /// - injecting T-113-102's per-chunk full-buffer copy moved this measurement
    ///   from 6.7 ms to 11.7 ms and the test still PASSED (113-22-SUMMARY.md);
    /// - plan 113.1-02's own post-fix control reverted the scan-window cursor,
    ///   and this test PASSED at 5.13 ms in the very run where the two guards
    ///   below failed at 4.36 s and 14.85x (113.1-02-SUMMARY.md).
    ///
    /// # Which of the three `feed` guards owns which shape
    ///
    /// | test | shape it owns | kind |
    /// |---|---|---|
    /// | this one | many lines per chunk, 4 KiB x 256 | absolute ceiling |
    /// | `sse_parser_feed_1b_chunks_stays_within_its_linear_time_budget` | adversarial 1-byte chunking | absolute ceiling |
    /// | `sse_parser_feed_cost_grows_linearly_not_quadratically` | adversarial 1-byte chunking | ratio, machine-independent |
    ///
    /// So what this test IS: a ceiling that catches a regression making the
    /// retained-state path egregiously expensive (a per-chunk re-parse or
    /// re-validation) at the many-lines-per-chunk shape — the failure mode fixed
    /// in `0493d9fb` — plus a pin on the retention contract: nothing dispatched,
    /// nothing overflowed, every byte still held. It is NOT the falsifiable O(n)
    /// claim for HTTP-09 clause 3; that lives in the two guards named above, and
    /// in `take_utf8_prefix_stays_within_its_linear_time_budget` for the scan that
    /// runs BEFORE this one and before every byte-count bound.
    #[test]
    fn sse_parser_feed_stays_within_its_linear_time_budget() {
        const CHUNK_BYTES: usize = 4 * 1024;
        const CHUNKS: usize = 256;
        // Large enough that the retained total (1 MiB) never trips the bound —
        // this test is about cost, not about enforcement.
        const BOUND: usize = 4 * 1024 * 1024;
        const CEILING: Duration = Duration::from_secs(1);

        let chunk = "x".repeat(CHUNK_BYTES);

        let best = min_elapsed(3, || {
            let mut parser = SseParser::with_max_buffer_size(BOUND);
            for _ in 0..CHUNKS {
                std::hint::black_box(parser.feed(&chunk));
            }
            std::hint::black_box(parser.buffered_bytes());
        });

        eprintln!(
            "SseParser::feed: {CHUNKS} x {CHUNK_BYTES} retained bytes in {best:?} \
             (ceiling {CEILING:?})"
        );

        assert!(
            best < CEILING,
            "SseParser::feed took {best:?} (minimum of 3 runs) to retain {CHUNKS} \
             newline-free chunks of {CHUNK_BYTES} bytes; the ceiling is {CEILING:?}. \
             The retained-state path costs single-digit milliseconds here, so a \
             measurement anywhere near this ceiling means a per-chunk cost over the \
             whole retained buffer was introduced. Bound the work per chunk; do not \
             raise this number."
        );

        // The retention contract this path depends on, checked on an untimed run
        // so a failure reads as a behaviour change rather than a slow machine.
        let mut parser = SseParser::with_max_buffer_size(BOUND);
        for _ in 0..CHUNKS {
            assert!(
                parser.feed(&chunk).is_empty(),
                "no chunk carries a newline, so no line completes and nothing dispatches"
            );
        }
        assert!(
            !parser.overflowed(),
            "{} retained bytes is under the {BOUND}-byte bound",
            parser.buffered_bytes()
        );
        assert_eq!(
            parser.buffered_bytes(),
            CHUNKS * CHUNK_BYTES,
            "every fed byte is still retained — that is what makes this the \
             accumulating path"
        );
    }

    /// The LOAD-BEARING absolute budget for `feed` at ADVERSARIAL chunking (HTTP-09 clause 3).
    ///
    /// `sse_parser_feed_stays_within_its_linear_time_budget` above cannot catch
    /// this defect class and says so in its own rustdoc: at its 4 KiB chunking
    /// the per-call re-scan is a memchr over at most 1 MiB, single-digit
    /// milliseconds, and a negative control (a per-chunk full-buffer copy) moved
    /// it 6.7 ms -> 11.7 ms while it still PASSED. The shape that separates
    /// linear from quadratic is the one a peer actually gets to choose: **one
    /// byte per `feed()` call**, with nothing ever completing, so the retained
    /// prefix grows monotonically and every call re-scans it.
    ///
    /// # Why 512 KiB and why one second
    ///
    /// Measured at `opt-level = 0` (the profile `cargo test` builds this module
    /// with — the crate sets no `[profile.dev]` override), 512 KiB fed one byte
    /// per call, minimum of 3 runs, on the plan 113.1-02 development machine:
    ///
    /// | shape | cost | vs. this ceiling |
    /// |---|---|---|
    /// | committed scan-window cursor | 63.6 ms | 15.7x UNDER |
    /// | pre-113.1-02: `find` restarting at offset 0 every call | 6.81 s | 6.8x OVER |
    ///
    /// BOTH rows were measured, the second twice: once against the unfixed tree
    /// before the fix landed, and again by a post-fix negative control that
    /// reverted only the cursor. The margins are asymmetric and written down on
    /// purpose — the honest defence of a wall-clock assertion is its measurement.
    #[test]
    fn sse_parser_feed_1b_chunks_stays_within_its_linear_time_budget() {
        const CHUNKS: usize = 512 * 1024;
        // Large enough that the retained total never trips the bound — this test
        // is about cost, not about enforcement. `DEFAULT_MAX_BUFFER_SIZE` is
        // 1 MiB, which would admit 512 KiB, but the bound is stated explicitly so
        // the test does not silently change meaning if that default moves.
        const BOUND: usize = 4 * 1024 * 1024;
        const CEILING: Duration = Duration::from_secs(1);

        let best = min_elapsed(3, || {
            let mut parser = SseParser::with_max_buffer_size(BOUND);
            // Neither `\n` nor `\r`: nothing ever drains, so every call scans a
            // buffer one byte longer than the last. That is the adversarial shape.
            for _ in 0..CHUNKS {
                std::hint::black_box(parser.feed("x"));
            }
            std::hint::black_box(parser.buffered_bytes());
        });

        eprintln!("SseParser::feed: {CHUNKS} single-byte chunks in {best:?} (ceiling {CEILING:?})");

        assert!(
            best < CEILING,
            "SseParser::feed took {best:?} (minimum of 3 runs) to absorb {CHUNKS} \
             single-byte chunks; the ceiling is {CEILING:?}.\n\
             \n\
             This excludes ONE shape: restarting the `find('\\n')` search at offset \
             0 on every call, so a peer sending N one-byte chunks pays O(N^2) \
             scanning. Measured at opt-level 0: that shape costs ~6.81 s here and \
             the committed scan-window cursor costs ~63.6 ms, so this ceiling sits \
             ~15.7x above linear and ~6.8x below quadratic.\n\
             \n\
             A measurement in the upper region means the per-call rescan is back, \
             not that the machine is slow. Do NOT raise this number to make the \
             test pass; that converts the guard back into the unfalsifiable ceiling \
             D-113-R exists to record (plan 113.1-02)."
        );

        // A time budget alone would also be satisfied by a parser that got fast by
        // retaining LESS. Pin the retention contract at the budget size too, so
        // "fast" has to mean "fast AND still holding every byte".
        let mut parser = SseParser::with_max_buffer_size(BOUND);
        for _ in 0..CHUNKS {
            assert!(
                parser.feed("x").is_empty(),
                "no chunk carries a newline, so no line completes and nothing dispatches"
            );
        }
        assert_eq!(
            parser.buffered_bytes(),
            CHUNKS,
            "every fed byte is still retained — that is what makes this the \
             accumulating path"
        );
    }

    /// The MACHINE-INDEPENDENT guard for `feed` at adversarial chunking (D-16).
    ///
    /// `take_utf8_prefix` carries both an absolute budget and this ratio; `feed`
    /// only ever got the budget half. A 4x input step predicts ~4x time under
    /// O(n) and ~16x under O(n^2), so a ceiling of 8x separates them with roughly
    /// equal log-scale headroom.
    ///
    /// This one cannot be flaked by a slow or loaded CI box — it cancels the
    /// machine's constant factor — and unlike the absolute budget it also catches
    /// a merely-worse-but-still-under-ceiling regression. Measured on the plan
    /// 113.1-02 development machine, both sizes fed one byte per call:
    ///
    /// | shape | 64 KiB | 256 KiB | ratio |
    /// |---|---|---|---|
    /// | committed scan-window cursor | 6.70 ms | 29.4 ms | **4.39x** |
    /// | pre-113.1-02: `find` restarting at 0 | 113 ms | 1.71 s | **15.06x** |
    #[test]
    fn sse_parser_feed_cost_grows_linearly_not_quadratically() {
        const SMALL: usize = 64 * 1024;
        const BIG: usize = 4 * SMALL;
        // Below this, the measurement is dominated by timer resolution and
        // allocator warm-up rather than by the scan.
        const RESOLUTION_FLOOR: Duration = Duration::from_micros(50);
        const MAX_RATIO: f64 = 8.0;
        const BOUND: usize = 4 * 1024 * 1024;

        let measure = |chunks: usize| {
            min_elapsed(3, || {
                let mut parser = SseParser::with_max_buffer_size(BOUND);
                for _ in 0..chunks {
                    std::hint::black_box(parser.feed("x"));
                }
                std::hint::black_box(parser.buffered_bytes());
            })
        };

        let small = measure(SMALL);
        let big = measure(BIG);

        if small < RESOLUTION_FLOOR {
            eprintln!(
                "sse_parser_feed_cost_grows_linearly_not_quadratically: SKIPPING the \
                 ratio assertion. The {SMALL}-chunk measurement is {small:?}, under \
                 the {RESOLUTION_FLOOR:?} floor, so the ratio would be asserted on \
                 noise. The absolute budget in \
                 sse_parser_feed_1b_chunks_stays_within_its_linear_time_budget is \
                 unaffected and remains load-bearing, so the guard does not degrade \
                 to nothing."
            );
            return;
        }

        let ratio = big.as_secs_f64() / small.as_secs_f64();
        eprintln!(
            "SseParser::feed growth: {SMALL} -> {small:?}, {BIG} -> {big:?}, \
             ratio {ratio:.2}x (ceiling {MAX_RATIO:.1}x)"
        );
        assert!(
            ratio <= MAX_RATIO,
            "SseParser::feed cost grew {ratio:.1}x for a 4x input step \
             ({SMALL} single-byte chunks -> {small:?}, {BIG} -> {big:?}); the \
             ceiling is {MAX_RATIO:.1}x.\n\
             \n\
             O(n) predicts ~4x (measured 4.39x on the committed scan-window cursor: \
             6.70 ms -> 29.4 ms). O(n^2) predicts ~16x (measured 15.06x on the \
             pre-113.1-02 shape that restarts `find` at offset 0: 113 ms -> \
             1.71 s). A ratio in the upper region is the quadratic shape, not \
             scheduling noise — this is a minimum over 3 runs at each size, and \
             noise raises a minimum only by raising every sample."
        );
    }

    /// The soundness precondition for the scan-window cursor, as a named test.
    ///
    /// `drain_complete_lines` starts its `find('\n')` search at the byte offset
    /// where the NEW chunk begins, skipping the retained prefix. That is only
    /// correct because the retained prefix cannot contain a `'\n'`. This test is
    /// what pins that invariant.
    ///
    /// It replaces the per-call `debug_assert!(!self.buffer.contains('\n'))`
    /// deleted in plan 113.1-02: that assertion was itself an O(retained-length)
    /// scan on EVERY call, so it kept the function quadratic in debug builds —
    /// which is exactly the profile `cargo test` and the 1-byte-chunk proptest
    /// run in. The invariant is unchanged; only its enforcement moved from a
    /// per-call scan to one test covering feeds of every shape.
    #[test]
    fn drain_leaves_no_newline_in_the_retained_buffer() {
        // Each case is a sequence of chunks fed to ONE parser in order.
        let cases: &[(&str, &[&str])] = &[
            (
                "a line split mid-way across two chunks",
                &["data: hel", "lo\n\n"],
            ),
            (
                "a CRLF split across a chunk boundary",
                &["data: x\r", "\n\r\n"],
            ),
            ("a bare empty line", &["\n\n"]),
            (
                "a chunk that is exactly one newline",
                &["data: y", "\n", "\n"],
            ),
            // NOT "a multi-byte character split across two feeds" — that is
            // unrepresentable through `feed(&str)`, since every `&str` is valid
            // UTF-8 by construction. What IS reachable is a chunk boundary placed
            // immediately before and immediately after such a character.
            (
                "a boundary hugging a multi-byte character",
                &["data: ", "\u{2602}", "\u{4f60}\u{597d}\n\n"],
            ),
            ("a lone CR retained with no LF yet", &["data: z\r"]),
            ("nothing but a partial line", &["data: unterminated"]),
        ];

        for (name, chunks) in cases {
            let mut parser = SseParser::new();
            for chunk in *chunks {
                let _ = parser.feed(chunk);
                assert!(
                    !parser.buffer.contains('\n'),
                    "case {name:?}: the retained buffer holds a newline ({:?}) after \
                     feeding {chunk:?}. The scan-window cursor in \
                     drain_complete_lines skips this prefix, so a newline hiding in \
                     it would silently drop or merge SSE frames.",
                    parser.buffer,
                );
            }
        }
    }

    /// The named, readable companion to the retained-tail property test below.
    ///
    /// A 4-byte UTF-8 sequence truncated to its first 3 bytes is the WORST case:
    /// 3 is the largest incomplete-character prefix that exists, so it is the
    /// exact figure `take_utf8_prefix`'s rustdoc promises ("at most 3 bytes").
    /// The property test proves the bound over arbitrary input; this states what
    /// the number means, so a future reader does not have to derive `4` from a
    /// `prop_assert!`.
    #[test]
    fn take_utf8_prefix_retained_tail_is_documented_bound() {
        // U+1F600 GRINNING FACE = F0 9F 98 80, cut one byte short.
        let mut buffer = vec![0xF0u8, 0x9F, 0x98];
        let text = take_utf8_prefix(&mut buffer);

        assert!(text.is_empty(), "nothing is decodable yet");
        assert_eq!(
            buffer,
            vec![0xF0, 0x9F, 0x98],
            "all three bytes are retained for the next chunk — the maximum this \
             function may ever hold, and the reason the accumulation in \
             `connect_sse` and in the listen client is bounded across iterations"
        );

        // The next chunk completes it and nothing was lost across the split.
        buffer.push(0x80);
        assert_eq!(take_utf8_prefix(&mut buffer), "\u{1F600}");
        assert!(buffer.is_empty());
    }

    proptest::proptest! {
        /// The ≤3-byte retained tail, over ARBITRARY bytes rather than three
        /// hand-picked fixtures (HTTP-09, T-113-103).
        ///
        /// This is the invariant that makes the accumulation in `connect_sse`'s
        /// reader and in the `subscriptions/listen` client bounded ACROSS
        /// iterations: each call hands back a buffer holding at most an
        /// incomplete character, so the undecoded `Vec` cannot grow with stream
        /// age no matter how a peer frames its chunks. Until now it rested on
        /// the three fixtures in
        /// `take_utf8_prefix_decodes_mixed_valid_and_invalid_runs`.
        #[test]
        fn property_take_utf8_prefix_retains_at_most_a_three_byte_tail(
            bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..4096),
        ) {
            let mut buffer = bytes.clone();
            let _text = take_utf8_prefix(&mut buffer);

            // (a) the documented bound. Only the HEAD of an over-long residual is
            // printed: on failure this can hold thousands of bytes, and a hex dump
            // of all of them buries the number that matters.
            proptest::prop_assert!(
                buffer.len() < 4,
                "retained {} bytes (starting {:x?}) — the incomplete-character tail \
                 is at most 3, and a larger residual is unbounded accumulation \
                 across chunks, not a decode detail",
                buffer.len(),
                &buffer[..buffer.len().min(8)],
            );

            // (b) the residual is a SUFFIX of the input. Anything else would mean
            // bytes were reordered or resurrected from the middle of the buffer,
            // and the caller appends the NEXT chunk directly onto this residual.
            proptest::prop_assert!(
                bytes.ends_with(&buffer),
                "residual {:x?} is not a suffix of the input",
                buffer,
            );

            // (c) no wedge. Feeding the residual plus one more ASCII byte always
            // terminates AND always clears: the retained tail is by construction
            // an incomplete-character PREFIX at the end of the buffer, and a
            // buffer ending in a complete 1-byte character has no such tail. This
            // is what stops hostile input from parking bytes in the decoder
            // forever (T-113-67).
            buffer.push(b'!');
            let follow_up = take_utf8_prefix(&mut buffer);
            proptest::prop_assert!(
                buffer.len() < 4,
                "the second call retained {} bytes",
                buffer.len(),
            );
            proptest::prop_assert!(
                buffer.is_empty(),
                "an ASCII byte completes the buffer, so nothing may be retained; \
                 residual {:x?} after decoding {:?}",
                buffer,
                follow_up,
            );
        }
    }

    #[test]
    fn test_sse_parser_simple() {
        let mut parser = SseParser::new();

        let input = "data: hello world\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello world");
        assert_eq!(events[0].event, None);
        assert_eq!(events[0].id, None);
    }

    #[test]
    fn test_sse_parser_with_event_type() {
        let mut parser = SseParser::new();

        let input = "event: message\ndata: hello\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
        assert_eq!(events[0].event, Some("message".to_string()));
    }

    #[test]
    fn test_sse_parser_multiline_data() {
        let mut parser = SseParser::new();

        let input = "data: line 1\ndata: line 2\ndata: line 3\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line 1\nline 2\nline 3");
    }

    #[test]
    fn test_sse_parser_with_id() {
        let mut parser = SseParser::new();

        let input = "id: 123\ndata: test\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, Some("123".to_string()));
        assert_eq!(parser.last_event_id(), Some("123"));
    }

    #[test]
    fn test_sse_parser_with_retry() {
        let mut parser = SseParser::new();

        let input = "retry: 5000\ndata: test\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].retry, Some(5000));
    }

    #[test]
    fn test_sse_parser_comments() {
        let mut parser = SseParser::new();

        let input = ": this is a comment\ndata: actual data\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "actual data");
    }

    #[test]
    fn test_sse_parser_incremental() {
        let mut parser = SseParser::new();

        // Feed data incrementally
        let events1 = parser.feed("data: par");
        assert_eq!(events1.len(), 0);

        let events2 = parser.feed("tial\ndata: more");
        assert_eq!(events2.len(), 0);

        let events3 = parser.feed("\n\n");
        assert_eq!(events3.len(), 1);
        assert_eq!(events3[0].data, "partial\nmore");
    }

    /// A remote peer that never emits a newline must not be able to grow a
    /// pmcp client's heap without limit.
    ///
    /// `feed` pushes every chunk into `buffer` and only ever drains as far as a
    /// `\n`, so before the bound existed a hostile or broken server could hold a
    /// `subscriptions/listen` stream open and stream newline-free bytes until
    /// the client ran out of memory (review CR-03, verification gap item 3,
    /// T-113-73).
    #[test]
    fn a_newlineless_flood_cannot_grow_the_buffer_past_the_bound() {
        let bound = SseConfig::default().max_buffer_size;
        let mut parser = SseParser::new();
        let chunk = "x".repeat(64 * 1024);
        // 2 MiB, with not one newline in it.
        for _ in 0..32 {
            assert!(
                parser.feed(&chunk).is_empty(),
                "an unterminated line completes no event"
            );
        }
        assert!(
            parser.buffered_bytes() <= bound,
            "the parser retained {} bytes, past the {bound}-byte bound",
            parser.buffered_bytes()
        );
    }

    /// The TWIN of the test above, and the one that matters: a peer sending
    /// perfectly ordinary NEWLINE-TERMINATED `data:` lines, forever, without ever
    /// sending the blank line that would dispatch the event.
    ///
    /// Every chunk here is well-formed, so the pre-113-17 bound — which only
    /// refused chunks carrying no newline at all — never fired: 100 000 chunks
    /// under a 64-byte bound accumulated 899 999 bytes into `current_event.data`
    /// with `overflowed()` still false (independently reproduced by the phase
    /// verifier; review CR-01, T-113-79). The bound must cover what accumulates
    /// ACROSS lines, not only the unterminated line.
    #[test]
    fn a_newline_carrying_flood_cannot_grow_the_event_past_the_bound() {
        let mut parser = SseParser::with_max_buffer_size(64);
        for _ in 0..100_000 {
            let _ = parser.feed("data: AAAAAAAA\n");
        }
        assert!(
            parser.overflowed(),
            "100,000 newline-terminated `data:` lines accumulated {} bytes \
             without tripping the 64-byte bound",
            parser.buffered_bytes()
        );
        assert!(
            parser.buffered_bytes() <= 64,
            "the parser retains {} bytes, past its 64-byte bound",
            parser.buffered_bytes()
        );
    }

    /// A single COMPLETE line larger than the bound is REFUSED, not parsed and
    /// emitted.
    ///
    /// Before 113-17 the `contains('\n')` escape let this through whole: the
    /// parser returned one 1 000 000-byte event from a 64-byte-bounded parser
    /// with `overflowed()` false, so the flag could not observe the condition it
    /// exists to detect (review CR-02, T-113-80).
    #[test]
    fn an_oversized_complete_line_is_refused_not_emitted() {
        let mut parser = SseParser::with_max_buffer_size(64);
        let body = format!("data: {}\n\n", "B".repeat(1_000_000));

        let events = parser.feed(&body);

        assert!(
            events.is_empty(),
            "a 1,000,000-byte line under a 64-byte bound must be refused; got \
             {} event(s), the first of {} bytes",
            events.len(),
            events.first().map_or(0, |event| event.data.len())
        );
        assert!(parser.overflowed(), "and the refusal is observable");
    }

    /// The default bound is the one `SseConfig` already documented — the number
    /// is sourced from there, not re-typed, so there is exactly ONE of it.
    #[test]
    fn new_takes_its_bound_from_the_sse_config_default() {
        let parser = SseParser::new();
        assert_eq!(parser.max_buffer_size, SseConfig::default().max_buffer_size);
        assert_eq!(parser.max_buffer_size, 1024 * 1024, "1 MiB");
        assert!(!parser.overflowed(), "a fresh parser has lost nothing");
    }

    /// The limit is real CONFIG, not a constant baked into `feed`: a parser
    /// built with a tighter bound trips on bytes the default-bounded one
    /// swallows without complaint.
    #[test]
    fn with_max_buffer_size_bounds_at_the_value_given() {
        let flood = "x".repeat(256);

        let mut tight = SseParser::with_max_buffer_size(64);
        assert!(tight.feed(&flood).is_empty());
        assert!(tight.overflowed(), "256 bytes is past a 64-byte bound");
        assert!(tight.buffer.is_empty(), "the oversized line was discarded");

        let mut wide = SseParser::new();
        let _ = wide.feed(&flood);
        assert!(
            !wide.overflowed(),
            "the same bytes are nowhere near the 1 MiB default"
        );
    }

    /// The flag never auto-clears, so a caller polling once per chunk cannot
    /// miss the event even when well-formed frames follow the bad one.
    #[test]
    fn the_overflow_flag_latches() {
        let mut parser = SseParser::with_max_buffer_size(64);
        assert!(parser.feed(&"x".repeat(256)).is_empty());
        assert!(parser.overflowed());

        let events = parser.feed("data: ok\n\n");
        assert_eq!(events.len(), 1, "the parser keeps working");
        assert_eq!(events[0].data, "ok");
        assert!(parser.overflowed(), "and the flag stays set");

        parser.reset();
        assert!(
            parser.overflowed(),
            "reset cannot un-lose the discarded bytes"
        );
    }

    /// Events completed BEFORE the oversized line are still delivered, and the
    /// overflowing feed itself completes nothing rather than emitting a
    /// truncated frame that would fail JSON parsing with a misleading error.
    #[test]
    fn events_completed_before_an_oversized_line_are_still_returned() {
        let mut parser = SseParser::with_max_buffer_size(64);

        let events = parser.feed("data: first\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "first");
        assert!(!parser.overflowed());

        let events = parser.feed(&"x".repeat(256));
        assert!(events.is_empty(), "an overflowing feed completes nothing");
        assert!(parser.overflowed());
    }

    /// A chunk that CONTAINS a newline skips the pre-check, so the bound has to
    /// hold on the RESIDUAL the drain loop leaves behind. Without the residual
    /// check a single `"\n" + 256 bytes` chunk parks 256 bytes in a 64-byte
    /// parser — and a peer can repeat that with megabyte chunks.
    #[test]
    fn a_newline_prefixed_flood_still_trips_the_bound() {
        let mut parser = SseParser::with_max_buffer_size(64);

        let mut chunk = String::from("\n");
        chunk.push_str(&"x".repeat(256));
        let events = parser.feed(&chunk);

        assert!(
            events.is_empty(),
            "the leading blank line dispatches nothing"
        );
        assert!(
            parser.overflowed(),
            "the residual unterminated line exceeds the bound and is discarded"
        );

        // The parser keeps working afterwards, exactly as after a pre-check trip.
        let events = parser.feed("data: ok\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "ok");
    }

    /// A parser that never exceeds its bound behaves byte-identically to the
    /// pre-bound parser — every other test in this module is the rest of that
    /// proof; this one pins the flag itself.
    #[test]
    fn a_parser_under_its_bound_never_reports_overflow() {
        let mut parser = SseParser::new();
        let events = parser.feed("id: 7\nevent: message\ndata: {\"a\":1}\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "{\"a\":1}");
        assert_eq!(parser.last_event_id(), Some("7"));
        assert!(!parser.overflowed());
    }

    #[test]
    fn test_sse_stream_builder() {
        let stream = SseStream::new()
            .data("simple message")
            .typed_event("ping", "pong")
            .event(SseEvent::new("complex").with_id("42").with_retry(1000))
            .build();

        assert!(stream.contains("data: simple message"));
        assert!(stream.contains("event: ping"));
        assert!(stream.contains("data: pong"));
        assert!(stream.contains("id: 42"));
        assert!(stream.contains("retry: 1000"));
    }

    /// Regression: a multi-byte character before the first `\n`, with a `\r`
    /// later in the buffer, used to PANIC with "byte index N is not a char
    /// boundary".
    ///
    /// The old CRLF check indexed by CHARACTER (`chars().nth(line_end - 1)`)
    /// while `line_end` is a BYTE index, so it reported `'\r'` for a position
    /// that was actually inside `'\u{2602}'`, and the slice that followed cut
    /// mid-character. These bytes come off the wire from a remote server, so
    /// the panic was a remote-triggerable client crash (T-113-67).
    #[test]
    fn feed_does_not_panic_on_a_multibyte_char_before_a_later_cr() {
        let mut parser = SseParser::new();
        // bytes: 0..2 = '\u{2602}', 3 = '\n', 4 = '\r', 5 = 'X', 6 = '\n'.
        // `find('\n')` is 3 and `chars().nth(2)` was `'\r'` — the disagreement.
        let events = parser.feed("\u{2602}\n\rX\n");
        assert!(
            events.is_empty(),
            "neither line carries data, so nothing dispatches: {events:?}"
        );
    }

    /// A CRLF-terminated line still has its `\r` stripped, and a multi-byte
    /// payload survives intact.
    #[test]
    fn feed_strips_crlf_and_preserves_multibyte_data() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: \u{2602}-\u{4f60}\u{597d}\r\n\r\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "\u{2602}-\u{4f60}\u{597d}");
    }

    proptest::proptest! {
        /// `feed` runs on bytes a remote peer chose. It must never panic, for
        /// ANY text, at ANY chunk split.
        #[test]
        fn feed_never_panics_on_arbitrary_text(
            chunks in proptest::collection::vec(
                "(\\PC|\r|\n|\u{2602}|\u{4f60}){0,40}",
                0..4,
            ),
        ) {
            let mut parser = SseParser::new();
            for chunk in chunks {
                let _ = parser.feed(&chunk);
            }
        }

        /// The SAME text, re-cut at different CHARACTER boundaries, must produce
        /// the same events and leave the same retained tail.
        ///
        /// This is the property the scan-window cursor (D-12) can violate: it
        /// starts each search at the offset where the new chunk begins, so a bug
        /// in where that offset lands would drop or merge frames only for SOME
        /// chunkings — invisible to any fixed-fixture test. The claim is
        /// deliberately *arbitrary text under arbitrary character-boundary
        /// chunking*, and NOT a claim over arbitrary raw bytes: `feed` takes
        /// `&str`, and every `&str` is valid UTF-8 by construction, so a `feed`
        /// call can never receive half a character. Byte-level fragmentation is
        /// [`take_utf8_prefix`]'s responsibility and has its own coverage there
        /// (`property_take_utf8_prefix_retains_at_most_a_three_byte_tail`), so
        /// this property has no hole where that case would be.
        ///
        /// The strategy is the block's established alphabet, which already mixes
        /// `\r`, `\n` and multi-byte characters — exactly what this needs.
        #[test]
        fn property_feed_chunking_invariance(
            chunks_a in proptest::collection::vec(
                "(\\PC|\r|\n|\u{2602}|\u{4f60}){0,40}",
                0..4,
            ),
            split_sizes in proptest::collection::vec(1usize..7usize, 1..8),
        ) {
            // Splitting A is the generated vector itself, so the reference text is
            // its concatenation — which makes character-boundary safety structural
            // rather than something the test has to get right.
            let text: String = chunks_a.concat();

            // Splitting B re-cuts that same text at independent character counts,
            // never at raw byte offsets (slicing `&str` at a non-boundary index
            // panics).
            let chars: Vec<char> = text.chars().collect();
            let mut chunks_b: Vec<String> = Vec::new();
            let mut at = 0usize;
            // `split_sizes` is generated non-empty, so the cycle never ends.
            let mut sizes = split_sizes.iter().copied().cycle();
            while at < chars.len() {
                let end = (at + sizes.next().unwrap_or(1)).min(chars.len());
                chunks_b.push(chars[at..end].iter().collect());
                at = end;
            }

            // The newline-free retained buffer is the SOUNDNESS PRECONDITION for
            // the scan-window cursor (D-15): `scan_start` skips the retained
            // prefix, which is only correct if no '\n' can hide there. The
            // per-call `debug_assert` that used to enforce it universally was
            // deleted because it was itself an O(retained) scan. Asserting it
            // here restores generative coverage — this test already drives
            // arbitrary text under arbitrary chunkings, which is exactly the
            // generator that finds shapes the seven fixtures in
            // `drain_leaves_no_newline_in_the_retained_buffer` do not.
            let mut parser_a = SseParser::new();
            let mut events_a = Vec::new();
            for chunk in &chunks_a {
                events_a.extend(parser_a.feed(chunk));
                proptest::prop_assert!(
                    !parser_a.buffer.contains('\n'),
                    "retained buffer holds a newline after feeding {:?}: {:?}",
                    chunk,
                    parser_a.buffer
                );
            }

            let mut parser_b = SseParser::new();
            let mut events_b = Vec::new();
            for chunk in &chunks_b {
                events_b.extend(parser_b.feed(chunk));
                proptest::prop_assert!(
                    !parser_b.buffer.contains('\n'),
                    "retained buffer holds a newline after feeding {:?}: {:?}",
                    chunk,
                    parser_b.buffer
                );
            }

            proptest::prop_assert_eq!(
                &events_a,
                &events_b,
                "the same text emitted a different event sequence under a \
                 different chunking (A: {:?}, B: {:?})",
                chunks_a,
                chunks_b
            );
            proptest::prop_assert_eq!(
                &parser_a.buffer,
                &parser_b.buffer,
                "the same text left a different retained tail under a different \
                 chunking (A: {:?}, B: {:?})",
                chunks_a,
                chunks_b
            );
        }

        /// And neither does a TIGHTLY bounded parser, whose enforcement branch
        /// discards a partial line mid-stream — including one cut in the middle
        /// of a multi-byte character (the 113-13 char-boundary guard, now
        /// exercised against the bound as well).
        #[test]
        fn a_bounded_feed_never_panics_on_arbitrary_text(
            chunks in proptest::collection::vec(
                "(\\PC|\r|\n|\u{2602}|\u{4f60}){0,40}",
                0..4,
            ),
        ) {
            let mut parser = SseParser::with_max_buffer_size(8);
            for chunk in chunks {
                let _ = parser.feed(&chunk);
                // The bound holds on RETURN, over BOTH accumulators, with no
                // slack for "the tail of this chunk". The looser form this
                // replaced (`buffer.len() <= max(8, chunk.len())`) is exactly
                // why the property test could not see GAP-A: it excused any
                // residual up to the size of the chunk, and it ignored the
                // event accumulator entirely (review CR-02).
                proptest::prop_assert!(
                    parser.buffered_bytes() <= parser.max_buffer_size(),
                    "the parser retains {} bytes, past its {}-byte bound",
                    parser.buffered_bytes(),
                    parser.max_buffer_size(),
                );
            }
        }
    }

    #[test]
    fn test_sse_event_display() {
        let event = SseEvent::new("test data")
            .with_id("123")
            .with_event("message")
            .with_retry(3000);

        let output = event.to_string();
        assert!(output.contains("id: 123"));
        assert!(output.contains("event: message"));
        assert!(output.contains("retry: 3000"));
        assert!(output.contains("data: test data"));
    }
}
