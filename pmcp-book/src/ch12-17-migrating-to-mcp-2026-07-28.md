# Chapter 12.17: Migrating to MCP 2026-07-28 (v2)

> **Note on vocabulary**: throughout this chapter, **v1** means the MCP protocol
> revision `2025-11-25` and **v2** means `2026-07-28`. These are *protocol eras*,
> not crate versions. The crate is always written with its name attached — "pmcp
> 2.18" — so a bare "2.18" can never be mistaken for a protocol revision. The two
> numbering schemes are unrelated: pmcp 2.18 speaks both eras.

MCP 2026-07-28 is not a version bump you take by editing a number in
`Cargo.toml`. It is a different wire contract: no `initialize` handshake, no
session identifier, no SSE resumption, and a request that declares its own era in
`params._meta`. pmcp implements both eras in one crate, and the era decision is
made **per request** rather than per binary.

That design has a consequence which shapes this entire chapter: *"how do I opt
into v2" has three genuinely different answers*, depending on whether you own a
server, a client, or an agent. A server owner has one builder call to add. A
client owner has exactly one line to write. An agent owner already has it, because
`pmcp-agent` opted in on their behalf. Reading the wrong track will leave you
looking for a switch that does not exist in your role.

So: find your track below and stop there. The sections after the three tracks —
the Tasks pointer, the behaviour changes, and the sunset link — apply to
everyone.

After this chapter you should be able to state which era your deployment speaks,
opt a client into v2 explicitly, build the v1-severed proof configuration of the
server, configure the one security-relevant deployment variable that v2's
statelessness makes load-bearing, and find the consumer-visible behaviour changes
that carry no semver signal.

## The dual-version story

**One binary, both eras, negotiated per request.** You do not run a v1 fleet
beside a v2 fleet, and you do not pick an era at build time. The same
`StreamableHttpServer` process answers a `2025-11-25` client with sessions and a
`2026-07-28` client without them, concurrently, on the same port.

The era boundary sits at the *request*, not the connection and not the process. A
v2 request carries its era in `params._meta` along with the
`MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name` headers; a v1 request carries
none of those and instead completed an `initialize` handshake earlier. The server
reads which shape arrived and dispatches accordingly.

This is why the migration is incremental in the only way that matters
operationally: you can deploy a pmcp 2.18 server today and it serves both your
existing v1 clients and any v2 client that shows up tomorrow, with no flag day
and no dual deployment.

## For servers

**v2 is opt-in, and upgrading the crate is not the opt-in.** A pmcp server built
normally — the default feature set, or `full` — answers **v1 only**. The default
accept-list is v1-only by design: `default_accept_list()`
(`src/types/protocol/context.rs`) is exactly `SUPPORTED_PROTOCOL_VERSIONS`, which
deliberately excludes `2026-07-28`, so no server reaches the v2 era by accident.
That guard is intentional and is not going to be relaxed; the cost is that you
must ask for v2 explicitly.

The call is `with_supported_protocol_versions`, and the important part is that
you list your v1 version **alongside** v2 rather than replacing it — the era is
negotiated per request, so one accept-list carrying both makes one binary serve
both:

```rust,ignore
use pmcp::types::protocol::{LATEST_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28};
use pmcp::types::ProtocolVersion;

let server = Server::builder()
    .name("my-server")
    .version("1.0.0")
    .capabilities(ServerCapabilities::tools_only())
    .with_supported_protocol_versions([
        ProtocolVersion(LATEST_PROTOCOL_VERSION.to_string()),
        ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
    ])
    .tool("my_tool", MyTool)
    .build()?;
```

`examples/s47_v2_stateless_mrtr.rs` is the compiled version of exactly this, and
`s48_v2_mrtr_client.rs` is the client that exercises it.

⚠ **Do not pass v2 alone.** `.with_supported_protocol_versions([v2])` is accepted
and looks like it works: v1 `initialize` still succeeds, because v1 negotiation
falls back through the global `SUPPORTED_PROTOCOL_VERSIONS` table rather than
through your accept-list. But `server/discover` will then advertise
`supportedVersions: ["2026-07-28"]`, so what you advertise and what you serve
disagree, and nothing warns you.

There is also a lever in the *opposite* direction: opting **out** of v1.

### Opting out of v1 (the severance build)

```bash
cargo build -p pmcp --no-default-features --features full-v2
```

Two things about that command are easy to get backwards.

First, **`--no-default-features` alone proves nothing**, which is why the
`--features full-v2` half is not optional. `default = ["logging", "v1-compat"]`,
so stripping defaults also strips `logging` — and with the rest of the feature
list gone, `http` and `streamable-http` never compile either. A build like that
would "prove" v1 is severable by never compiling the HTTP transport at all.
`full-v2` is therefore a parallel *positive* list: exactly what `full` has,
except `v1-compat`. `Cargo.toml` states the reason at the definition site.

Second, **there is no `v2-only` feature and there deliberately never will be.**
Cargo features are additive and cannot be subtracted, so any crate anywhere in
the dependency graph enabling a negative feature would silently strip v1 for
every other consumer of that same build. An inverted feature was considered and
rejected for exactly that reason.

`v1-compat` is a dependency-free marker feature. SSE *framing and parsing* stay
compiled either way — v2 uses SSE too, for `subscriptions/listen` — so this is
not a "drop SSE" switch.

A `full-v2` build necessarily answers a v1 client differently: that is the
severance, not a defect. Exactly what it gates and exactly how a v1 client's
experience differs are enumerated normatively in the sunset policy — see
[The v1 sunset](#the-v1-sunset) below rather than reasoning from this paragraph.

### Statelessness is a per-request gate, not a build switch

The most common misreading of v2 is that "stateless" is a server mode you
configure. It is not. Run the shipped example:

```bash
cargo run --example s47_v2_stateless_mrtr --features full
```

That server is built with the **stateful default HTTP config**. It still mints
sessions for v1 clients. And yet no v2 response it emits carries a session id.
Nothing was switched off — the era gate is evaluated per request, and a v2
request simply never reaches the session bookkeeping. Drive it with the paired
client in another terminal:

```bash
cargo run --example s48_v2_mrtr_client --features full
```

The client's second round trip arrives as an independent HTTP request with a
different JSON-RPC id and no session, and the handler resumes exactly where it
left off.

### Lambda, and why v2 makes serverless coherent

The `pmcp-server-lambda` crate
(`crates/pmcp-server/pmcp-server-lambda/`) is the standard pattern for running
any pmcp server on AWS Lambda: it starts `StreamableHttpServer` on localhost in
the background and proxies Lambda HTTP events into it.

That pattern has always been in tension with v1's session model. A session id
means server-side state, and a horizontally scaled serverless deployment gives
you N instances with no shared memory and no affinity guarantee. v2's
sessionlessness removes the tension: every request is self-contained, so which
instance answers it stops mattering.

Stops mattering, that is, **for exactly one variable** — which is the next
section, and the single most important paragraph in this chapter for anyone
deploying v2 behind a load balancer.

### `PMCP_REQUEST_STATE_KEY` — the deployment decision that is a security decision

v2's multi-round-trip elicitation replaces the session with an opaque,
AEAD-sealed `requestState` continuation token, bound to the caller's principal,
the method, and a digest of the request's salient parameters. The client echoes
it verbatim on the follow-up request; it cannot read or forge it.

That token is sealed with a 32-byte key. When the key is unset, the server
generates a fresh per-process key and logs this at startup:

> **WARN**: PMCP_REQUEST_STATE_KEY is not set — generated a per-process
> requestState key. Multi-round-trip requests whose follow-up lands on a
> DIFFERENT instance behind a load balancer cannot be resumed and will be
> re-elicited. Set PMCP_REQUEST_STATE_KEY to the SAME 32-byte base64url (or hex)
> value on every instance to enable resumption.

Read that as a MUST, not a suggestion:

- **Multi-instance deployments MUST set `PMCP_REQUEST_STATE_KEY` to the same
  32-byte value on every instance.** N instances with N different keys means
  instance B cannot open instance A's token, so every load-balanced follow-up is
  re-elicited from scratch — the user is asked the same question again, and the
  in-flight work is discarded.
- **A malformed value fails the server build.** A silently degraded crypto key
  is worse than a refusal to start, so the server refuses to start.
- **Unset is fine for single-instance development** and nothing else. The
  `s47` example deliberately leaves it unset so you see the warning.

Generate the value with a CSPRNG, never by hand and never by copying one out of
documentation:

```bash
openssl rand -base64 32
```

Then inject it from your secrets manager. For rotation, `PMCP_REQUEST_STATE_KEY_PREVIOUS`
joins the **accepting** set only — it verifies tokens sealed with the old key
but never mints new ones. The rotation is therefore: set
`PMCP_REQUEST_STATE_KEY_PREVIOUS` to the outgoing value and
`PMCP_REQUEST_STATE_KEY` to the new one, roll every instance, then drop
`PMCP_REQUEST_STATE_KEY_PREVIOUS` once the old tokens have aged out.
`PMCP_REQUEST_STATE_TTL_SECS` overrides the continuation lifetime, which is what
"aged out" means.

If your key comes from a secrets manager rather than the process environment,
the programmatic form beats the environment entirely:
`Server::builder().with_request_state_key([u8; 32])`, paired with
`.with_request_state_ttl(..)`.

## For clients

The client opt-in is **explicit and per connection**. One call:

```rust,no_run
use pmcp::{ClientBuilder, StdioTransport};
use pmcp::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28};

# fn main() -> Result<(), pmcp::Error> {
let client = ClientBuilder::new(StdioTransport::new())
    .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))?
    .build();
# Ok(()) }
```

**With no such call, the client behaves exactly as it does today** — v1, full
`initialize` handshake, no v2 headers, no per-request `_meta`. Nothing about
upgrading the crate changes an existing client's era. The selection is pushed
into the transport exactly once at `ClientBuilder::build` time; a transport with
no wire representation for it (stdio, WebSocket) logs a `tracing::warn!` at build
time.

### There is no auto-probe, and that is deliberate

pmcp's `Client` never probes `server/discover` to *choose* an era. The lock is
carried in the source itself, at `src/client/mod.rs:1101` and `src/client/mod.rs:5153`,
in both cases as a comment telling a future contributor not to restore it.

`server_discover` exists — v2 has no `initialize`, so it is how a v2 client
learns what a server supports — but it is explicit, and populating capabilities
from a call *you* made is a different thing from probing to decide which protocol
to speak.

A silent probe would be worse than an explicit choice for a reason that outlives
the implementation: it makes your client's era depend on a server's response to a
speculative request, so the same client code speaks different protocols against
different peers, and a peer that changes its answer changes your wire format
without a deploy. Debugging that means reconstructing a negotiation you never
wrote down. An explicit `with_protocol_version` call is a fact in your source
tree.

### What changes on the wire

Selecting `PROTOCOL_VERSION_2026_07_28` switches the connection to the v2 wire
contract:

- **No `initialize` and no `notifications/initialized`.** v2 has no handshake.
  The first byte a v2 client sends can be a `tools/call`.
- **No `Mcp-Session-Id`, in either direction.**
- **No `Last-Event-Id` SSE resumption.** SSE itself is still used — a v2-only
  `subscriptions/listen` returns a long-lived `text/event-stream` — but the
  resumability cursor is v1 machinery and is not offered on v2.
- **Every request carries `params._meta`** with the reserved
  `io.modelcontextprotocol/*` keys.
- **Every request carries `MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name`**
  (the last empty for a name-less method).
- **`resources/subscribe` and `resources/unsubscribe` are removed** on the v2
  path; change notifications arrive through the subscription stream instead.

Three transport-layer error codes come with it, all mapping to **HTTP 400** as a
spec MUST:

| Code | Name | Meaning | `error.data` |
|---|---|---|---|
| `-32020` | `HEADER_MISMATCH` | A required v2 header is missing or malformed, or a header value disagrees with the corresponding value in the JSON-RPC body | — |
| `-32021` | `MISSING_REQUIRED_CLIENT_CAPABILITY` | Processing needs a client capability absent from `_meta.clientCapabilities` — e.g. a handler wanting `elicitation/create` from a client that never declared `elicitation` | `requiredCapabilities`, a `ClientCapabilities` **object** (`{"sampling": {}}`), never an array |
| `-32022` | `UNSUPPORTED_PROTOCOL_VERSION` | The requested version is not in the server's accept-list | `supported` (so you can pick a mutually supported version and retry) and `requested` |

These live in the spec-reserved `-32020..=-32099` range and are distinct from
pmcp's own implementation-defined codes. In particular `-32021` is **not**
`UNSUPPORTED_CAPABILITY` (`-32002`); that one is the long-standing pmcp code for
a different direction, and the two are not interchangeable.

## For agents

If you build agents with `pmcp-agent`, **you are already on v2 where the server
supports it.** Start from the CLI:

```bash
cargo pmcp agent new research-agent
cargo pmcp agent dev
```

Those two verbs are the complete set. The agent's MCP invoker prefers v2 and
falls back to v1: attempt one pins `2026-07-28` via
`ClientBuilder::with_protocol_version` and confirms with `server_discover`;
attempt two runs the byte-identical v1 `initialize` path.

The important design point is *where* that probe lives. It is in `pmcp-agent`,
not in `Client` — the host makes an explicit era choice and `server_discover`
only confirms it. That placement is what keeps the client-side no-auto-probe lock
intact while still giving agents a working fallback: the agent is a host with a
policy, and `Client` remains a transport that does what it is told.

For everything else about building agents as MCP clients, see
[Chapter 12.15: Agents as MCP Clients](ch12-15-agents-as-mcp-clients.md).

## Tasks on v2

MCP Tasks changes shape on v2: task lifecycle moves off the v1 task methods and
onto the extension surface, which pairs naturally with v2's stateless
request model — you poll a task store rather than hold a stream open.

**The wire values here are provisional.** The Tasks extension schema is still
`draft/` upstream, with no tagged release, so anything this SDK emits for v2
Tasks may change before the extension is final. Do not pin production behaviour
to it yet.

The full era delta lives in
[Chapter 12.7: MCP Tasks — Long-Running Operations](ch12-7-tasks.md).

## Behaviour changes & known limitations

`CHANGELOG.md` remains the per-**release** record: what changed between pmcp
2.18 and pmcp 2.19. This section is the per-**migration** view: the things a
consumer moving to v2 will observe, gathered from two sources that a release
changelog alone does not cover.

### Wire changes in pmcp 2.19.0

**Embedded resources now serialize as the spec's `EmbeddedResource`.** An
embedded resource inside `CallToolResult.content` or
`GetPromptResult.messages[].content` now emits the nested shape the schema
declares:

```json
{ "type": "resource", "resource": { "uri": "…", "mimeType": "…", "text": "…" } }
```

rather than the previous flat object with the payload hoisted to the top level.
This applies to **both eras** — it is a conformance fix, not an era-dependent
shape — and there is no opt-out, because the flat object matched no arm of the
schema's `TextResourceContents | BlobResourceContents` union and every other MCP
implementation rejected or misread it.

Consequences you have to act on:

- **Readers must read `content.resource.uri` instead of `content.uri`** (and
  `content.resource.text` / `content.resource.blob`). `content._meta` and
  `content.annotations` stay at content level and are unmoved.
- `ReadResourceResult.contents` **stays flat** — the spec declares that position
  flat and pmcp already emitted it correctly. It gains `blob` and nothing else.
- `Content::Resource` is now `#[non_exhaustive]`; construct it through
  `Content::resource_with_text` / `Content::resource_with_blob` and match it with
  a `..` rest pattern.
- `Content::resource(uri)` is deprecated — a URI-only value is a `ResourceLink`,
  not an `EmbeddedResource`.

The reader is tolerant in the other direction: it accepts both the nested and the
legacy flat shape, which also fixes a client-side defect where a spec-conformant
embedded resource from another SDK's server failed to parse with ``missing field
`uri` ``.

### Consumer-observable changes with no semver signal

The following changes are observable by a downstream consumer, but with one
named exception **every symbol involved is private**, so `cargo semver-checks`
reports no semver update required. (The exception is entry 27's additive
`SharedSender` trait and its defaulted `Transport::shared_sender` accessor —
additive, so still no semver *update* required, but a public surface rather than
a private one.) This section is the only place a consumer learns of these
changes. Each is recorded in the project's broken-windows ledger
(`.planning/WINDOWS.md`) under the entry id given, and each is marked there with
the `[CONSUMER-OBSERVABLE]` sentinel.

A mechanical gate (`tests/windows_disclosure_tripwire.rs`) fails the build if a
marked ledger entry has **no** paragraph here. Read what that guarantees
carefully, because it is narrower than it looks: **the tripwire guarantees these
paragraphs EXIST; it cannot guarantee they are still TRUE.** It compares id sets,
not prose, so a paragraph that went stale when the code beneath it changed stays
green forever. That is exactly how two of the paragraphs below came to describe a
client that no longer shipped, and it is why **a behaviour change is not complete
until the paragraph for its entry has been re-read**, not merely until the
tripwire passes.

- **WINDOWS.md entry 12 — `Transport::receive()`'s terminal reason is now
  STICKY.** Previously a terminal stream reason was pushed once onto the response
  channel, so it reached exactly one caller and every later caller blocked
  forever. Now the reason is latched on the transport and returned by every
  subsequent `receive()` call, immediately. **A consumer that loops on
  `receive()` and merely logs errors will now spin rather than hang.** The
  contract is to *stop* on a terminal error; `Transport::receive`'s rustdoc says
  so under the heading "The terminal reason is STICKY — stop on it, do not loop".
  Sticky is no longer *permanent*: a successful SSE re-open clears the latch.

- **WINDOWS.md entry 13 — response frames are ROUTED by id, not discarded.**
  (Rewritten in place; the text that stood here described a discard-and-loop
  client that pmcp stopped shipping — see entry 24.) A response frame is now
  delivered to whichever in-flight request registered its id, and the
  registration happens *before* the frame goes on the wire. So a frame that
  arrives out of order, or belongs to a **different live caller** on the same
  `Client`, is no longer discarded and no longer costs that caller its answer —
  which is what the earlier discard-and-loop behaviour did, and what a
  concurrent-tool-calling agent hit routinely.

  A frame that **nobody** awaits is dropped, and only such a frame counts against
  anything. Of those, one that is the **late answer of a call that already stopped
  waiting** — your own cancelled or timed-out request — is absorbed silently and
  charged to no one, so it cannot make the *next* call fail. A frame addressed to
  an id this client never minted still counts, and still fails the call that is
  currently waiting; see entry 20 for the bounds and the blast radius.

  A **lenient server that re-types the id** — a JSON string where the client sent
  a number — still lands in that last category, because `RequestId` equality is
  typed and structural and JSON-RPC 2.0 requires the response id to be the same
  *value* as the request's.

- **WINDOWS.md entry 19 — three timing/text changes in the transport.**
  (1) `receive()` no longer answers immediately with a latched terminal reason
  while a streaming POST response is in flight; it waits. A consumer that relied
  on the immediate (stale) answer now sees a wait. (2) A successful SSE re-open
  clears a latch that was previously permanent, so a peer can cause more than one
  latched reason over a transport's life where before it could cause exactly one.
  (3) The rendered **text** of every terminal reason gains a stream-name prefix
  ("the GET session stream ended: …" / "this call's own POST response stream
  ended: …"). Every message *body* is preserved verbatim, so substring matching
  still works — but a consumer matching on the whole string, or logging it, sees
  different text.

- **WINDOWS.md entry 20 — a bounded discard introduces a new failure mode.**
  A `dispatch_request` that has seen a mis-addressed frame now fails with a
  timeout, or with a named `TransportError::InvalidMessage` after a discard cap,
  where it previously waited forever. **The blast radius is wider than "wedged
  becomes bounded":** the deadline is armed on the *first* mismatch and stays
  armed for the life of that call, so a peer that emits one stray frame early and
  then answers correctly late previously succeeded and now fails. That is a new
  failure mode for a previously-working slow-but-lenient peer. Arming on the
  first mismatch is what makes the ceiling un-extendable by a peer emitting a
  steady drip of wrong ids, so it is the mechanism the design requires — but if
  your peer is slow and slightly lenient, you will observe it. **This warning is
  still live at the current release.** The ceiling was not removed by the routing
  work described in entry 24 — it was re-added under two renamed private
  constants — and it still arms on the first frame nobody awaits and still stays
  armed for the life of that call. What *was* removed is the chain: the arming
  frame can no longer be your own earlier call's leftover answer (entry 25).

  A separate sentence used to stand here, and has been **deleted rather than
  annotated**, because a false claim cannot be left readable as current beside its
  own correction. It promised that while one call was stuck, any other operation
  on the same `Client` would be held up by no more than a single 250 ms slice.
  **That was only ever true of the receive path.** On the send path a stalled peer — one that
  accepts your POST and never writes its response head — froze every other
  operation on the client, `close()` included, with nothing to bound it. That is
  fixed as of entry 27, and both paths now hold: one stalled call blocks only
  itself. Two carve-outs remain and are named in entry 27 — a **pooled**
  `StreamableHttpTransport`, and `Client::open_event_stream`
  (`subscriptions/listen`).

- **WINDOWS.md entry 23 — an open, deferred defect in the POST-response SSE
  reader.** With two or more concurrent SSE-answered POST responses live on one
  transport, a caller whose own stream has already latched its own correct
  terminal reason cannot see that reason while an unrelated reader is still open,
  because a per-caller question is answered with a transport-wide count. The
  trigger is concurrency ≥ 2 SSE-answered POST responses **and** a terminal
  failure on one of them — two concurrent streaming tool calls suffice, which is
  ordinary for an agent doing parallel tool invocation; the rare half is the
  transport failure, not the concurrency. The consequence is a **delayed error**,
  bounded by the other reader's lifetime: no answer is lost, none is mis-delivered
  to the wrong caller, and nothing is starved. It is deferred rather than fixed
  because it lives exclusively on the POST-response SSE reader path — the v1
  surface — and the v2 stateless direction does not create concurrent POST
  readers at all, so the affected population shrinks rather than grows. It is
  **deferred, not closed and not waived**: no fix is scheduled and none is implied.

- **WINDOWS.md entry 24 — JSON-RPC responses are routed by id.** This is the
  largest behaviour change in the group and it shipped without its own record;
  the entry is retrospective. `Client::dispatch_request` was rewritten into a
  per-id router: each request registers an answer channel keyed by its id before
  its frame leaves the process, and each arriving response is handed to whoever
  registered that id. **What you gain:** concurrent calls on one `Client` no
  longer destroy each other's answers, and an out-of-order response is delivered
  rather than dropped. **What did not change, contrary to what an earlier version
  of entry 20 claimed:** frames addressed to ids this client never minted are
  still dropped and still counted, under a 10-second deadline and a 32-frame cap
  (both private constants). **What to do:** nothing, unless you were relying on
  the old behaviour of a mis-addressed frame failing an unrelated concurrent
  call — that no longer happens.

- **WINDOWS.md entry 25 — a call you stopped waiting for can no longer break
  every later call.** Previously, one abandoned call (cancelled, or failed at its
  ceiling) plus a peer whose honest round trip exceeds ten seconds failed **every
  subsequent call on that `Client`**, silently, for the life of the process: the
  dead call's own late answer was left on the queue with no owner, and the next
  call booked it as peer misbehaviour and armed its own deadline on frame one.
  Now that late answer is recognised as your own debris and absorbed once,
  charging nobody. **What to do:** if you worked around this with a client-per-
  request pattern or a periodic reconnect, you can stop. The bounds themselves
  are unchanged, and a peer that has simply **gone quiet** is still not bounded
  at all — pmcp has no default request timeout, deliberately, because a blanket
  deadline would break long-running `tools/call` handlers. Set your own deadline
  around a call if you need one.

- **WINDOWS.md entry 26 — concurrent `401`s on one transport now refresh the
  token once.** If you share **one** `StreamableHttpTransport` across concurrent
  tasks — the durable-agent shape — two overlapping `401`s used to call
  `AuthProvider::on_unauthorized()` twice and then reach `get_access_token()`
  twice, concurrently, against a cache the first had just emptied. With a
  **rotating refresh token** the identity provider accepts one and rejects the
  other, and the loser's purge destroys the token the winner had just cached, so
  the transport's auth fails permanently until you re-authenticate out of band.
  That sequence is now single-flighted from the purge through the retry's token
  fetch. A **cold-start fan-out** — N concurrent first requests against an empty
  cache, with no `401` involved — is single-flighted too, by a separate gate
  that engages only while the cache is cold, so a warm transport takes no lock
  per request. One limit remains: a second `401` on the retry is returned to you
  unchanged rather than triggering another refresh. Separately, two overlapping
  session-stream **restarts** can
  no longer leave an orphaned reader that `close()` cannot reach; a restart
  overlapping a concurrent `close()` is a different pairing, and there the
  reader is stopped by the transport's shutdown signal rather than by `close()`'s
  abort. ⚠ **One cost of that restart lock, which you can observe:** it is held
  across the session-stream `GET`'s response head, and pmcp has no request
  timeout on any path. A peer that **accepts** the `GET` and then never writes
  its response head therefore holds a transport-wide lock indefinitely, and
  every later session-stream restart on that transport — a resumption-cursor
  send, or a second handshake on a cloned transport — waits behind it with
  nothing to bound the wait. This is a known, accepted residual, not a
  regression: it is the duration cost of the lock, not a hole in the "exactly
  one reader" guarantee above. Closing it needs a bounded-request decision this
  release deliberately did not take: pmcp has **no default request timeout**,
  and activating the public `RequestOptions::timeout` field — which has never
  been read — was rejected because it would silently change behaviour for every
  existing caller who already sets it, with no version signal, and because a
  blanket deadline would break long-running `tools/call` handlers. If your peers
  can stall mid-response, impose your own deadline around the call.
  **What you must check on your side:** "exactly one vend" holds
  only if your `AuthProvider` **caches what
  `get_access_token` returns**. The trait does not require that, and pmcp cannot
  enforce it. Against a non-caching provider the two vends are merely serialised
  rather than simultaneous, which a rotating refresh token still rejects — so if
  you rotate refresh tokens, cache the vended token inside your provider and
  evict it in `on_unauthorized()`.

- **WINDOWS.md entry 27 — one stalled call no longer freezes the whole client,
  and two POSTs can now be in flight at once.** A peer that accepts your POST and
  never writes its response head used to hold the client's single transport guard
  for as long as it liked, blocking every other operation on that `Client` —
  a second call, a notification, a cancellation, even `close()` — with nothing to
  bound it. Client-side sends now take an owned send handle and release the guard
  before the round trip, so a stall blocks only its own call. **Three things to be
  aware of, not just the fix.** (1) The stalled call itself still waits
  indefinitely: no timeout was added, because a server that answers from a
  long-running handler writes its response head only when that handler finishes,
  and any finite bound would break calls that work today. (2) **Two POSTs can now
  be in flight on one transport simultaneously**, where pmcp previously imposed a
  total order, and **the client no longer orders outbound frames across separate
  POSTs.** HTTP never guaranteed such an order, but if you were depending on
  pmcp's incidental one, you no longer have it. (3) The fix does **not** reach a
  **pooled** transport (`PooledTransport`, by explicit decision — a pool picks a
  connection per send, so no single inner handle can speak for it) nor
  `Client::open_event_stream`, which still holds a guard across the
  `subscriptions/listen` POST's response head. Both are recorded as open
  residuals with an owner. The API change is **additive**: a new `SharedSender`
  trait and a `Transport::shared_sender()` accessor defaulted to `None`, so an
  external `Transport` implementation compiles and behaves exactly as before
  without touching a line.

## The v1 sunset

v1 support is not going away on a schedule, and this chapter does not set one.
The normative statement of what `v1-compat` gates, what removal is conditioned
on, and what a consumer has to do today lives in one place:

**→ [`docs/v1-sunset-policy.md`](https://github.com/paiml/rust-mcp-sdk/blob/main/docs/v1-sunset-policy.md)**

That document is the authority. If you need to know whether something you depend
on is severable, whether a date exists, or what is deliberately *not* severed,
read [the policy](https://github.com/paiml/rust-mcp-sdk/blob/main/docs/v1-sunset-policy.md)
rather than anything summarised here.

## What You Built

You can now:

- state which protocol era your server and clients speak, and why the answer is
  per request rather than per binary,
- opt a client into v2 explicitly with `with_protocol_version`, and explain why
  pmcp will not do it for you,
- build the v1-severed proof configuration with
  `cargo build -p pmcp --no-default-features --features full-v2`, and say what
  `--no-default-features` alone would fail to prove,
- deploy a multi-instance or Lambda v2 server with a shared, CSPRNG-generated
  `PMCP_REQUEST_STATE_KEY` and a working rotation story, and
- find every consumer-observable behaviour change that ships without a semver
  signal, by entry id.

For the agent-side story, see [Chapter 12.15: Agents as MCP Clients](ch12-15-agents-as-mcp-clients.md);
for the Tasks era delta, [Chapter 12.7: MCP Tasks — Long-Running Operations](ch12-7-tasks.md);
for the transports these eras run over, [Chapter 10: Transport Layers](ch10-transports.md)
and [Streamable HTTP](ch10-03-streamable-http.md).
