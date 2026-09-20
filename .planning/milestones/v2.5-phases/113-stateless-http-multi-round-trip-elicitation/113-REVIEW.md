---
phase: 113-stateless-http-multi-round-trip-elicitation
reviewed: 2026-07-26T14:20:00Z
depth: standard
files_reviewed: 9
files_reviewed_list:
  - src/shared/sse_parser.rs
  - src/shared/streamable_http.rs
  - src/shared/http.rs
  - src/client/subscriptions.rs
  - src/client/mod.rs
  - src/server/subscriptions.rs
  - tests/v2_subscriptions.rs
  - tests/v2_subscriptions_client.rs
  - fuzz/fuzz_targets/subscription_listen_frames.rs
findings:
  critical: 3
  warning: 12
  info: 0
  total: 15
status: issues_found
---

# Phase 113: Code Review Report (re-review after gap closure 113-17…113-20)

**Reviewed:** 2026-07-26T14:20:00Z
**Depth:** standard
**Files Reviewed:** 9
**Status:** issues_found

## Summary

The four gap-closure plans do, mechanically, what they say they do. Each claim was
checked against the code:

| Claim | Verdict |
|---|---|
| 113-17: `feed`'s pre-check dropped the `!data.contains('\n')` escape; bounds `buffered_bytes() + chunk` unconditionally | **TRUE** — `sse_parser.rs:391` |
| 113-17: post-drain check widened from `buffer.len()` to `buffered_bytes()` | **TRUE** — `sse_parser.rs:412` |
| 113-17: `DEFAULT_HTTP_SSE_BUFFERED_BYTES` as a private field + additive builder | **TRUE** — `http.rs:71,107,209` |
| 113-18: all three `ListenRejection`s map to `RATE_LIMITED` | **TRUE** — `server/subscriptions.rs:431` |
| 113-18: `RATE_LIMITED` is not in `v2_status_for_code`'s 400 arm ⇒ HTTP 200 | **TRUE** — `streamable_http_server.rs:690-702` |
| 113-18: `prune_after_rejection` on both entry-creating rejection paths | **TRUE** — `server/subscriptions.rs:641,677` |
| 113-18: fresh-id contract tripwired | **TRUE** — `v2_subscriptions_client.rs:316` |
| 113-20: `Limited`, not collect-then-check, at "all three" `collect()` sites | **TRUE for the three named**, **FALSE as a statement about the transport** — CR-01 |
| 113-20: `Content-Length` is an optimisation, never the authority | **TRUE** — `streamable_http.rs:528-553` |
| 113-19: fuzz seam feature-gated + retention invariant | **TRUE mechanically**, but the gate is not what the doc claims — WR-07 |

Answering the five questions posed in the review context directly:

1. **Are the bounds enforced on every path, including unnamed ones?** No.
   `client/subscriptions.rs:148` performs a fourth, completely uncapped whole-body
   read of a peer-controlled response — on the v2 `subscriptions/listen` path this
   phase built. `http.rs:347` does the same for `HttpTransport`'s POST response.
   Both are the exact defect class 113-20 exists to close (CR-01, CR-03).
2. **Is `Content-Length` load-bearing for a security decision?** No.
   `collect_body_within_cap` uses it only to refuse early and never to admit;
   `Limited` is the authority on every path. The one other `content-length` read
   (`streamable_http.rs:1179`) selects a parse branch and is guarded by a real
   `modified_body.is_empty()` check. This claim survives review intact.
3. **Does the `fuzzing` gate leak into a default build?** Not into `default`, not
   into `full`. But it *is* on in every `--all-features` build — which is what this
   repo's own CI runs — and the two enforcement tools the docs cite
   (`cargo public-api`, `cargo semver-checks`) exist nowhere in `Makefile` or
   `.github/`. The guarantee is a comment (WR-07).
4. **Is `prune_after_rejection`'s drop ordering a deadlock?** No. `entries` is
   released at `server/subscriptions.rs:673` before every `prune_after_rejection`
   call; `register` never nests `per_principal` inside `entries` (the
   `per_principal` guard is scoped to lines 627-637 and dropped before the
   `entries.write()` at 657); `ListenGuard::drop` takes the two strictly
   sequentially. There is **no lock nesting anywhere in `ListenRegistry`**, so no
   cycle is constructible. This one is clean.
5. **Does any error-code mapping make a non-retryable condition look retryable?**
   Yes, and it loses information doing it. `RATE_LIMITED` now covers three distinct
   conditions and is *also* already emitted for genuine rate limiting
   (`shared/middleware.rs:1056`), leaving a prose substring as the only
   discriminator. A client that follows the doc's own advice — "back off and retry"
   — with a sticky id against a genuinely live incumbent retries forever (WR-02).

Beyond the four plans' scope, the most serious finding is CR-02:
`take_utf8_prefix`, the incremental UTF-8 decoder that runs **before** every one of
this phase's bounds, is quadratic on invalid bytes. Measured on this machine: 400 KiB
of `0xFF` costs **1.17 seconds of CPU**, and a peer that terminates each garbage frame
with a newline keeps the parser's buffer drained so the overflow latch never fires and
the loop never ends. Phase 113 bounded memory and left an unbounded CPU channel open
on the same untrusted input.

---

## Critical Issues

### CR-01: `subscriptions/listen` rejection path collects an unbounded peer-controlled body

**Severity:** BLOCKER
**File:** `src/client/subscriptions.rs:147-166` (reached from `src/client/subscriptions.rs:129-131`)

**Issue:**

```rust
async fn rejection_error(status: hyper::StatusCode, body: hyper::body::Incoming) -> Error {
    let collected = match body.collect().await {          // <-- NO CAP
        Ok(collected) => collected.to_bytes(),
        ...
    };
    ...
    truncate(&String::from_utf8_lossy(&collected))        // <-- second full copy
}
```

This is a **fourth** whole-body read on `StreamableHttpTransport`, reached from
`EventStreamTransport::open_event_stream` whenever the response's `Content-Type` is
anything other than `text/event-stream`. It is not covered by
`DEFAULT_MAX_COLLECTED_BODY_BYTES`, and it directly falsifies that constant's own
rustdoc:

> "Every one of this transport's response reads is a whole-body read — the POST
> response, the GET SSE stream and the v2 structured-error envelope — and the peer
> chooses how many bytes it sends." (`streamable_http.rs:286-288`)

A hostile or merely broken server needs only to answer the `subscriptions/listen`
POST with `Content-Type: application/json` and a chunked body of arbitrary length.
The client allocates all of it, then `String::from_utf8_lossy` allocates up to a
second full copy (up to 3× for non-ASCII), then `truncate` walks it with
`chars().count()` — all *before* the 200-char echo bound applies. `post_streaming`
hands the body back deliberately unread, so nothing upstream capped it either.

Secondary: `truncate` (`subscriptions.rs:308-315`) bounds by **characters**, not
bytes, so even the intended echo bound is up to 4× `MAX_ECHOED_FRAME` in bytes.

Note the existing cap suite's own reasoning at `streamable_http.rs:2305-2307` —
"The two parser-feeding sites are SEPARATE `collect()` call sites, so each gets its
OWN over-cap test and its OWN negative control. A single shared test would pass with
one of them uncapped." That is precisely the discipline that should have found this
site, and it stopped at the module boundary.

**Fix:** route this read through the same enforcement the other three use. Both the
cap field and `collect_body_within_cap` are private to `shared::streamable_http`, so
this needs a `pub(crate)` seam:

```rust
// src/shared/streamable_http.rs
impl StreamableHttpTransport {
    /// The cap this transport enforces on ONE collected body.
    pub(crate) fn max_collected_body_bytes(&self) -> usize {
        self.max_collected_body_bytes
    }

    pub(crate) async fn collect_body_within_cap(   // was: private
        response: HyperResponse<hyper::body::Incoming>,
        max_bytes: usize,
    ) -> Result<Bytes> { /* unchanged */ }
}

// src/client/subscriptions.rs
#[async_trait]
impl EventStreamTransport for StreamableHttpTransport {
    async fn open_event_stream(&self, body: Vec<u8>) -> Result<SubscriptionFrameStream> {
        let response = self.post_streaming(body).await?;
        let is_event_stream = /* unchanged */;
        if !is_event_stream {
            return Err(rejection_error(response, self.max_collected_body_bytes()).await);
        }
        Ok(Box::pin(sse_payload_stream(response.into_body())))
    }
}

async fn rejection_error(
    response: hyper::Response<hyper::body::Incoming>,
    max_bytes: usize,
) -> Error {
    let status = response.status();
    let collected =
        match StreamableHttpTransport::collect_body_within_cap(response, max_bytes).await {
            Ok(bytes) => bytes,
            // An over-cap rejection body is not a JSON-RPC envelope; report the refusal.
            Err(e) => return e,
        };
    /* unchanged from here */
}
```

Add a test mirroring
`collected_body_cap::an_over_cap_v2_error_envelope_falls_back_to_the_status_error`
for this fourth site.

---

### CR-02: `take_utf8_prefix` is quadratic on invalid bytes — a sustained remote CPU DoS that no Phase-113 bound covers

**Severity:** BLOCKER
**File:** `src/shared/sse_parser.rs:164-189`
**Callers:** `src/shared/http.rs:270`, `src/client/subscriptions.rs:237`, `src/client/subscriptions.rs:680`

**Issue:** for every invalid byte the loop performs
`buffer.drain(..valid_up_to + invalid_len)`, an `O(remaining)` memmove. A run of `n`
invalid bytes therefore costs `O(n²)`. Measured with an `-O` build of the exact
function body:

```
n=  16384 bytes ->    3.09 ms
n=  65536 bytes ->   34.27 ms      (4×  input → 11×  time)
n= 262144 bytes ->  457.13 ms      (16× input → 148× time)
n= 409600 bytes -> 1171.05 ms      (25× input → 379× time)
```

This runs **before** every bound this phase added: `take_utf8_prefix` is fed the raw
chunk, and only its *output* reaches `SseParser::feed`. Neither
`MAX_LISTEN_LINE_BYTES` (256 KiB), `DEFAULT_HTTP_SSE_BUFFERED_BYTES` (16 MiB) nor
`DEFAULT_MAX_COLLECTED_BODY_BYTES` (16 MiB) constrains it.

It is also not self-limiting. A peer that appends a single `\n` to each garbage frame
keeps the parser's line buffer drained, so `overflowed()` never latches and the reader
loop never breaks:

* **`HttpTransport::connect_sse`** — frame = `0xFF × 400 KiB` + `\n` → 1.2 MiB of
  U+FFFD, well under the 16 MiB ceiling → `drain_complete_lines` consumes the line
  (`process_line` finds no `:`, treats the whole 1.2 MiB as an unknown field, ignores
  it) → buffer empty → no overflow → repeat. **≈1.17 CPU-seconds per 400 KiB
  received, indefinitely** (≈3 CPU-seconds per MiB of attacker traffic).
* **`client/subscriptions.rs::read_next_frame`** — frame = `0xFF × 85 KiB` + `\n` →
  ≈255 KiB of U+FFFD, just under the 256 KiB listen bound → same loop, ≈50 ms per
  frame, indefinitely.

No existing test can see this. `an_oversized_complete_line_is_refused_not_emitted`,
`a_newline_carrying_flood_cannot_grow_the_event_past_the_bound` and
`the_configured_ceiling_admits_up_to_and_including_itself` all feed valid ASCII. The
proptests and the fuzz target *do* generate invalid bytes but assert only "does not
panic" and "retention ≤ bound" — never wall time, and the fuzz campaign is run
without `-timeout`.

**Fix:** decode in a single pass with a cursor and drain exactly once, so the drain
cost is `O(n)` total instead of `O(n²)`:

```rust
pub(crate) fn take_utf8_prefix(buffer: &mut Vec<u8>) -> String {
    let mut text = String::new();
    let mut consumed = 0usize;
    loop {
        let rest = &buffer[consumed..];
        match std::str::from_utf8(rest) {
            Ok(valid) => {
                text.push_str(valid);
                buffer.clear();
                return text;
            },
            Err(error) => {
                let valid_up_to = error.valid_up_to();
                if let Ok(valid) = std::str::from_utf8(&rest[..valid_up_to]) {
                    text.push_str(valid);
                }
                let Some(invalid_len) = error.error_len() else {
                    // Incomplete tail: keep exactly those bytes, drain ONCE.
                    buffer.drain(..consumed + valid_up_to);
                    return text;
                };
                text.push('\u{FFFD}');
                consumed += valid_up_to + invalid_len;
            },
        }
    }
}
```

Add a deterministic regression that pins the *shape* rather than the clock
(`256 KiB` of `0xFF` yields `262144` replacement chars and leaves `buffer.is_empty()`),
and run the existing fuzz target with `-timeout=1` so an algorithmic-complexity
regression produces an artifact instead of a silent pass. Separately, consider
capping the raw chunk size in both incremental feeders before `take_utf8_prefix`, so
even the single-pass cost per chunk is bounded.

---

### CR-03: `HttpTransport::send_request` collects the response body with no cap

**Severity:** BLOCKER
**File:** `src/shared/http.rs:346-356`

**Issue:**

```rust
let body_bytes = response
    .collect()                                  // <-- NO CAP
    .await
    .map_err(...)?
    .to_bytes();
let response_msg = crate::shared::stdio::StdioTransport::parse_message(&body_bytes)?;
```

Plan 113-17 hardened this exact file's *SSE reader* against unbounded peer-driven
retention and added `DEFAULT_HTTP_SSE_BUFFERED_BYTES` with a long rustdoc reasoning
about which quantity is bounded where — while the sibling whole-body read three
methods below stayed uncapped. Read together, the two new constants' docs
(`http.rs:84-107` and `streamable_http.rs:280-325`, which explicitly instruct the
reader "do not 'unify' them") give the impression that the pair of HTTP transports is
fully accounted for. `HttpTransport`'s request/response path is not accounted for at
all: a server answering a POST with a multi-gigabyte body OOMs the client, and then
`parse_message` deserializes whatever survived.

Same threat model as 113-20's HIGH-3/T-113-84 — peer-chosen byte count, one-shot
collection into memory — with no mitigation. `response.status() != StatusCode::OK` is
checked first (line 337), which narrows this to 200 responses and not meaningfully.

**Fix:** mirror `collect_body_within_cap` using the private-field pattern
`sse_buffered_bytes` already established in this struct (semver-invisible, per the
same `<config_surface_decision>` reasoning):

```rust
pub struct HttpTransport {
    ...
    sse_buffered_bytes: usize,
    /// Cap on ONE fully-collected response body, defaulted from
    /// `DEFAULT_HTTP_COLLECTED_BODY_BYTES` and overridable through
    /// `Self::with_max_collected_body_bytes`.
    max_collected_body_bytes: usize,
}

// in send_request, replacing the bare collect:
use http_body_util::{LengthLimitError, Limited};
let body_bytes = match Limited::new(response.into_body(), self.max_collected_body_bytes)
    .collect()
    .await
{
    Ok(collected) => collected.to_bytes(),
    Err(e) if e.is::<LengthLimitError>() => {
        return Err(crate::error::Error::Transport(
            crate::error::TransportError::InvalidMessage(format!(
                "response body exceeds this transport's {}-byte collected-body cap",
                self.max_collected_body_bytes
            )),
        ))
    },
    Err(e) => {
        return Err(crate::error::Error::Transport(
            crate::error::TransportError::InvalidMessage(e.to_string()),
        ))
    },
};
```

---

## Warnings

### WR-01: the collected-body cap runs *before* response middleware, so `feed_complete_body`'s precondition is not actually established

**Severity:** WARNING
**File:** `src/shared/streamable_http.rs:660-675` and `1192-1231`, feeding `684` and `1335`

**Issue:** `collect_body_within_cap` produces `body_bytes` (≤ cap), then
`apply_response_middleware` returns a user-supplied `Vec<u8>` (`modified_body`) of
**arbitrary** size — and it is `modified_body`, not `body_bytes`, that reaches
`sse_parser.feed_complete_body(&body)`. A body-transforming response middleware (a
gzip/deflate decoder is the obvious one) makes the parser's documented precondition
false:

> "Since plan 113-20 that cap is an established fact, not an obligation."
> (`sse_parser.rs:432`)

It becomes an obligation again the moment any middleware touches the body, and the
classic instance is a compression bomb: 16 MiB of gzip expands past the cap by orders
of magnitude, straight into `feed_complete_body`, which by design performs no check.

**Fix:** re-check after the chain, or state the boundary honestly. Cheap fix at both
sites:

```rust
let modified_body = /* … as today … */;
if modified_body.len() > self.max_collected_body_bytes {
    return Err(collected_body_over_cap(
        self.max_collected_body_bytes,
        Some(modified_body.len()),
    ));
}
```
and amend `feed_complete_body`'s "Precondition — SATISFIED by both call sites" to say
the cap is enforced on both sides of the middleware chain.

---

### WR-02: `RATE_LIMITED` now means three different things, and the only discriminator is a prose substring

**Severity:** WARNING
**File:** `src/server/subscriptions.rs:356-437`; consequence documented at `src/client/mod.rs:3915-3918`

**Issue:** 113-18 collapsed `PerPrincipalLimit`, `GlobalLimit` and
`DuplicateSubscriptionId` onto `-32005`. That code is *also* emitted for genuine rate
limiting (`src/shared/middleware.rs:1056` → `Error::RateLimited` →
`ErrorCode::RATE_LIMITED`, `src/error/mod.rs:290`), and `error_codes.rs:101`
documents it as "Rate limited — the client exceeded a rate limit." Three
consequences, all acknowledged in the code comments and none mitigated:

1. A client cannot programmatically distinguish "your subscription id is taken"
   (retrying with the **same id** can never succeed while the incumbent lives) from
   "you're at your stream cap" (close one, then retry) from "you're being rate
   limited" (back off). The code was chosen *because* it carries retry semantics, and
   it now carries three incompatible retry strategies.
2. The stated discriminator is `message().contains("too many concurrent")`, asserted
   in `server/subscriptions.rs:1740`, `tests/v2_subscriptions.rs:745,803` and
   `tests/v2_subscriptions_client.rs:431`. Load-bearing prose is brittle: any
   rewording or localisation silently breaks two suites and any third-party client
   that copied the pattern. `ListenRejection::code`'s own rustdoc calls this out
   ("the MESSAGE is now the ONLY discriminator") and then ships it anyway.
3. `src/client/mod.rs:3915` tells third-party clients that "backing off and retrying
   is the correct response". A client that does so with a sticky id against a live
   incumbent retries forever, and no `Retry-After` is emitted on this path.

**Fix:** carry the discriminator in machine-readable `error.data` rather than prose —
`listen_rejection_response` already has a `data` parameter
(`streamable_http_server.rs:735-756`):

```rust
// server/subscriptions.rs
pub(crate) fn data(self) -> serde_json::Value {
    serde_json::json!({
        "reason": match self {
            Self::PerPrincipalLimit       => "per_principal_limit",
            Self::GlobalLimit             => "global_limit",
            Self::DuplicateSubscriptionId => "duplicate_subscription_id",
        },
        // A duplicate NEVER clears for the same id while the incumbent lives.
        "retryWithSameId": !matches!(self, Self::DuplicateSubscriptionId),
    })
}
```
Then reorient the three test suites onto `error.data.reason` instead of the substring.

---

### WR-03: server-side docs still describe the pre-113-18 mapping and contradict the shipped code

**Severity:** WARNING
**File:** `src/server/streamable_http_server.rs:2849-2852` and `2955-2957`

**Issue:** 113-18 changed the duplicate refusal but left the call site documenting
the old behaviour:

```
/// 5. a duplicate LIVE `(principal, subscriptionId)` -> `-32600` at HTTP 400.
```

and, immediately above the code that now proves the opposite:

```rust
// PER-VARIANT code, owned by the rejection itself: a capacity
// refusal and a duplicate subscription id are different conditions
// and must not share one status.
```

Both statements are now false — `ListenRejection::code()`
(`server/subscriptions.rs:431-437`) returns `RATE_LIMITED` for all three, which
`v2_status_for_code` (`streamable_http_server.rs:690-702`) maps to HTTP 200. A
maintainer reading the call site will draw exactly the wrong conclusion about the wire
contract, which is how the pre-113-18 mapping got shipped in the first place.

**Fix:** update both to the current mapping (`-32005` at HTTP 200 for all three) and
cross-reference `ListenRejection::code`'s "Do not add `RATE_LIMITED` to that arm."

---

### WR-04: `SseParser::feed` is public API that now silently discards valid data, with no error channel and no public escape

**Severity:** WARNING
**File:** `src/shared/sse_parser.rs:365-419` and `456-458`; export at `src/shared/mod.rs:29`

**Issue:** `pub mod sse_parser` + `pub fn feed` means downstream crates call this
directly. Before 113-17 the `!data.contains('\n')` escape let a *complete* over-bound
body through; now `feed` returns `Vec::new()` for it. The signature cannot express
failure, so the only way a caller learns their data was dropped is by remembering to
poll `overflowed()`. `feed_complete_body`, the entry point that would serve such a
caller, is `pub(crate)` by deliberate design ("a public unbounded parser entry point
is an attractive nuisance").

This is silent data loss on stable public API. There is a workaround
(`SseParser::with_max_buffer_size(usize::MAX)`), but nothing points a caller at it at
the moment of loss, and no release note exists (see WR-12).

**Fix:** make the loss loud at the source, once per parser:

```rust
if self.buffered_bytes().saturating_add(data.len()) > self.max_buffer_size {
    if !self.overflowed {
        tracing::warn!(
            bound = self.max_buffer_size,
            held = self.buffered_bytes(),
            chunk = data.len(),
            "SseParser discarded in-flight SSE bytes past its bound; poll \
             `overflowed()` to detect this, or raise the bound with \
             `SseParser::with_max_buffer_size`"
        );
    }
    self.overflowed = true;
    ...
}
```
and consider a bounded *public* complete-body entry point so external callers are not
pushed toward `usize::MAX`:

```rust
/// Parse a COMPLETE body the caller has already capped at `cap` bytes.
///
/// # Errors
/// Returns `Err` when `body.len() > cap`, so the refusal is impossible to miss.
pub fn feed_complete_body_within(&mut self, body: &str, cap: usize)
    -> crate::error::Result<Vec<SseEvent>>;
```

---

### WR-05: `feed`'s bound conflates *chunk size* with *retained state*, and 256 KiB sits below hyper's read-buffer ceiling

**Severity:** WARNING
**File:** `src/client/subscriptions.rs:71-87` (`MAX_LISTEN_LINE_BYTES`), enforced at `src/shared/sse_parser.rs:391`

**Issue:** the pre-check refuses a chunk whose *size* exceeds the bound even when the
chunk contains nothing but complete events and would leave ~0 bytes retained. The
comment at `sse_parser.rs:381-390` accepts this ("behaviour depends partly on how the
transport frames its reads") but never connects it to the numbers actually in play:
hyper's HTTP/1 read buffer grows to roughly 400 KiB, i.e. **larger than the 256 KiB
`MAX_LISTEN_LINE_BYTES`**. A healthy server that flushes a burst of small
notifications in one write can therefore produce a single body frame this client
refuses whole, latching `overflowed()` and ending the stream with a protocol error —
from entirely correct server behaviour, on the path 113-19's fuzz campaign is meant to
be defending.

Nothing in the suite covers "one chunk, many complete small events, total over the
bound"; every bound test feeds either one oversized line or repeated unterminated
`data:` lines.

**Fix:** raise `MAX_LISTEN_LINE_BYTES` above the transport's maximum achievable frame
size, or restrict the pre-check to chunks that contain no event terminator (`"\n\n"`)
and let the already-correct post-drain residual check at `sse_parser.rs:412` carry the
rest. Either way, pin the decision:

```rust
#[test]
fn a_chunk_of_many_small_complete_events_is_refused_on_its_total() {
    let mut parser = SseParser::with_max_buffer_size(64);
    let chunk: String = std::iter::repeat("data: x\n\n").take(20).collect(); // 180 bytes
    let events = parser.feed(&chunk);
    // Document whichever answer is intended — today it is empty + overflowed.
    assert!(events.is_empty());
    assert!(parser.overflowed());
}
```

---

### WR-06: the cap is per-clone on a `Clone` transport whose every other setting is shared, and `0` is accepted

**Severity:** WARNING
**File:** `src/shared/streamable_http.rs:359-401` and `508-512`

**Issue:** `StreamableHttpTransport` derives `Clone`, and every other piece of state
(`config`, `protocol_version`, `v2_mode`, `abort_handle`, `last_event_id`, `receiver`)
is behind `Arc`, i.e. **shared** across clones. `max_collected_body_bytes` is a bare
`usize`, i.e. **copied**:

```rust
let t = StreamableHttpTransport::new(cfg);
let a = t.clone();                                    // cap = 16 MiB
let b = t.with_max_collected_body_bytes(64 * 1024 * 1024);
// `a` and `b` share the session, the protocol version and the message channel,
// but silently disagree about the cap.
```

Nothing documents the asymmetry, and `with_max_collected_body_bytes` takes `self` by
value, making it easy to apply after a clone has escaped. The same shape exists for
`HttpTransport::with_sse_buffered_bytes`, safe today only because `HttpTransport` is
not `Clone` — a fact nothing pins.

Separately, neither builder validates its argument.
`with_max_collected_body_bytes(0)` makes `Limited::new(body, 0)` reject every
non-empty response with "response body delivered more than the cap (Content-Length
absent or understated)" — a message that reads like a peer fault for what is a local
misconfiguration.

**Fix:** make the field `Arc<AtomicUsize>` so it shares clone semantics with the rest
of the struct (`load(Ordering::Relaxed)` at the three read sites), or document the
copy semantics explicitly on both the field and the builder. And refuse the degenerate
value:

```rust
#[must_use]
pub fn with_max_collected_body_bytes(mut self, max_collected_body_bytes: usize) -> Self {
    // A zero cap refuses every non-empty response; it is never what a caller means.
    self.max_collected_body_bytes = max_collected_body_bytes.max(1);
    self
}
```

---

### WR-07: the `fuzzing` gate does not do what its rustdoc claims, and the tools it cites are not in this repo

**Severity:** WARNING
**File:** `src/client/subscriptions.rs:584-663` (esp. `595-612`, `660-663`), `src/shared/streamable_http.rs:390-399`, `src/shared/http.rs:65-70`

**Issue:** three related, checkable claims:

1. `subscriptions.rs:598-600` — "absent from BOTH `default` and `full`, so
   `cargo public-api` never sees it on the shipped surface." `cargo public-api`
   appears nowhere in `Makefile` or `.github/`. Meanwhile this repo's CI runs
   `cargo build --all-features` (`ci.yml:90`), `cargo test --all-features`
   (`ci.yml:93`), `cargo check --all-features` (`ci.yml:304`) and
   `cargo doc --all-features` (`Makefile:409`) — all of which enable `fuzzing`. In
   those builds `pmcp::client::subscriptions::decode_listen_chunks_for_fuzz` **is** a
   `pub` item, and any downstream crate on an `--all-features` CI matrix entry can
   call a function whose own docs enumerate three properties that "would be defects in
   stable API" (unvalidated `max_buffer_size` accepting `0`, errors flattened to
   `String`, terminal frames silently dropped).
2. `streamable_http.rs:394-398` and `http.rs:66-70` — "fails `cargo semver-checks`'s
   `constructible_struct_adds_field` … Measured, not assumed." `cargo semver-checks`
   also appears nowhere in `Makefile` or `.github/`. The design conclusion may well be
   right; the enforcement it is justified by does not run, so nothing stops the next
   contributor from adding a `pub` field to `HttpConfig` and reintroducing exactly the
   break the design avoided.
3. Nothing structurally prevents a future edit from folding `fuzzing` into `full`.

**Fix:** make the invariants executable rather than asserted.

```rust
// src/lib.rs
#[cfg(all(feature = "fuzzing", feature = "full"))]
compile_error!(
    "`fuzzing` is an internal test seam and must never be composed with `full`; \
     see the fuzz-support section of src/client/subscriptions.rs"
);
```
and add both tools to the quality gate (e.g. `cargo semver-checks check-release` and
`cargo public-api --features full --deny changed` in `ci.yml`) so the rustdoc claims
are enforced. If the tools are deliberately not adopted, soften the rustdoc to what is
actually true: "not in `default` or `full`; an `--all-features` build does expose it."

---

### WR-08: a local client-side resource trip is reported as `INVALID_REQUEST` (-32600)

**Severity:** WARNING
**File:** `src/client/subscriptions.rs:292-305`

**Issue:** `listen_overflow` builds `Error::protocol(ErrorCode::INVALID_REQUEST, …)`
for a condition that is entirely local — *this client's* parser hit *its own*
configured bound. `-32600` means "the Request object is not valid"; the request was
fine, the response was too big. An application branching on
`Error::Protocol { code, .. }` will attribute a local capacity trip to a malformed
request it authored. This is the same misattribution class 113-18 just corrected on
the server side (`server/subscriptions.rs:411-419`), left uncorrected on the client
side in the same phase.

**Fix:** use the transport family, which is what the condition actually is:

```rust
fn listen_overflow(parser: &SseParser) -> Option<Error> {
    if !parser.overflowed() {
        return None;
    }
    Some(Error::Transport(TransportError::Request(format!(
        "a subscriptions/listen chunk pushed the buffered stream state past this \
         client's {}-byte parser bound; the buffered bytes were discarded and the \
         stream was ended",
        parser.max_buffer_size()
    ))))
}
```
and update `a_line_past_the_bound_latches_the_parser_and_ends_the_stream`
(`subscriptions.rs:1059-1064`), which currently asserts the `-32600` shape.

---

### WR-09: the bound's public names all say "line"/"buffer" and no longer describe what they bound

**Severity:** WARNING
**File:** `src/client/subscriptions.rs:71-87`, `src/shared/sse_parser.rs:133`, `288-305`, `684-694`

**Issue:** the code concedes the problem rather than fixing it:

> "Named for the line buffer it originally bounded; since 113-17 it bounds BOTH of the
> parser's accumulators … It is NOT a per-line limit, and no message derived from it
> may say it is." (`subscriptions.rs:73-76`)

`MAX_LISTEN_LINE_BYTES`, `SseParser::max_buffer_size()` (**public**),
`SseConfig::max_buffer_size` (**public field**) and `SseParser::with_max_buffer_size`
(**public**) all now bound `buffer + current_event.data + chunk`. A prohibition
enforced only by a comment is the kind of rule that decays across refactors, and for
the three public items the misleading name is the *first* thing a downstream reader
sees — well before the rustdoc that corrects it.

**Fix:** rename the private constant now (`MAX_LISTEN_IN_FLIGHT_BYTES`), and
deprecate-and-alias the public ones so the rename is additive:

```rust
impl SseParser {
    /// The in-flight bound THIS parser was built with, in bytes.
    #[must_use]
    pub fn max_in_flight_bytes(&self) -> usize { self.max_buffer_size }

    #[deprecated(
        since = "2.18.0",
        note = "renamed to `max_in_flight_bytes`: this bounds the line buffer PLUS \
                the pending event, not a buffer"
    )]
    #[must_use]
    pub fn max_buffer_size(&self) -> usize { self.max_in_flight_bytes() }
}
```

---

### WR-10: `open_event_stream` ignores the HTTP status whenever the content type is `text/event-stream`

**Severity:** WARNING
**File:** `src/client/subscriptions.rs:120-133`

**Issue:** `status` is read at line 122 and used only on the rejection branch. A
`4xx`/`5xx` response that happens to carry `Content-Type: text/event-stream` — a
misconfigured reverse proxy is the realistic source — is treated as a served stream.
The failure then surfaces from `SubscriptionStream::open` as "subscriptions/listen
stream ended before the mandatory acknowledgement", a message that names the wrong
cause and discards the status. That is precisely the information loss this module goes
out of its way to prevent elsewhere ("so 'this server does not do subscriptions' is
distinguishable from a transport fault", `subscriptions.rs:17-18`).

**Fix:**

```rust
if !status.is_success() || !is_event_stream {
    return Err(rejection_error(response, self.max_collected_body_bytes()).await);
}
```
(the `rejection_error` fallback message already embeds the status). Compose this with
the CR-01 signature change.

---

### WR-11: fuzz invariant 2 self-disables on any input containing a backslash

**Severity:** WARNING
**File:** `fuzz/fuzz_targets/subscription_listen_frames.rs:118-124`

**Issue:**

```rust
if outcomes.iter().any(std::result::Result::is_ok) && !text.contains('\\') {
    assert!(text.contains(SUBSCRIPTION_ID), ...);
}
```

The `\u`-escape reasoning behind the guard is correct, but the guard is far broader
than the hazard: a single `\` **anywhere** in the input disables the cross-delivery
invariant for that whole input. libFuzzer produces a backslash within the first
handful of runs and then keeps it in the corpus, because `\` appears in every JSON
string escape the target must explore to reach the delivery path at all. As the corpus
matures toward JSON-shaped inputs — the inputs most likely to actually deliver a
notification — the fraction of runs where invariant 2 is live trends toward zero. That
is structurally the same failure mode as the latch tautology this campaign was just
fixed to remove.

**Fix:** narrow the guard to the actual hazard:

```rust
// Only a `\u` escape can spell the id indirectly; no other escape can.
if outcomes.iter().any(std::result::Result::is_ok) && !text.contains("\\u") {
    assert!(text.contains(SUBSCRIPTION_ID), "...");
}
```
Better still, remove the textual precondition entirely by having
`decode_listen_chunks_for_fuzz` return the observed `subscriptionId` alongside each
outcome, so the invariant becomes structural (`observed == SUBSCRIPTION_ID`) rather
than a substring search over the raw bytes.

---

### WR-12: three documented behaviour changes ship with no CHANGELOG entry

**Severity:** WARNING
**File:** `CHANGELOG.md` (absent), vs `src/shared/sse_parser.rs:334-338`, `src/shared/http.rs:86-107`, `src/shared/streamable_http.rs:316-324`

**Issue:** all three new bounds carry an explicit "What breaks at this boundary"
section in their own rustdoc:

* `SseParser::feed` now discards over-bound chunks that previously parsed — public
  API, silently (WR-04);
* `HttpTransport::connect_sse` now **ends the reader task** on a payload over 16 MiB
  that previously accumulated and was delivered (`http.rs:89-93`);
* `StreamableHttpTransport` now **fails** a response over 16 MiB that previously
  succeeded (`streamable_http.rs:318-322`) — and the doc itself points out that a
  12 MiB base64 `image`/`audio` payload does not fit under the default, an arithmetic
  fact already pinned as a test
  (`base64_expansion_puts_a_12_to_16_binary_over_the_ceiling`, `http.rs:655`).

`grep -n "SseParser\|max_buffer_size\|collected_body\|collected-body\|sse_buffered" CHANGELOG.md`
returns nothing. Users upgrading into these caps get a silent regression (the first)
or a loud but unexplained one (the other two) on media-carrying deployments, with no
release note pointing at the escape hatches.

**Fix:** add a `### Changed` block under the unreleased heading naming all three
defaults and their three escape hatches
(`StreamableHttpTransport::with_max_collected_body_bytes`,
`HttpTransport::with_sse_buffered_bytes`, `SseParser::with_max_buffer_size`), and
reproduce the base64 arithmetic so operators can size their own ceilings.

---

_Reviewed: 2026-07-26T14:20:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
