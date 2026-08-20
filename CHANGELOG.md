# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.19.0] - 2026-08-20

### ⚠ WIRE CHANGE — embedded resources now serialize as the spec's `EmbeddedResource`

**This is a conformance FIX, and there is no opt-out escape hatch.** An embedded
resource inside a tool result (`CallToolResult.content`) or a prompt message
(`GetPromptResult.messages[].content`) now serializes as the shape the MCP
schema declares:

```json
{ "type": "resource", "resource": { "uri": "…", "mimeType": "…", "text": "…" } }
```

Previously pmcp emitted a FLAT object with the payload hoisted to the top level:

```json
{ "type": "resource", "uri": "…", "text": "…", "mimeType": "…" }
```

The change applies to **both** 2025-11-25 and 2026-07-28 — the era decision is
per request, and this is not an era-dependent shape.

**Why there is no opt-out.** `EmbeddedResource.resource` is
`TextResourceContents | BlobResourceContents` in every published version of the
schema (`schema.ts:1734-1748`). The flat object matched no arm of that union, so
every other MCP implementation rejected or misread pmcp's embedded resources.
Reliance on the flat shape was reliance on a bug, and keeping a flag to reproduce
it would keep pmcp emitting a shape no conformant peer can read.

**What did NOT change: `ReadResourceResult.contents` stays FLAT.** That position
is `ResourceContents[]` (`schema.ts:1514-1560`), which the spec declares flat and
without a `type` discriminator, and which pmcp already emitted correctly. It
gains `blob` and nothing else.

### Added

- `Content::Resource` gains **`blob`** — the `BlobResourceContents` arm
  (`schema.ts:1548`), so a binary resource can finally be embedded and so
  `resources/read` on a binary resource answers `{uri, mimeType, blob}`.
- `Content::Resource` gains **`annotations`** — `EmbeddedResource.annotations`
  (`schema.ts:1741`), emitted as a SIBLING of `resource`, never inside it.
- `Content::resource_with_blob(uri, blob, mime_type)`, `Content::with_annotations`
  and `Content::with_meta`. These are **mandatory, not convenient**: see the
  `#[non_exhaustive]` note below.
- `Content`'s reader is now TOLERANT: it accepts the nested spec shape **and**
  the legacy flat shape, and emits only the nested one. This also fixes a
  client-side defect — before this release a spec-conformant embedded resource
  from any other SDK's server failed to parse at all with ``missing field `uri` ``.
  The tolerance is a compatibility affordance for mixed-version fleets, not a
  second supported wire format.
- **`SharedSender`** — a new public trait, plus a defaulted
  `Transport::shared_sender()` accessor that returns `None`. A transport that
  implements it hands the client an OWNED send handle, so `Client` takes its
  transport guard only long enough to ask for that handle, drops it, and only
  then awaits the send. Additive: a transport that does not implement it keeps
  the previous exclusive path. `StreamableHttpTransport` implements it;
  `PooledTransport`, `HttpTransport` and `WasmHttpTransport` do not.
- **`Era` / `protocol_era()` and the `V2_PROTOCOL_VERSIONS` table** — v2
  membership now has ONE source of truth that both the era classifier and the
  `MCP-Protocol-Version` echo read, so adding a future v2-generation version is
  a single-table edit and cannot make the echo advertise the wrong spelling.

### Changed

- **`Content::Resource` is now `#[non_exhaustive]`.** Downstream crates construct
  it through `Content::resource_with_text` / `Content::resource_with_blob` (plus
  the `.with_annotations(..)` / `.with_meta(..)` builders) and match it with a
  `..` rest pattern. This is deliberate and one-time: landing the attribute in
  the same change as `blob` and `annotations` makes every FUTURE spec field on
  this variant a minor version bump instead of a major one.
- An embedded resource carrying neither `text` nor `blob` emits
  `{"type":"resource","resource":{"uri":"…","text":""}}`. Both arms of the spec
  union require content, so `text` is the default arm; an object carrying neither
  key would match no arm at all.
- A payload carrying BOTH `text` and `blob` inside `resource` is REJECTED on
  input, because the spec type is an XOR.
- **A stalled peer can no longer wedge a whole `Client`.** Client sends route
  through an owned handle taken under a momentary guard that is released BEFORE
  the round trip is awaited, so a peer that accepts a POST and never writes its
  response head now blocks only its own call — not every other operation on that
  client, `close()` included. Two disclosed exceptions remain: a POOLED
  `StreamableHttpTransport` keeps the exclusive path, and `Client::open_event_stream`
  still holds a read guard across the `subscriptions/listen` response head.
- **Concurrent token vends on one transport are single-flighted.** Two paths that
  were serialised only by accident are now serialised in their own right: the
  `401` refresh (purge through retry BUILD), and — new in this release — the
  ORDINARY vend while the credential cache is cold. Several concurrent first
  requests on one cloned transport previously reached `AuthProvider::get_access_token`
  once each; against a ROTATING refresh token the identity provider accepts one and
  rejects the rest, and each rejection invalidates the token the winner cached, so
  the transport's auth failed permanently before any `401` had occurred. NOTE the
  limit: this holds only if your `AuthProvider` CACHES what `get_access_token`
  returns. The trait states no caching contract and pmcp cannot enforce one —
  against a non-caching provider the vends are serialised but still plural.
- **Session-stream restarts are atomic.** Two overlapping restarts can no longer
  leave an orphaned reader holding a live connection.

### Fixed

- **Docs: the v2 server track said the opposite of the code.** The README and the
  migration guide both stated that servers need do nothing for v2 — that a server
  built with default features "already answers both eras". The default accept-list
  is v1-only BY DESIGN (`default_accept_list()` excludes `2026-07-28`, so no server
  reaches the v2 era by accident), and v2 is reached only through
  `ServerBuilder::with_supported_protocol_versions(..)`. A team following the guide
  shipped a v1-only server and believed it was done, with the failure silent until
  a v2 client arrived. Both documents now teach the opt-in, with the v1 version
  listed ALONGSIDE v2. No code change: the docs were wrong, not the default.
- **`pmcp-code-mode` no longer declares `execute_code` as a safe read.**
  `readOnlyHint`/`destructiveHint` were shared with `validate_code`, telling every
  host it could auto-approve execution of caller-supplied code. `execute_code` now
  declares nothing and the MCP defaults (`readOnlyHint = false`,
  `destructiveHint = true`) stand.

### Deprecated

- `Content::resource(uri)` — a URI-only value cannot be a spec-valid
  `EmbeddedResource`; that is `ResourceLink`. Use `Content::resource_with_text` /
  `Content::resource_with_blob` for embedded content, or `Content::resource_link`
  for a reference. The body and the returned variant are unchanged; removal is
  scheduled for the next major.

### Migration

- **Readers of pmcp tool results and prompt messages must read
  `content.resource.uri` instead of `content.uri`** (and `content.resource.text`
  / `content.resource.blob`). `content._meta` and `content.annotations` stay at
  content level and are unmoved — the MCP Apps widget path that destructures
  `{ uri, meta, .. }` at content level reads `_meta` there and is unaffected.
- pmcp's own reader accepts BOTH shapes, so a mixed-version fleet keeps working
  in the client direction while servers roll forward.
- Rust callers who built `Content::Resource { .. }` with a struct literal switch
  to `Content::resource_with_text` / `Content::resource_with_blob`; callers who
  matched it exhaustively add a `..` rest pattern.
- Readers of `ReadResourceResult.contents` change nothing, except that a binary
  resource now actually carries its payload under `blob`.

Phase 119's migration guide is written against this section.

## [2.18.0] - 2026-08-16

SEP-2352 credential storage and OAuth discovery hardening (Phase 116). `pmcp`
gains a credential-storage seam addressed by `(issuer, account, server)`, and
`cargo-pmcp` 0.19.0 drops its own parallel token cache onto it — one machine,
one credential store, one format. Additive on the `pmcp` side; the two
behaviour changes are called out below because both are visible to operators.

### Added

- `pmcp::shared::credential_store` (ungated, I/O-free): `CredentialKey`,
  `StoredCredentials`, `CredentialSnapshot`, `parse_credential_snapshot`,
  `normalize_server_key`, `MigrationReport` / `DroppedEntry`, the
  `CredentialStore` platform seam and its `CredentialStoreAdmin` sibling, and
  `InMemoryCredentialStore`.
- `pmcp::shared::credential_file` (`oauth`, non-wasm): `FileCredentialStore` —
  the default on-disk store, one serialized read-modify-write per mutation,
  `0o600` files in a `0o700` parent — and `default_credential_path`.
- `OAuthHelper::with_credential_store` / `with_account_scope` /
  `with_interactivity`, and `Interactivity::RefreshOnly` for callers (scripts,
  CI) that must never be handed a browser.

### Fixed

- **stdin EOF disabled stdout, dropping responses to requests the server had
  already accepted** (#316). `StdioTransport` tracked both directions with a
  single `closed` flag, so the moment stdin reached EOF the transport refused
  to *write* as well — `stdio.rs:172` set the flag on read EOF and `send()` at
  `:79` returned `ConnectionClosed` on that same flag. A client that writes
  its requests and closes stdin — the ordinary shape for a batch or one-shot
  invocation — is a client that has already handed the server work; those
  responses were silently discarded, and from the client's side the server
  looked like it had hung or answered nothing.

  The flag is now split into `read_closed` and `write_closed`. EOF on stdin
  closes the read side only; the write side latches shut on a real write error
  or an explicit `close()`, so the server can still deliver what it owes.
  Downstream impact, stated precisely: pmat worked around this with a
  three-layer mitigation, the last layer of which exists *only* because of this
  defect — it catches the response pmcp refuses to write and re-emits it
  through `serialize_message` so the bytes match what the transport would have
  produced. That layer can now be deleted. The other two layers address
  separate races (a session ending while a consumed request is in flight, and
  the actor's `select!` breaking without draining its outbound queue) and are
  still needed.

### Changed

- **RFC 8414 §3.3: a trailing-slash issuer is now REFUSED when the
  authorization server's discovery document declares a slash-free `issuer`**
  (and vice versa). The specification requires the document's `issuer` to be
  byte-identical to the value its metadata URL was built from, and this anchor
  deliberately does **not** normalise — a lenient comparison is precisely what
  the rule exists to prevent. The URL *derivation* still normalises a trailing
  slash away, so the two halves of discovery disagree on purpose and a test
  pins the disagreement. An operator who configures `https://as.example/pool/`
  against a provider whose document declares `https://as.example/pool` now gets
  a hard refusal naming BOTH values, where before they got a working provider;
  configure the issuer exactly as the provider publishes it. Real providers are
  unaffected: Auth0 declares its trailing slash, and Cognito, Google, Okta and
  Entra all publish slash-free issuers. Affects `GenericOidcProvider`,
  `CognitoProvider` and the client discovery path.
- `cargo pmcp auth` (cargo-pmcp 0.19.0) stores credentials through
  `FileCredentialStore`, at the same `~/.pmcp/oauth-cache.json` as before. An
  existing login that recorded its issuer is migrated in place on the first
  WRITE; a read-only command leaves the file byte-identical. An entry with no
  recorded issuer cannot be re-keyed without guessing which authorization server
  issued it, so it is dropped, named and counted, and that one server needs a
  fresh `cargo pmcp auth login`.
- `cargo pmcp auth refresh` renews an EXPIRED token instead of posting to the
  token endpoint unconditionally, because the expiry decision now lives in the
  SDK alone rather than in two implementations. It reports which of the two
  happened rather than announcing a refresh it did not perform.
- **`Mcp-Name` is now required on 2026-07-28 requests exactly where the method
  carries a routing name** — `tools/call` / `prompts/get` (`params.name`),
  `resources/read` (`params.uri`) and `tasks/get` / `tasks/update` /
  `tasks/cancel` (`params.taskId`) — rather than on every v2 request (Phase 118
  D-13, widened by D-18). This is one RELAXATION and one TIGHTENING, and both
  are visible on the wire:
  - **Relaxation.** A v2 request for a name-less method (`tools/list`, `ping`,
    `server/discover`, `completion/complete`, `subscriptions/listen`, …) that
    omits `Mcp-Name` is now SERVED; it was refused `-32020 HEADER_MISMATCH` at
    HTTP 400 before dispatch. The old rule was the deliberately-stricter
    Phase-113 DRIFT-1 adjudication, and it rejected effectively the entire
    2026-07-28 scored set of the official `@modelcontextprotocol/conformance`
    suite, which sends the header only for name-bearing methods. A value sent on
    a name-less method is now accepted and DISCARDED, so it is neither branched
    on nor reflected in the response headers.
  - **Tightening.** A v2 `tasks/get` / `tasks/update` / `tasks/cancel` that
    omits `Mcp-Name`, or whose `Mcp-Name` disagrees with `params.taskId`, is now
    refused `-32020 HEADER_MISMATCH` before dispatch. Previously the server
    neither required nor cross-checked it for those three methods while pmcp's
    own client already emitted it — an emitter/validator asymmetry. **A
    non-pmcp v2 client that sent an empty `Mcp-Name` for a `tasks/*` method must
    now send the task id.** `pmcp::Client` over `StreamableHttpTransport` is
    unaffected: it already derives the value from the request body.

  `Mcp-Method` and `MCP-Protocol-Version` remain required on EVERY v2 request,
  and the header/body cross-check is unchanged wherever a name exists. Nothing
  about 2025-11-25 changes — the era decision is per request.

## [2.17.0] - 2026-07-19

Hosted-agent loop enablement (Phase 108). Three paired, fully **additive** core
changes that let a tool handler call back into its client mid-request without
deadlocking, and let that round-trip carry tool calls:

- **Transport Actor pump in `Server::run`.** The transport is now owned by a
  single actor task instead of a shared `Arc<RwLock<T>>`. Inbound responses are
  routed to the peer dispatcher immediately (unblocking an in-tool
  `peer.sample()` / `.list_roots()`), inbound requests go to one sequential
  worker via an unbounded queue, and all outbound frames funnel through one
  channel — so the receive/drain path never blocks on request execution, queue
  capacity, or a transport write-lock. Request handling stays serialized (zero
  behavior change for existing servers).
- **End-to-end WithTools sampling.** New `HostSamplingHandlerWithTools` +
  `ClientBuilder::on_sampling_with_tools` let a client host answer with
  `tool_use` / `tool_result` blocks; legacy `HostSamplingHandler`s keep working
  via `LegacyHostSamplingAdapter`. New `PeerHandle::sample_with_tools` (additive
  default method) returns `CreateMessageResultWithTools`, with a legacy
  single-content decode fallback on `DispatchPeerHandle`.

### Added

- `PeerHandle::sample_with_tools` (additive default method) and its
  `DispatchPeerHandle` override with a legacy `CreateMessageResult` fallback.
- `pmcp::client::host::HostSamplingHandlerWithTools`, `LegacyHostSamplingAdapter`,
  and `ClientBuilder::on_sampling_with_tools`.
- `CreateMessageResultWithTools::from_single` /
  `SamplingMessageContent::from_content` canonical conversions.
- `Transport::receive` now documents a `# Cancellation` contract; `StdioTransport`
  is cancel-safe via a persistent partial-line buffer.

### Changed

- `Server::run` internals refactored to the single-owner Transport Actor
  (transport no longer wrapped in `Arc<RwLock<T>>`). No public API change.

## [2.16.0] - Unreleased

Client host surface (Phase 106): a pmcp `Client` can now **answer**
server→client requests — the MCP spec host direction. Register host handlers on
`ClientBuilder` (`on_sampling`, `on_elicitation`, `on_roots`) plus optional
sampling approval hooks (`on_sampling_approval`, `on_sampling_result_review`),
and the client answers inbound `sampling/createMessage`, `elicitation/create`,
and `roots/list` while one of its own requests is in flight. The legacy inverted
`Client::create_message` path is kept and documented as the distinct
"LLM-server pattern".

### Added

- `pmcp::client::host` module: `HostSamplingHandler`, `HostElicitationHandler`,
  `RootsProvider`, and the two-stage sampling approval seam (`PreflightApproval`,
  `SamplingResultReview`) with `ApprovalDecision`.
- `ClientBuilder` registration methods: `on_sampling`, `on_elicitation`,
  `on_roots`, `on_sampling_approval`, `on_sampling_result_review`.

### Fixed

- `Client::create_message` no longer always fails: `assert_capability` gained
  the missing `"sampling"` arm, so the LLM-server pattern works against any
  server advertising `sampling`.
- Inbound server→client `ping` is now answered with an empty result per the
  spec MUST instead of `-32601`, so keepalive pings no longer fail.
- A failed host-response send no longer leaks the in-flight request's
  `active_requests` entry.

### Changed / Behavior notes

- **Registry-derived host capabilities (HOST-05, potential silent change for
  `Client::new` users).** The `sampling`, `elicitation`, and `roots` capability
  fields sent on `initialize` are now **derived from the registered host
  handlers**, not from the `ClientCapabilities` passed to `initialize`. If no
  matching handler is registered, that field is forced to `None` on the wire
  even when the caller set it (anti-capability-lie: a client must not advertise
  a host capability it cannot service). Code built via `Client::new` that set
  `capabilities.roots`/`sampling`/`elicitation` by hand will no longer advertise
  those capabilities unless it registers the corresponding handler via
  `ClientBuilder`. When a handler *is* registered, caller-configured detail
  (e.g. `roots.list_changed`) is preserved. All other capability fields are
  unaffected.

## [2.15.0] - 2026-07-10

Typed tool output on the wire — a declared `outputSchema` now automatically
produces `structuredContent`. Per the MCP spec, a tool that declares an
`outputSchema` SHOULD return `structuredContent` conforming to it; previously
the SDK published the schema in `tools/list` but stringified every result into
`content[0].text`, setting `structured_content` only for UI widget tools.
Structured-output consumers (web apps, durable agents, codegen clients) had to
`JSON.parse` an undocumented text blob. Motivated by a pmcp.run dev-team
filing (their web-channel typed-results track was blocked on results actually
carrying `structuredContent`).

### Added

- **`CallToolResult::structured(value)`** (`src/types/tools.rs`) — the
  success-side counterpart of `CallToolResult::rejected`: one value, one call,
  both voices. `structuredContent` carries the value verbatim;
  `content[0].text` carries its canonical JSON serialization for text-only
  clients. **`CallToolResult::structured_with_text(value, text)`** keeps a
  distinct human-readable text voice. Intended for handlers that own their
  full envelope (the verbatim `ToolOutput::Result` path, hand-rolled servers).
- **Warn-only output validation** (`src/server/output_validation.rs`, gated on
  the existing `validation` feature) — when a tool declares an `outputSchema`
  and the dispatcher emits `structuredContent`, the value is validated against
  the schema and mismatches log a `tracing` WARNING (never an error result).
  Compiled validators are cached per schema; conforming values take an
  `is_valid` short-circuit. Catches schema drift in dev/CI with no production
  failure mode.
- Docs: "Typed Output on the Wire" section in pmcp-book ch05; shared duplex
  test harness extracted to `tests/common/duplex.rs`.
- `cargo-pmcp` 0.17.2 → 0.17.3: workbook scaffold's emitted `pmcp` pin bumped
  to 2.15.0 (tripwire-enforced against the workspace root version).

### Changed (behavior, non-breaking API)

- **Both native dispatchers** (high-level `Server` and `ServerCore`, so
  stdio/HTTP/WASM deployments alike) now bridge a declared `outputSchema` to
  the wire: Payload-path results from tools with an `outputSchema` (e.g.
  `TypedToolWithOutput`, `tool_typed_with_output`, `#[mcp_tool]` functions
  returning typed `Result<T>`) are dual-emitted — the serialized text voice
  plus `structuredContent`. Tools without a declared schema keep the
  text-only envelope, byte-for-byte unchanged. Widget enrichment precedence
  is preserved.
- **Heads-up for drifted schemas**: clients that validate `structuredContent`
  against the declared schema will now see mismatches that were previously
  hidden by text-only emission. That is the spec-correct outcome; enable the
  `validation` feature in dev/CI to catch drift server-side first.
- `ServerCore`'s text voice for schema-declaring tools is now compact JSON
  (was pretty-printed), matching the high-level `Server` dispatcher and
  shrinking dual-emit payloads.

## [2.14.0] - 2026-07-08

`Task.diagnostic_detail` — a PMCP extension field for two-voice task status.
One additive, wire-neutral change (no breaking changes): `Task` gains an
optional second status voice so producers can carry a business-friendly
`status_message` and a full operator/developer diagnostic independently.
Motivated by a pmcp.run dev-team request (their task servers need a redacted,
end-user message plus an expandable operator detail behind a "details"
affordance).

### Added — `Task.diagnostic_detail` (`src/types/tasks.rs`)

- **`Task.diagnostic_detail: Option<String>`** (wire key `diagnosticDetail`, D-17)
  — a **PMCP extension, not an MCP-spec field**. The MCP `Task` type carries a
  single `status_message` voice, which PMCP treats as the business-friendly,
  user-facing voice; `diagnostic_detail` is a separate operator/developer voice
  (step ids, URLs, internal error text) meant for an expandable-detail UI.
  Producers MUST redact secrets/tokens before setting it (documented on the
  field).
- **`Task::with_diagnostic_detail(detail)`** builder — mirrors
  `with_status_message` exactly.
- Additive and non-breaking: `Task` is `#[non_exhaustive]` and built via
  `Task::new()` + builders (no struct-literal breakage), the field is
  `skip_serializing_if = "Option::is_none"` (callers that never set it produce
  byte-identical JSON to 2.13.0), and `Task` has no `deny_unknown_fields` so
  strict/older consumers ignore the extra key. Covered by three serde tests
  (absent-when-`None`, `Some(..)` round-trip, and consumer-tolerance of the new
  key plus an unrelated unknown field).
- **Future migration note:** if the MCP spec grows a `_meta` extension slot on
  `Task`, this field is a candidate to move under it rather than staying a
  top-level struct field.

## [2.13.0] - 2026-07-05

Loop-free task poll-decision classifier + durable/replay-consumer docs. One
additive, wire-neutral feature set (no breaking changes): the terminal /
pollable / input-required poll decision is factored OUT of `Client::wait_for_task`'s
loop into a shared classifier that `wait_for_task` consumes internally (so the two
poller shapes cannot drift) and durable/replay consumers can call per-poll without
blocking. Motivated by a pmcp.run dev-team request (their durable poller cannot
use the blocking `wait_for_task` and had to re-derive terminal/input-required
detection by hand).

### Added — Task poll-decision classifier (`src/types/tasks.rs`)

- **`TaskPollDecision`** — a `#[non_exhaustive]` enum with three variants
  (`Terminal { status } | InProgress { poll_hint } | InputRequired`). Deliberately
  carries no `serde` derives: it is a returned classifier value, not a wire type.
- **`Task::poll_decision(&self) -> TaskPollDecision`** — a pure, total function over
  the five `TaskStatus` variants (no `_` arm, no I/O, no `CallToolResult` fetch).
  Safe to call on every replay of a durable workflow because it classifies an
  already-deserialized `Task`. The terminal `CallToolResult` still comes from a
  separate `tasks/result` call the consumer owns.
- **`resolve_poll_interval(caller, hint) -> u64`** plus **`pub const DEFAULT_POLL_MS`
  (1000) / `MIN_POLL_MS` (50)** — the poll-interval precedence chain
  (caller override → server hint → default → 50 ms floor), now a single shared
  source of truth documented as stable public defaults.

### Changed — `Client::wait_for_task` (`src/client/mod.rs`)

- The poll loop is now an explicit `match task.poll_decision()` that consumes the
  shared classifier instead of re-deriving `is_terminal()` / `== InputRequired`
  inline. Blocking behavior, the budget clamp, and the `input_required` typed-error
  message are **byte-identical** to 2.12.0 (pinned by a strengthened regression
  test that also asserts no `tasks/result` fetch happens on the `input_required`
  path). No wire changes; no new `TaskStatus` variants.

### Added — Docs & example

- **`examples/s48_durable_poll_decision.rs`** — a runnable plain classifier poll
  loop (no `wait_for_task`, no fake durable runtime), with the `tasks/result` fetch
  guarded so it is unreachable on the `InputRequired` path.
- **"Durable and replay consumers"** section in the Tasks chapter (pmcp-book)
  covering the `ctx.step`/`ctx.wait` typed-accessors-without-the-loop pattern, the
  replay-determinism caveat, the two distinct semver claims, and an explicit
  "when NOT to use `wait_for_task`" warning.

## [2.12.0] - 2026-07-05

Task-Augmented Tool Results DX (SEP-1686 junction). One additive feature set, no
breaking changes: a tool can now return a full, task-augmented `CallToolResult`
(`_meta` included) through normal `Server`/`ServerCore` dispatch without it being
re-stringified into `content[0].text` — killing the silent double-wrap bug class
(5 incident variants documented by the pmcp.run team, including a 2-week silent
production outage).

### Added — Task-augmented tool results (SEP-1686)

- **`ToolOutput` enum + `ToolHandler::handle_output()`** — a handler can return
  `ToolOutput::Result(CallToolResult)` to land its envelope on the wire VERBATIM
  (`_meta` preserved, no text-wrap, no widget enrichment). The default
  `handle_output()` delegates to `handle()` → `ToolOutput::Payload`, so every
  existing handler is unchanged. Both native dispatchers (`Server` and
  `ServerCore`) branch through one shared decision helper so they cannot drift.
  ⚠ `ToolOutput::Result` deliberately **bypasses response middleware**
  (redaction/sanitization/audit) — the handler owns its own redaction (D-04a);
  request middleware, auth, and error routing are unchanged.
- **`ServerBuilder::tool_with_result()` / `tool_with_result_and_description()`** —
  typed-closure sugar (mirrors `tool_typed_with_output`) whose closure returns a
  full `CallToolResult`; routed through `ToolOutput::Result`.
- **`RequestHandlerExtra::set_result_meta()`** — lowest-friction `_meta` retrofit
  for existing Payload-path handlers: merged into the outgoing result with
  handler-key-wins precedence (unrelated widget/native `_meta` keys preserved).
- **Double-wrap tripwire** — dispatch now emits a `tracing::warn!` (all builds)
  and `debug_assert!`-fails (debug builds only; release never panics) when about
  to text-wrap a `Value` that structurally IS an already-built `CallToolResult`
  (high-precision envelope-key markers, no full deserialize). Per-tool opt-out:
  `suppress_double_wrap_check(name)` on both builders.
- **Client-side SEP-1686 surface** — typed `TaskMetadata`
  (`_meta["io.modelcontextprotocol/related-task"]`),
  `CallToolResult::with_related_task()` / `related_task()` accessor twins, and a
  wasm-safe `Client::wait_for_task()` / `wait_for_related_task()` polling
  convenience (`WaitForTaskOptions::from_metadata` composes straight from
  `TaskMetadata`; timeout budget strictly honored; 50 ms interval floor;
  non-terminal `input_required` surfaces as a typed error instead of hanging).
- **Migration guide + example** — runnable BEFORE/AFTER example
  `s47_task_augmented_result`, live-HTTP `_meta`-at-top-level regression test
  consuming real dispatch output, `docs/design/sep-1686-task-augmented-results.md`,
  and pmcp-book chapter 12.7 (Task-Augmented Tool Results).

## [2.11.0] - 2026-06-30

Three additive feature sets, no breaking changes: (1) reusable browser web-channel
building blocks — a target-agnostic OAuth PKCE crypto helper and a now-functional
WASM HTTP transport; (2) MCP **task dispatch on the high-level `Server` / HTTP**,
closing the 2.10.0 follow-up; and (3) a **workbook accuracy-verification surface**
for the config-driven workbook toolkit.

### Added — Web-channel browser client (PKCE + Tasks over Fetch)

- **Wasm-safe PKCE crypto helper** —
  `pmcp::shared::pkce::{generate_code_verifier, code_challenge_s256, generate_state}`.
  A target-agnostic OAuth 2.0 PKCE primitive (RFC 7636) backed by
  `getrandom` + `sha2` + `base64`. The S256 challenge helper is infallible
  (`#[must_use] String`); only the RNG-backed verifier/state generators return
  `Result` — no `unwrap`/`expect` in production. Links on both host and
  `wasm32-unknown-unknown`, so a browser client can drive the OAuth
  authorization-code-with-PKCE flow without a server round-trip for the
  challenge (D-02/D-03).
- New reference example `examples/web-channel-client` — a browser MCP client
  split into a `client/` wasm `cdylib` crate (zero native HTTP deps) and a
  bundled `server/` native crate (OAuth IdP merged with MCP via the public
  `pmcp::axum::router().merge()` seam), demonstrating PKCE + the high-level
  `Client` task lifecycle over browser Fetch.

### Fixed

- **`WasmHttpTransport` now correlates `send()` / `receive()`** — the
  `Transport` impl was a non-functional stub; it now buffers the Fetch
  response in a one-slot pending buffer (with a double-send guard that errors
  on an occupied slot) so the high-level `Client` and its typed task helpers
  work over browser Fetch (D-08). The `WasmHttpTransport` symbol was already
  exported; this is a behavior change that makes it usable.
- **`Instant::now()` no longer panics on `wasm32`** — `MiddlewareContext`
  stamped a `std::time::Instant`, which panics with "time not implemented on
  this platform" on `wasm32-unknown-unknown`. Because `Client::send_request`
  builds a `MiddlewareContext` per request, *every* MCP request aborted in the
  browser. `src/shared/middleware.rs` now uses `web_time::Instant` (a drop-in
  that is `std::time::Instant` on native and `performance.now()`-backed on
  wasm). Adds the lightweight `web-time` dependency.
- **`WasmHttpTransport` puts a valid JSON-RPC frame on the wire** — it
  serialized the untagged `TransportMessage` enum directly, so a request went
  out as `{"id":…,"request":…}` and servers rejected it with `-32700`
  "Unknown message type". The pure JSON-RPC codec
  (`serialize_message`/`parse_message`) is now the single source of truth in
  `pmcp::shared::transport`; `StdioTransport` delegates to it and
  `WasmHttpTransport` uses it for both directions.
- **`WasmHttpTransport` reads SSE tool responses** — the streamable-HTTP
  server answers `initialize` as `application/json` but streams `tools/call` /
  `tasks/*` results as a single `text/event-stream` frame. A browser Fetch
  cannot negotiate SSE streaming, so the transport now accepts *both* a raw
  JSON body and a single SSE `data:` frame before parsing.

  These last three fixes were surfaced by end-to-end browser UAT of the
  `web-channel-client` example; they are invisible to `wasm-pack build` and to
  the native HTTP tests, which run on a host target with SSE-aware transports.

### Changed

- `getrandom` was relocated into the cross-target `[dependencies]` table
  (dependency hygiene, HIGH-1) so the ungated PKCE helper links on host as
  well as wasm32. No new external dependency is added — this is a manifest
  relocation only.

### Added — HTTP task dispatch on the high-level server

Closes the 2.10.0 follow-up ("the high-level `pmcp::Server` / `StreamableHttpServer`
does not yet carry a `TaskStore`"). A tool exposed as an MCP Task is now pollable
over HTTP end to end, not just on `ServerCore`.

- `ServerBuilder::with_task_store(...)` wires a `TaskStore` into the high-level
  `pmcp::Server`, so `pmcp::axum::router(server)` serves the full `tasks/*` surface
  (`tasks/get`, `tasks/result`, `tasks/cancel`) over streamable HTTP.
- New worked example `examples/s46_http_tool_as_task.rs` and a live HTTP
  acceptance test (`tests/tool_as_task_lifecycle_http.rs`) driving
  `initialize → call(task) → tasks/get → tasks/result` over the wire.

### Added — Workbook accuracy-verification surface (config-driven workbook toolkit)

Trust tooling for the Excel-as-configuration workbook servers: let a business
analyst re-verify that a compiled workbook still reproduces its authored results.

- New `verify_accuracy` meta tool — re-runs the shared executor at the workbook's
  reference inputs and reconciles the recomputed outputs against the cached oracle
  values, reporting per-output deltas.
- `render_workbook` gains an `inputs_only` mode, and text/bool formula outputs now
  render as formula-with-cached-result (not just a literal value) so a downloaded
  workbook re-verifies in Excel.
- Runtime `reconcile` + render support in `pmcp-workbook-runtime`; served through
  the toolkit `workbook` module. Design:
  `docs/design/2026-06-22-workbook-accuracy-verification-design.md`.

## [2.10.0] - 2026-06-21

### Added — Tools-as-Tasks server DX (correct-by-construction task lifecycle)

Make a server that exposes a tool as an MCP **Task** (async, pollable)
correct-by-construction: the entire `tasks/*` wire surface is served from the
SDK's typed structs, eliminating the class of silent shape-mismatch bugs that
came from hand-writing the protocol. Extends the existing `TaskStore` path with
no breaking changes.

- `tasks/result` is now served typed (`CallToolResult`) from the configured
  `TaskStore`, with `TaskRouter` fall-through for the legacy path. A tool-created
  task is now actually pollable: the store mints the task id and it is written to
  both the wire `task.taskId` and the `_meta` related-task link.
- New additive `TaskStore` methods — `set_result` / `get_result` /
  `supports_results` — all with default impls (existing stores keep compiling);
  a not-yet-completed `tasks/result` returns a specified error.
- Registering a `TaskStore` (or a `with_task_support` tool) now auto-advertises
  the server-level `tasks` capability; a `TaskSupport::Required` tool with no
  backing store/router makes `build()` return an error instead of advertising a
  hollow capability.
- The client emits a `WARN` (method + transport identity + serde error) on any
  `tasks/*` deserialize failure instead of silently folding it into the result.
- New feature-gated `pmcp::testing` module with
  `assert_roundtrips_through_client`, a conformance helper that feeds real server
  dispatch output through the client deserialization types (`testing` is folded
  into `full`, omitted from `default`).
- New worked example `s45_tool_as_task_lifecycle` and a live in-process
  round-trip acceptance test driving `initialize → call(task) → tasks/get →
  tasks/result`.
- Docs: the book/course/design task chapters and rustdoc now lead with the
  recommended `task_store` + `with_task_support` pattern; the hand-rolled
  `TaskRouter` / hand-written task JSON path is reframed as legacy.

Note: task dispatch + `TaskStore` currently live on `ServerCoreBuilder` /
`ServerCore`; the high-level `pmcp::Server` (and `StreamableHttpServer`) does not
yet carry a `TaskStore` (tracked as a follow-up).

## [2.9.2] - 2026-06-20

Excel-as-Configuration workbook servers: the table-based authoring contract, and
the first crates.io release of the workbook crate tree.

### Added

- **Table-based workbook authoring** — author inputs and outputs as named Excel
  Tables (`name | value | description | tier`); each output Table becomes its own
  named, DAG-typed MCP tool (multi-tool fan-out, one tool per output Table with a
  per-tool input schema reaching only the inputs its formulas use). Supersedes the
  per-cell `in_*`/`out_*` named-range model.
- **`cargo pmcp workbook explain <file>`** (`cargo-pmcp` 0.17.0) — read-only,
  pre-deploy preview of the exact served tool surface, projected through the same
  production compiler path the server registers (so it cannot drift); text +
  `--format json`.
- **First crates.io release of the workbook crate tree** — `pmcp-workbook-dialect`,
  `pmcp-workbook-runtime`, `pmcp-workbook-compiler`, and `pmcp-workbook-server`
  at 0.1.0. The Excel reader (umya) stays confined to the compiler; the served
  crates are reader-free (purity gate).

### Changed

- **`pmcp-server-toolkit` 0.1.1** — the served workbook surface fans out to one
  tool per output Table with DAG-derived per-tool input schemas; `get_manifest`
  advertises the stripped served keys; compile-time gates reject reserved-tool-name
  and output-key collisions. (Additive behind the `workbook` feature — the SQL
  connectors' `^0.1.0` pins are unaffected.)

### Fixed

- **`pmcp-toolkit-mysql` 0.1.1** — sqlx 0.9 `SqlSafeStr`: wrap the audited dynamic
  query in `AssertSqlSafe` (the SQL is placeholder-translated and every value is
  bound via `bind_one`, never interpolated).

## [2.9.1] - 2026-05-31

Completes the v2.9.0 release and fixes a yanked-dependency breakage.

### Fixed

- **`pmcp-code-mode` 0.5.3** — repin the optional `swc_ecma_*` JS-parser
  dependencies (used by the `openapi-code-mode` feature) off the **yanked swc
  "40" generation** back to the non-yanked "39" generation. `swc_ecma_parser`
  40.0.0, `swc_ecma_ast`/`swc_ecma_visit` 24.0.0, and `swc_common` 22.0.0 were
  all yanked from crates.io, which made 0.5.2's `^40` pin unresolvable and broke
  every build of `pmcp-code-mode` (and the toolkit crates) with the swc-backed
  feature enabled.

### Added

- The six v2.2 config-driven toolkit crates — `pmcp-server-toolkit`,
  `pmcp-toolkit-postgres` / `-mysql` / `-athena`, `pmcp-sql-server`,
  `pmcp-openapi-server` — are now published to crates.io. They were absent from
  `release.yml`'s hardcoded publish list and so were skipped during the v2.9.0 run.

Versions: `pmcp-code-mode` 0.5.2 → 0.5.3; `pmcp` (2.9.0) and the six toolkit
crates (0.1.0) are unchanged.

## [2.9.0] - 2026-05-30

The **config-driven servers** release: build production MCP servers over SQL
databases and OpenAPI/HTTP backends from a `config.toml` alone — **no Rust
required**. This removes the biggest blocker to putting organizational data
behind MCP: a business analyst curates the API slice in config, and the toolkit
synthesizes the server. Curated tools cover the common ~20%; Code Mode handles
the long-tail ~80% under a static, default-deny policy.

### Added

- **New crates (first publish) — the config-driven server toolkit:**
  - `pmcp-server-toolkit` — backend-agnostic library: config types, the
    `[[tools]]` synthesizer, Code Mode wiring, and the connector/outgoing-auth seams.
  - `pmcp-sql-server` — Shape-A binary serving SQLite / Postgres / MySQL / Athena
    from `config.toml` + a schema file. Ships a runnable `sqlite-explorer` example.
  - `pmcp-openapi-server` — Shape-A binary serving any OpenAPI / HTTP backend, with
    six outgoing-auth models including OAuth **passthrough** (forwards the caller's
    own token; the server holds no standing credential). Ships `london-tube`
    (api_key) and `contoso-m365` (oauth_passthrough, Microsoft Graph + Excel) examples.
  - `pmcp-toolkit-postgres`, `pmcp-toolkit-mysql`, `pmcp-toolkit-athena` —
    per-backend SQL connectors for the toolkit.
- **`pmcp` SDK — new public APIs** (consumed by the toolkit):
  - `Server::tool_arc`, `Server::prompt_arc`, `Server::resource_arc` — register a
    pre-built `Arc<dyn …Handler>` (handler-level composition + testing).
  - `Server::get_tool` — accessor mirroring the existing `get_prompt` / `get_resource`.
  - `CallToolResult::rejected(message, details)` — companion to the new
    `Error::ToolRejected` variant for policy-rejection results.
- **`cargo pmcp new --kind sql-server` / `--kind openapi-server`** — scaffold a
  config-driven server as a small, deployable crate (Shape-B/C sibling of the
  Shape-A binaries), ready for `cargo pmcp deploy`.
- Main README gains a config-first **Path 1: Config-Only Servers (No Rust)** and a
  Config-Driven Servers ecosystem section; `pmcp-book` / `pmcp-course` gain
  Config-Driven SQL and OpenAPI chapters (incl. the Contoso M365 oauth_passthrough
  walkthrough).

### Changed

- `pmcp-code-mode` 0.5.2 — additive validation-config types backing the SQL /
  OpenAPI Code Mode policy surfaces (`sql-code-mode`, `openapi-code-mode`).
- `cargo-pmcp` 0.15.0 — config-driven scaffold kinds, deploy enhancements, and a
  comprehensive command/README reference.

## [2.8.1] - 2026-05-17

### Added

- `AuthContext.claims` now includes Cognito `custom:*` attributes forwarded
  by pmcp.run mcp-proxy via `x-pmcp-claim-custom-*` headers. See
  `docs/proxy-contract.md` for the wire format. Additive change — no
  public API break; consumers reach the new keys via the existing
  `ctx.claim::<T>("custom:<snake>")` helper.

## [2.8.0] - 2026-05-16

### Added

- **`AuthProvider::on_unauthorized()` transport hook.** New default-method on the
  `pmcp::shared::streamable_http::AuthProvider` trait, invoked by the
  streamable-HTTP transport immediately after a 401 response and before the
  single auto-retry's `get_access_token()` call. Default impl is a no-op,
  preserving backward compatibility for all existing implementers
  (`ProxyProvider`, `NoOpAuthProvider`, downstream user impls). Implementers
  of cached-token providers (e.g. pmcp.run's `OutboundOAuthAuthProvider`)
  should override this method to evict stale cached tokens so the SDK's
  subsequent retry fetches a fresh credential. See the trait doc comment
  in `src/shared/streamable_http.rs` for the retry guarantee.

### Changed

- **Streamable-HTTP transport: single retry on HTTP 401.** When an `AuthProvider`
  is configured and the upstream server returns 401, the SDK now calls
  `on_unauthorized()` and re-sends the request exactly once with a freshly
  vended token. The retry preserves the original method, byte-identical body,
  session ID, resumption token, extra_headers, and middleware chain — only
  the `Authorization` header is recomputed. A second 401 on the retry is
  returned to the caller unchanged (no infinite loop). For requests with no
  `auth_provider` configured, behavior is unchanged.

  Behavior note: callers running against a server that returns 401 for an
  auth-bearing request will now see one extra round-trip per failed request.
  This is the intended fix for stale-token flows and is negligible in normal
  operation. Five inline unit tests (`src/shared/streamable_http.rs`) plus two
  property-based tests (`tests/streamable_http_oauth_properties.rs`) cover
  the invariant.

- **MSRV bumped to Rust 1.91.** The CI MSRV job now pins
  `dtolnay/rust-toolchain@1.91`; root `Cargo.toml` declares
  `rust-version = "1.91.0"`. The current 401-retry implementation uses
  nested `if let` rather than let-chains, so 1.91 is a policy refresh
  (six-month headroom from latest stable) rather than a hard requirement
  of the new feature.

### Internal

- Workspace pin ripple: `mcp-tester` 0.6.0 → 0.7.0, `cargo-pmcp` 0.13.0 →
  0.14.0; pmcp dep version updated to `2.8.0` in `crates/mcp-tester`,
  `cargo-pmcp`, `crates/pmcp-server`, `crates/pmcp-server/pmcp-server-lambda`,
  and `crates/pmcp-tasks`. `pmcp-code-mode` and `pmcp-code-mode-derive`
  use `>=2.2.0` range pins and were not touched.

## [2.7.0] - 2026-05-10

### Security

- **RUSTSEC-2026-0098, RUSTSEC-2026-0099, RUSTSEC-2026-0104** — Resolve three
  transitive `rustls-webpki 0.101.7` advisories pulled in via the AWS SDK
  default `rustls` feature → `aws-smithy-runtime/tls-rustls` →
  `aws-smithy-http-client/legacy-rustls-ring` → `hyper-rustls 0.24` →
  `rustls 0.21.12` / `rustls-webpki 0.101.7`. The workspace's AWS SDK direct
  deps (`aws-sdk-dynamodb`, `aws-sdk-secretsmanager`, `aws-sdk-verifiedpermissions`,
  and the three `aws-config` pins) now use `default-features = false` and opt
  into `default-https-client` only, which maps to the modern
  `rustls-aws-lc` path (`rustls 0.23` / `rustls-webpki 0.103` via `aws-lc-rs`).
  No public API change.

### Changed

- **Phase 75 / 75.5 — Cognitive-complexity refactors across the SDK.** Workspace
  PMAT cognitive-complexity hotspots reduced to ≤ 25 per function (with a hard
  cap of 50 for irreducibly complex protocol/parser dispatch). Notable
  refactored hotspots: `streamable_http_server` (`handle_post_with_middleware`,
  `handle_post_fast_path`, `validate_headers`, `handle_get_sse`,
  `validate_protocol_version`, `build_response`), `path_validation::validate_path`
  (cog 103 → ≤25), `schema_utils` (3 hotspots cog 56/55/41 → ≤25),
  `workflow::task_prompt_handler::classify_resolution_failure`, and
  `utils::json_simd` (`parse_json_fast`, `pretty_print_fast`). No public API
  changes intended; behavior covered by existing tests.
- **CI quality gate** — `pmat quality-gate --fail-on-violation --checks complexity`
  now runs in `.github/workflows/ci.yml` and is gated as a required check via
  the `gate` aggregate job. PMAT pinned to `3.15.0`.

### Internal

- Cumulative phase work since v2.6.0 (Phases 73–79). Per-phase summaries live
  under `.planning/phases/<NN>-*/SUMMARY.md` in the source tree.

## [2.6.0] - 2026-04-21

### Added

- **pmcp 2.6.0 — Typed client helpers** (Phase 73, PARITY-CLIENT-01):
  `Client::call_tool_typed<A: Serialize>`, `Client::call_tool_typed_with_task<A: Serialize>`,
  `Client::call_tool_typed_and_poll<A: Serialize>`, and `Client::get_prompt_typed<A: Serialize>`.
  Each serializes caller-provided `&A` via `serde_json::to_value` and delegates to the existing
  untyped sibling method. Serialization failures return `Error::validation` naming the argument
  source. Signatures match the live siblings exactly — `call_tool_typed_with_task` is two-arg;
  `call_tool_typed_and_poll` is three-arg with `max_polls: usize`.
- **pmcp 2.6.0 — Auto-paginating list helpers** (Phase 73, PARITY-CLIENT-01): `Client::list_all_tools`,
  `Client::list_all_prompts`, `Client::list_all_resources`, and
  **`Client::list_all_resource_templates`** (the last uses the distinct `resources/templates/list`
  capability). Each loops on `next_cursor` and returns the full concatenated item list. A bounded
  `max_iterations` safety cap (configured via `ClientOptions::max_iterations`, default 100) returns
  `Error::validation` rather than looping indefinitely on a buggy server. Empty-string cursors
  (`Some("")`) continue the loop; only `None` terminates. Each helper's rustdoc documents the
  memory-amplification caveat.
- **`pmcp::ClientOptions`** — new `#[non_exhaustive]` configuration struct. Constructed via
  `ClientOptions::default()` + the builder-style `with_max_iterations` setter (external crates)
  or via field-update syntax (`..Default::default()`) from inside the `pmcp` crate. Future
  client-level knobs can land non-breakingly. `max_iterations = 0` is a legal but degenerate
  value (documented in rustdoc; produces immediate `Error::Validation` from every `list_all_*`
  helper).
- **`Client::with_client_options(transport, options)`** — new constructor for wiring a custom
  `ClientOptions`. Does not collide with the pre-existing
  `Client::with_options(transport, info, ProtocolOptions)`. `ClientBuilder` intentionally does not
  expose a `.client_options()` setter in this release — builder-level parity is tracked for a future
  phase.
- **`examples/c09_client_list_all.rs`** — end-to-end demo exercising `Client::with_client_options`,
  `call_tool_typed`, `get_prompt_typed`, and all four `list_all_*` helpers (including
  `list_all_resource_templates`). Drives an MCP server over stdio — see the file header for pairing
  instructions; the binary is not self-contained.
- **`examples/c02_client_tools.rs`** updated to showcase `call_tool_typed` with a
  `#[derive(Serialize)]` struct instead of the prior hand-rolled `json!({...})` pattern.

### Fixed

- **REQUIREMENTS.md §55** — renamed `call_prompt_typed` to `get_prompt_typed` to match the
  MCP method name (`prompts/get`) and the shipped helper name (Phase 73 D-15).

## [2.5.0] - 2026-04-21

### Added

- **pmcp 2.5.0 — Dynamic Client Registration (RFC 7591) support in `OAuthHelper`** (Phase 74).
  `OAuthConfig` gains `client_name: Option<String>` and `dcr_enabled: bool` (default: `true`).
  When `dcr_enabled && client_id.is_none() && discovery.registration_endpoint.is_some()`,
  `OAuthHelper` auto-registers with the server's DCR endpoint before PKCE, eliminating
  the need to pre-provision a client_id against OAuth servers that support RFC 7591.
  Public `DcrRequest` / `DcrResponse` types are re-exported from `pmcp::client::oauth`
  so library consumers can build custom flows on top. New example
  `examples/c08_oauth_dcr.rs` demonstrates the library-user path.
- **`OAuthHelper::authorize_with_details()` + `AuthorizationResult` struct** (Phase 74,
  Blocker #6): returns the full set of OAuth artifacts (access_token, refresh_token,
  expires_at, scopes, effective issuer, effective client_id) so cache consumers can
  persist refresh state across runs. The existing `get_access_token()` API is
  preserved unchanged for simple bearer-header callers.
- **cargo-pmcp 0.9.0 — `cargo pmcp auth` command group** (Phase 74, Plan 02).
  Five subcommands (`login`, `logout`, `status`, `token`, `refresh`) manage per-server
  OAuth tokens in a new `~/.pmcp/oauth-cache.json` (schema_version: 1). `--client <name>`
  flag on `auth login` drives the SDK's new DCR path. `auth token <url>` prints the raw
  access token to stdout (`gh auth token` ergonomics). All server-connecting commands
  (`test/*`, `connect`, `preview`, `schema`, `dev`, `loadtest/run`, `pentest`) now
  consult the cache as the lowest-precedence auth source after explicit flags and
  env vars.

### Changed

- **BREAKING (minor-within-v2.x window):** `OAuthConfig::client_id` type changed `String` -> `Option<String>` to enable DCR auto-trigger when `client_id.is_none()`.
  Existing callers must wrap pre-registered ids in `Some(...)`:

  ```rust
  // Before (pmcp 2.4.x):
  OAuthConfig { client_id: "my-client".to_string(), /* ... */ }

  // After (pmcp 2.5.0+):
  OAuthConfig {
      client_id: Some("my-client".to_string()),
      client_name: None,
      dcr_enabled: false,  // opt out of DCR; use the provided id as-is
      /* ... */
  }
  ```

  Per the v2.x breaking-change window policy in MEMORY.md (v2.0 cleanup philosophy),
  this ships as a minor bump rather than a major.

- **cargo-pmcp `pentest`**: migrated from local `--api-key` flag to shared `AuthFlags`.
  `--api-key` continues to work identically; `--oauth-client-id` / `--oauth-issuer`
  / `--oauth-scopes` are now also accepted for OAuth-protected targets.

## [2.4.0] - 2026-04-17

### Added
- **pmcp 2.4.0 — rmcp parity: request extensions typemap and peer back-channel** (Phase 70). `RequestHandlerExtra` now carries an `http::Extensions` typemap (`.extensions()` / `.extensions_mut()`) that lets middleware attach arbitrary typed state visible to tool/prompt/resource handlers, and a `.peer()` accessor that returns an `Arc<dyn PeerHandle>` so handlers can send notifications, log, or cancel from inside a request without reaching back into the server. Closes the two concrete ergonomics gaps vs. the rmcp SDK surfaced in the rmcp-parity research report.
- **pmcp-macros 0.6.0**: `#[mcp_tool]` now harvests the annotated function's rustdoc comment as the tool description when the `description = "..."` attribute is omitted (PARITY-MACRO-01). Explicit attributes always win over rustdoc; when neither is present, the macro fails with a clear error naming both options. Backwards-compatible — all existing call sites continue to work unchanged.
- **pmcp-macros-support 0.1.0** (new workspace crate): pure non-proc-macro helpers for `pmcp-macros`, extracted so property tests and fuzz targets can consume the rustdoc-harvest normalizer without running into the proc-macro crate's public-API restrictions. Workspace-internal — external users should depend on `pmcp` (with the `macros` feature) or `pmcp-macros` directly, not on this crate.
- **pmcp-macros README**: New "Rustdoc-derived descriptions (pmcp-macros 0.6.0+)" migration section with a compiling `rust,no_run` doctest, plus a "Limitations" subsection enumerating unsupported rustdoc forms (`#[doc = include_str!(...)]`, `#[cfg_attr(..., doc = "...")]`, indented code fences, explicit empty-string descriptions).
- **pmcp-code-mode-derive 0.2.0** (first crates.io publish): companion proc-macro crate to `pmcp-code-mode` providing derive-macro support for Code Mode validation.
- **cargo-pmcp 0.8.0 — pmcp.run landing template: `[login]` section + sign-up flow**. Two change requests from the pmcp.run platform team:
  - **CR-01 — `[login]` section** (silent data-loss fix): `LandingConfig` now has a `login: Option<LoginConfig>` field with `primary_color` / `background_color` / `logo`. Previously any `[login]` block in `pmcp-landing.toml` was dropped by serde before reaching the platform's `deploy-landing` Lambda, so Cognito `UpdateManagedLoginBranding` was never fired end-to-end from a developer deploy. Hex-color validation is mirrored at parse time to catch bad colors locally instead of deferring to the Lambda.
  - **CR-02 — sign-up flow + `/connect` page + `[signup]` TOML**: new Next.js App Router routes (`app/signup/page.tsx`, `app/signup/callback/page.tsx`, `app/connect/page.tsx`), new `Header` and `ConnectSnippet` components (client-side with clipboard + `prompt()` fallback), commented `[signup] redirect_after` block in the template TOML, and a `SignupConfig` struct with open-redirect-safe path validation (rejects absolute URLs, protocol-relative `//host`, non-`/`-prefixed paths). `next build` is verified to succeed cleanly with all four platform-injected `NEXT_PUBLIC_*` env vars unset.

### Changed
- **pmcp-macros**: Error message for `#[mcp_tool]` without a description updated from ``mcp_tool requires at least `description = "..."` attribute`` to ``mcp_tool requires either a `description = "..."` attribute or a rustdoc comment on the function`` — names both fallback sources.
- **pmcp**: Minor version bump 2.3.0 → 2.4.0. Two additive feature surfaces land in this version: the macro surface accepts a newly valid source form (rustdoc-only tool functions via pmcp-macros 0.6.0), and handler code gains the extensions typemap + peer back-channel accessors on `RequestHandlerExtra`.
- Bumped `pmcp-macros` 0.5.0 → 0.6.0 (additive, backwards-compatible minor bump).
- **cargo-pmcp**: Minor bump 0.7.0 → 0.8.0. Covers the concurrent downstream bump for pmcp 2.4.0 (per CLAUDE.md §"Version Bump Rules") plus CR-01 and CR-02 above; intermediate `0.7.1` (CR-01) and `0.7.2` (CR-01 re-issue) were never published to crates.io.
- **mcp-tester**: Patch bump 0.5.0 → 0.5.1 (concurrent downstream bump for pmcp 2.4.0).
- **pmcp-code-mode**: Minor bump 0.4.0 → 0.5.0. Bumps the optional JS-parsing dependencies to the latest compatible swc set: `swc_ecma_parser` 32 → 38, `swc_ecma_ast` 19 → 23, `swc_ecma_visit` 19 → 23, `swc_common` 18 → 21. Supersedes dependabot #233, #235, #236, #237. Verified with `cargo test -p pmcp-code-mode --features openapi-code-mode --lib` (112/112 lib tests pass, no API drift).
- **Docs layout**: Repo root markdown files trimmed from 17 to 5 (README, CRATE-README, CHANGELOG, CLAUDE, QUALITY_REPORT). Five active reference docs moved to `docs/` (MIGRATION, TUTORIAL, RELEASE, TOYOTA_WAY, MIDDLEWARE_ROADMAP); seven historical artifacts moved to `docs/archive/` (MIGRATION_GUIDE, REFACTORING_{FIXES,ISSUE,SUMMARY}, RELEASE_NOTES_v1.5.0, WASM_FIXES_SUMMARY, WASM_POLISH_COMPLETE). All moves preserve file history via `git mv`; two inbound cross-links updated.

### Internal
- New workspace crate `crates/pmcp-macros-support/` scaffolded with the pure normalization helper, unit tests for normalization vectors + unsupported-form cases, and 4 proptest invariants at 1000 cases each (reference-equivalence, determinism, no-panic on arbitrary UTF-8, mixed-attr-shape robustness).
- New trybuild compile-fail snapshots `mcp_tool_missing_description_and_rustdoc.rs` (empty-args) and `mcp_tool_nonempty_args_missing_description_and_rustdoc.rs` (non-empty-args) lock the new error wording against regression.
- New fuzz target `fuzz/fuzz_targets/rustdoc_normalize.rs` exercises the normalizer via `pmcp-macros-support` with mixed attribute shapes (plain doc + `#[doc(hidden)]` + `#[doc(alias = ...)]` + non-doc attrs).
- Shared resolver `pmcp-macros/src/mcp_common.rs::resolve_tool_args` is the single entry point consumed by both `#[mcp_tool]` parse sites (standalone fn in `mcp_tool.rs`, impl-block method in `mcp_server.rs::parse_mcp_tool_attr`) — eliminates the drift risk of duplicated call sequences.

## [2.3.0] - 2026-04-11

### `pmcp` 2.3.0 — no behavioral change, pmcp-macros bump signal

#### Changed
- **Dependency pin bump:** `pmcp-macros` dev-dep and optional-feature-dep both pinned at `0.5.0` (was `0.4.1`). See the `pmcp-macros` 0.5.0 sub-entry below for the breaking-change surface. `pmcp`'s own re-exported public API (`pub use pmcp_macros::{mcp_prompt, mcp_server, mcp_tool};`) is unchanged — users of the `macros` feature who only import `pmcp::mcp_tool` / `pmcp::mcp_server` / `pmcp::mcp_prompt` need no code changes. Users who depend on `pmcp-macros` directly and were still using the deprecated `#[tool]` / `#[tool_router]` / `#[prompt]` / `#[resource]` macros must migrate; see [pmcp-macros/CHANGELOG.md](pmcp-macros/CHANGELOG.md) for the migration guide with before/after code snippets.
- **Version bumped to 2.3.0** to signal the transitive macro-surface change to users checking `cargo update --dry-run` or crates.io diff feeds. A patch bump would have under-communicated the semver-legal breakage in the workspace's macro crate.

### `pmcp-macros` 0.5.0 — Deprecated macros removed, README rewritten

#### Removed (breaking)
- `#[tool]` macro (use `#[mcp_tool]`).
- `#[tool_router]` macro (use `#[mcp_server]`).
- `#[prompt]` zero-op stub (use `#[mcp_prompt]`).
- `#[resource]` zero-op stub (use `#[mcp_resource]`).
- `tool_router_dev` Cargo feature (gated the deleted `#[tool_router]` integration tests).

898 lines of deprecated/stub source removed across 6 files. `lib.rs` crate root shrank from 374 to 226 lines. See [pmcp-macros/CHANGELOG.md](pmcp-macros/CHANGELOG.md) for the complete migration guide including before/after code snippets for each removed macro.

#### Changed
- **Crate-level docs sourced from `pmcp-macros/README.md`** via `#![doc = include_str!("../README.md")]`. docs.rs and GitHub render the same 355-line document — no more stale `pmcp = "1.1"` crate-root docs.
- **README fully rewritten** (252 → 355 lines) to document `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, and `#[mcp_resource]` as the primary API with `rust,no_run` doctest-verified examples. Zero `rust,ignore` fences; API drift is now caught automatically by `cargo test --doc -p pmcp-macros`.
- **Per-macro `///` documentation** references the renamed `examples/s23_mcp_tool_macro.rs` and `examples/s24_mcp_prompt_macro.rs` files from Phase 65 — the previous `63_`/`64_` numbers have been removed from both rustdoc comments and runnable example headers.
- **`docs/advanced/migration-from-typescript.md` and four pmcp-course chapters** updated to `#[mcp_tool]` / `#[mcp_server]` syntax (Phase 66 Wave 1 cleanup of downstream consumers).

## [2.2.0] - 2026-04-06

### `pmcp` 2.2.0 — IconInfo wire format spec compliance (CR-002)

#### Fixed
- **`IconInfo.url` renamed back to `IconInfo.src`** — matches MCP 2025-11-25 spec field name. ChatGPT's pydantic validator rejects responses where the icon field is named `url`. Wire format now emits `src`. `#[serde(alias = "url")]` retained so legacy servers serializing as `url` continue to deserialize correctly. Constructor and fluent API (`IconInfo::new(...)`, `with_mime_type`, `with_sizes`, `with_theme`) are unchanged — the only source-level breakage is direct field access (`icon.url`), which is not used in this workspace.
- **CR-002 regression tests** added: serialization asserts the wire key is `src` and never `url`; deserialization tests cover both new (`src`) and legacy (`url`) inputs; round-trip preserves value.

### `pmcp-macros` 0.4.1 — `#[mcp_tool]` alias matching

#### Fixed
- **`is_value_type()` recognizes common aliases** for `serde_json::Value`. Previously a tool returning `pmcp::Result<JsonValue>` (where `JsonValue` is `use serde_json::{Value as JsonValue}`) generated an `outputSchema` of `{"$schema": "...", "title": "AnyValue"}` — missing the required `"type": "object"` field, causing MCP clients like Gemini CLI to reject **all** tools on the server. The macro now matches `Value`, `JsonValue`, and the fully qualified `serde_json::Value` and skips schema generation for all three.

### `mcp-tester` 0.5.0 — outputSchema conformance check

#### Added
- **T-05: outputSchema validation** in `cargo pmcp test conformance --domain tools`. Validates that every tool with an `outputSchema` has `"type": "object"` at the root per the MCP spec. Skipped if no tools declare `outputSchema`. Catches the macro-generated `AnyValue` schema bug independent of the SDK fix above (defense in depth).

### `cargo-pmcp` 0.6.0 — Billing audience flag, sha2 0.11 fix, deploy hint fix

#### Added
- **`--audience {mcp|billing}` global flag** on `cargo pmcp secret set/get/list/delete` (per CR: pmcp.run billing audience). Default is `mcp` (backwards compatible). `billing` targets the subscription Lambda for servers that opt into Stripe billing via pmcp.run. Threaded through the GraphQL `setServerSecret`/`getServerSecret`/`listServerSecrets`/`deleteServerSecret` operations as `$audience: ServerSecretAudience`. Non-pmcp-run targets (local, aws) reject `--audience billing` with a clear error since they have no subscription-Lambda concept.
- **Platform warning display**: when `setServerSecret` succeeds but no subscription Lambda is registered yet, the platform's non-fatal warning (e.g., "Secret saved but no subscription Lambda is registered…") is shown on stderr in yellow. Exit code stays 0 — the secret was stored, the warning is about downstream propagation, not failure.
- **`Audience` enum** (`Mcp`/`Billing`) with `clap::ValueEnum` derive — gives tab-completion for free.

#### Fixed
- **`sha2` 0.11 `LowerHex` regression**: `format!("{:x}", hasher.finalize())` no longer compiles because `sha2` 0.11's `Array<u8, ...>` output type doesn't implement `LowerHex`. Replaced with explicit hex encoding in `cargo-pmcp/src/pentest/sarif.rs` (SARIF fingerprint) and the `cargo-pmcp/src/templates/oauth/proxy.rs` template (which would have generated uncompilable code for projects scaffolded against sha2 0.11).
- **`cargo pmcp deploy --target pmcp-run` missing-secret hint** now correctly suggests `cargo pmcp secret set --target pmcp-run` instead of `--target pmcp` (which doesn't exist).
- **`cargo pmcp deploy init` tsconfig template** uses `types: ["node"]` instead of `typeRoots: ["./node_modules/@types"]` to avoid TS2580 `Cannot find name 'process'` errors when `node_modules` isn't local (#696c7d4b).
- **`pmcp-server-lambda`** updated to set the new `max_request_bytes` field on `StreamableHttpServerConfig` introduced in pmcp 2.1.0.

#### Internal
- Removed unused `anyhow::Result` import in `pentest/attacks/transport_security.rs`.
- `is_value_type` zero-alloc refactor: direct `Ident` comparison instead of `String` allocation.

## [2.0.2] - 2026-03-24

### Fixed
- **`IconInfo.src` renamed to `IconInfo.url`** — matches MCP spec field name. Servers sending icons (like pmcp.run) caused `initialize` deserialization failure. `#[serde(alias = "src")]` added for backward compatibility.
- **Initialize error reporting** — `Client::initialize()` now reports the actual serde deserialization error instead of a generic "Invalid initialize result format" message.
- Bumped `mcp-tester` to 0.4.1, `mcp-preview` to 0.2.5, `pmcp-server` to 0.2.1, `cargo-pmcp` to 0.5.1 — all aligned to `pmcp` 2.0.2

## [2.0.1] - 2026-03-23

### MCP Tasks — Client API and Server Fixes

### Added
- **Client task methods**: `call_tool_with_task()`, `tasks_get()`, `tasks_result()`, `tasks_list()`, `tasks_cancel()` on the MCP Client
- **`call_tool_and_poll()`**: High-level convenience that calls a tool, auto-polls `tasks/get`, and returns the final `CallToolResult`
- **`ToolCallResponse` enum**: Distinguishes sync results from async task creation on `call_tool_with_task`
- **`RequestHandlerExtra.task_request`**: Tool handlers can check `extra.is_task_request()` to branch between sync and async paths
- **`with_execution()` builder**: All TypedTool variants now support declaring `TaskSupport` via `.with_execution(ToolExecution::new().with_task_support(TaskSupport::Optional))`
- **Task detection in `handle_call_tool`**: Standard `task_store` path returns `CreateTaskResult` with `_meta` related-task metadata when tool declares `taskSupport` and client sends `task` field
- **MCP Tasks documentation**: Book chapter (Ch 12.7), course chapter (Ch 21 with exercises), and updated `docs/TASKS_WITH_POLLING.md`

### Fixed
- **Requestor-driven task detection**: `CreateTaskResult` only returned when client explicitly sends `task` field in `tools/call` — non-task-aware clients (ChatGPT) get `CallToolResult` for compatibility
- **`tracing::warn!`** emitted when tool declares `TaskSupport::Required` but client doesn't send `task` field
- **`call_tool_and_poll` robustness**: Handles `InputRequired` status, only falls back on method-not-found errors (not transport/auth), honors server-updated `poll_interval`
- **Release workflow**: Added `pmcp-macros` publish step before `pmcp` to resolve crates.io dependency ordering

## [2.0.0] - 2026-03-22

### PMCP v2.0 — Aligned with the MCP TypeScript SDK v2.0

This is the first major version bump, marking full alignment with the MCP protocol v2025-03-26 and the TypeScript SDK v2.0 release. PMCP v2.0 brings MCP Apps, MCP Tasks, a conformance test suite, production-grade HTTP security, and improved developer ergonomics across the board.

### Added
- **MCP Protocol v2025-03-26**: Full support for the latest protocol specification with backward compatibility for `2024-11-05`
- **MCP Tasks** (`pmcp-tasks` crate): Experimental shared client/server task state with DynamoDB backend
  - Task lifecycle management (create, update, complete, cancel)
  - Task variables for shared client/server state
  - In-memory backend for dev/tests
- **Conformance Test Suite**: 19-scenario engine across 5 domains (initialize, tools, resources, prompts, notifications)
  - `cargo pmcp test conformance` CLI command with `--strict` and `--domain` flags
  - `mcp-tester conformance` with per-domain CI summary
- **Tower Middleware Stack**: Production-ready HTTP security
  - DNS rebinding protection with configurable allowed origins
  - CORS with origin-locked headers (no wildcard in production)
  - Configurable security headers layer
  - `AllowedOrigins` configuration (localhost, any, custom list)
- **Uniform Constructor DX**: Default impls, builders, and constructors for all protocol types
- **MCP Apps DevTools improvements**: Resizable/collapsible DevTools panel, "Dev Tools" toggle button, global "Clear All", Console tab removed (browser DevTools sufficient)
- **PMCP Server**: MCP server crate exposing SDK developer tools via Streamable HTTP
  - Protocol compliance testing, scenario generation, MCP Apps validation
  - Schema export, code scaffolding, documentation resources
  - Deployed on AWS Lambda at `https://pmcp-server.us-east.true-mcp.com/mcp`

### Changed
- Bumped `pmcp` to 2.0.0
- Bumped `mcp-tester` to 0.4.0
- Bumped `mcp-preview` to 0.3.0
- Bumped `cargo-pmcp` to 0.5.0
- Protocol version negotiation accepts both `2025-03-26` and `2024-11-05`
- `RouterConfig` and `StreamableHttpServerConfig` now include `allowed_origins` field

### Fixed
- Clippy warnings across workspace (derivable_impls, clone_on_copy, map_or patterns)
- Lambda server missing `allowed_origins` field in `StreamableHttpServerConfig`

## [1.19.0] - 2026-03-14

### Added
- **PMCP Server** (`pmcp-server` crate): MCP server exposing SDK developer tools via Streamable HTTP
  - `test_check`: Protocol compliance testing against remote MCP servers
  - `test_generate`: Test scenario generation from server schemas
  - `test_apps`: MCP Apps metadata validation (standard, ChatGPT, Claude Desktop modes)
  - `scaffold`: Code template generation for MCP servers, tools, and resources
  - `schema_export`: Schema discovery and export (JSON and Rust type stubs)
  - Documentation resources and workflow prompts
- **AWS Lambda deployment**: Lambda wrapper crate for running pmcp-server on AWS
- **MCP Registry**: Deployed server at `https://pmcp-server.us-east.true-mcp.com/mcp`
- **Release binaries**: Cross-platform pmcp-server binaries attached to GitHub releases

### Fixed
- `schema_export` and `test_apps` tools now correctly discover tools (was silently failing after `run_quick_test()`)
- `cargo-pmcp deploy`: workspace binary path resolution for Lambda builds
- `cargo-pmcp deploy`: OAuth Lambda copy using correct source path
- `pmcp-macros`: deduplicated `to_pascal_case` into shared `utils` module

### Changed
- Bumped `pmcp-macros` to 0.2.2
- Bumped `mcp-tester` to 0.3.4
- Bumped `cargo-pmcp` to 0.4.5

## [1.11.0] - 2026-02-26

### v1.3 MCP Apps Developer Experience

This release delivers the complete MCP Apps milestone — a full widget authoring, preview, and publishing pipeline for building interactive UI extensions on top of MCP servers.

### Added
- **MCP Apps Preview Server** (`mcp-preview` crate): Live widget preview with dual proxy and WASM bridge modes
  - Axum-based dev server with WebSocket hot-reload
  - Embedded bridge runtime for browser-based MCP communication
- **Widget Authoring**: File-based `WidgetDir` hot-reload and `cargo pmcp app new` scaffolding
  - Automatic bridge script injection via shared `pmcp-widget-utils` crate
- **Publishing Pipeline**: `cargo pmcp app manifest` (ChatGPT action manifest), `cargo pmcp app landing` (standalone demo pages), `cargo pmcp app build` (production bundles)
- **Shared Bridge Library**: TypeScript `App`, `PostMessageTransport`, and `AppBridge` classes for browser ↔ MCP communication
- **New crates**: `pmcp-widget-utils` (shared bridge injection), `mcp-e2e-tests` (browser test harness)
- **Example Apps**: Chess analyzer, interactive map, and data-viz dashboard — each with full preview support
- **E2E Browser Tests**: 20 chromiumoxide CDP tests across all three widget suites
- **cargo-pmcp loadtest module**: TOML config types, MCP client with full handshake and error classification

### Changed
- Bumped `cargo-pmcp` to 0.2.0 (new app subcommands)
- Bumped `mcp-preview` to 0.1.1

## [1.9.1] - 2025-12-29

### Added
- **cargo-pmcp validate command**: New CLI command for project-wide workflow validation
  - `cargo pmcp validate workflows` - Runs cargo check and workflow validation tests
  - `--generate` flag to create test scaffolding
  - `--verbose` and `--server` options for detailed output and workspace support

### Improved
- **TypedTool annotation convenience methods**: Added `.read_only()`, `.destructive()`, `.idempotent()`, `.open_world()` chainable methods
- **TypedToolWithOutput annotation merging**: User-provided annotations now automatically merge with auto-generated output schema
- **Course documentation**: Complete rewrite of Chapter 6 covering soft/hard workflow spectrum and resource embedding

## [1.6.1] - 2025-10-02

### Added
- **Enhanced Prompt Management**: Safer and more flexible way to add prompts to MCP servers
  - Improved workflow integration for tools and resources
  - Better error handling for prompt creation and management
  - Enhanced type safety for prompt arguments
  - Streamlined API for defining prompts with tools and resources workflows

### Improved
- Refined prompt builder patterns for better developer experience
- Enhanced validation for prompt configurations
- Better integration between prompts, tools, and resources

## [1.5.3] - 2025-09-26

### Fixed
- Removed accidentally committed 96MB spin binary from package
- Package size reduced from 98.2MB to ~2MB for successful crates.io publishing

## [1.5.2] - 2025-09-25 (Failed to publish)

### Fixed
- Release workflow to handle existing releases gracefully
- Cargo.toml version alignment for proper crates.io publishing
- Ensure correct tag checkout in GitHub Actions workflow

### Changed
- Updated release workflow to use GitHub CLI instead of deprecated actions/create-release

## [1.5.1] - 2025-09-25 (Skipped)

### Fixed
- Release workflow to handle existing releases gracefully
- Cargo.toml version alignment for proper crates.io publishing

### Changed
- Updated release workflow to use GitHub CLI instead of deprecated actions/create-release

## [1.5.0] - 2025-09-25

### Added
- **WASM MCP Server Support**: Complete WebAssembly deployment capabilities
  - Platform-agnostic WasmMcpServer implementation using PMCP SDK
  - Cloudflare Workers deployment with worker crate
  - Fermyon Spin deployment with spin-sdk
  - "Write once, deploy everywhere" architecture
  - Calculator tool example with comprehensive operations
- **MCP Scenario Testing**: YAML/JSON-based test scenarios
  - Declarative test definitions for MCP servers
  - Support for tool testing with assertions
  - Integration with mcp-tester for automated validation
  - Example scenarios for calculator tool testing
- **Streamable HTTP Transport**: Enhanced HTTP transport with empty response handling
  - Support for 200 OK with empty body
  - Proper Content-Type detection for responses
  - Improved error handling for edge cases

### Fixed
- JSON-RPC notification handling in WASM servers (notifications have no 'id' field)
- Verbose flag propagation in mcp-tester
- Scenario executor assertion logic for Success/Failure cases
- Windows release asset upload paths in GitHub Actions

### Changed
- Refactored WASM server into platform-specific implementations
- Separated core MCP logic from transport/platform layers
- Improved scenario executor to return actual tool responses

## [1.4.2] - 2025-01-15

### Added
- **MCP Server Tester**: Comprehensive testing tool for MCP server validation
  - Protocol compliance validation for JSON-RPC 2.0 and MCP
  - Multi-transport support (HTTP, HTTPS, WebSocket, stdio)
  - Layer-by-layer connection diagnostics
  - Tool testing with custom arguments
  - Server comparison capabilities
  - CI/CD ready with JSON output format
- **Release Workflow**: Automated binary builds and distribution
  - Pre-built binaries for Linux, macOS, and Windows
  - Automatic release creation for forks
  - Cross-platform path compatibility

### Fixed
- JSON-RPC 2.0 compatibility for HTTP transport (Issue #38)
- Null params handling for various MCP methods
- Transport layer fuzz test memory exhaustion issues
- Auth flows fuzz test integer overflow protection
- Windows path format compatibility in CI workflows

## [1.4.1] - 2025-01-16

### 🔧 Enhanced Developer Experience & TypeScript SDK Parity

### Added
- **ToolResult Type Alias (GitHub Issue #37)**
  - `ToolResult` type alias now available from crate root: `use pmcp::ToolResult;`
  - Full compatibility with existing `CallToolResult` - they are identical types
  - Comprehensive documentation with examples covering all usage patterns
  - Complete test suite including unit tests, property tests, and doctests
  - Resolves user confusion about importing tool result types

- **NEW: Complete Example Library with TypeScript SDK Parity**
  - `47_multiple_clients_parallel` - Multiple parallel clients with concurrent operations and error handling
  - `48_structured_output_schema` - Structured output schemas with advanced data validation and response formatting
  - `49_tool_with_sampling_server` - Tool with LLM sampling integration for text processing and summarization
  - All examples developed using Test-Driven Development (TDD) methodology
  - 100% TypeScript SDK feature compatibility verified

- **Enhanced Testing & Quality Assurance**
  - 72% line coverage with 100% function coverage across 390+ tests
  - Comprehensive property-based testing for all new functionality
  - Toyota Way quality standards with zero tolerance for defects
  - All quality gates passing: lint, coverage, and TDD validation

### Fixed
- Fixed GitHub issue #37 where `ToolResult` could not be imported from crate root
- Improved developer ergonomics for MCP tool implementations
- Enhanced API documentation with comprehensive usage examples

### Changed
- Updated to full compatibility with TypeScript SDK v1.17.5
- Improved type ergonomics across all tool-related APIs

## [1.4.0] - 2025-08-22

### 🚀 Enterprise Performance & Advanced Features

This major release introduces enterprise-grade features with significant performance improvements, advanced error recovery, and production-ready WebSocket server capabilities.

### Added
- **PMCP-4001: Complete WebSocket Server Implementation**
  - Production-ready server-side WebSocket transport with full connection lifecycle management
  - Automatic ping/pong keepalive and graceful connection handling
  - WebSocket-specific middleware integration and comprehensive error recovery
  - Connection monitoring and metrics collection for production deployments
  - Example: `25_websocket_server` demonstrating complete server setup

- **PMCP-4002: HTTP/SSE Transport Optimizations** 
  - 10x performance improvement in Server-Sent Events processing
  - Connection pooling with intelligent load balancing strategies
  - Optimized SSE parser with reduced memory allocations
  - Enhanced streaming performance for real-time applications
  - Example: `26_http_sse_optimizations` showing performance improvements

- **PMCP-4003: Advanced Connection Pooling & Load Balancing**
  - Smart connection pooling with health monitoring and automatic failover
  - Multiple load balancing strategies: round-robin, least-connections, weighted
  - Automatic unhealthy connection detection and replacement
  - Comprehensive connection pool metrics and monitoring integration
  - Example: `27_connection_pooling` demonstrating pool management

- **PMCP-4004: Enterprise Middleware System**
  - Advanced middleware chain with circuit breakers and rate limiting
  - Compression middleware with configurable algorithms (gzip, deflate, brotli)
  - Metrics collection middleware with performance monitoring
  - Priority-based middleware execution with dependency management
  - Example: `28_advanced_middleware` showing all middleware features

- **PMCP-4005: Advanced Error Recovery System**
  - Adaptive retry strategies with configurable jitter patterns (Full, Equal, Decorrelated)
  - Deadline-aware recovery with timeout propagation and management
  - Bulk operation recovery with partial failure handling
  - Health monitoring with cascade failure detection and prevention
  - Recovery coordination with event-driven architecture
  - Examples: `29_advanced_error_recovery`, `31_advanced_error_recovery`

- **PMCP-4006: SIMD Parsing Acceleration**
  - **10.3x SSE parsing speedup** using AVX2/SSE4.2 vectorization
  - Runtime CPU feature detection with automatic scalar fallbacks
  - Parallel JSON-RPC batch processing with 119.3% efficiency gains
  - Memory-efficient SIMD operations with comprehensive performance metrics
  - SIMD-accelerated Base64, HTTP headers, and JSON validation
  - Example: `32_simd_parsing_performance` with comprehensive benchmarks

### Performance Improvements
- **SSE parsing**: 10.3x speedup (336,921 vs 32,691 events/sec)
- **JSON-RPC parsing**: 195,181 docs/sec with 100% SIMD utilization
- **Batch processing**: 119.3% parallel efficiency with vectorized operations
- **Memory efficiency**: 580 bytes per document with optimized allocations
- **Base64 operations**: 252+ MB/s encoding/decoding throughput

### Enhanced Developer Experience
- Comprehensive examples for all new features with real-world use cases
- Property-based testing for robustness validation
- Performance benchmarks demonstrating improvements
- Production-ready configurations with monitoring integration

### Security & Reliability
- Circuit breaker patterns preventing cascade failures
- Health monitoring with automatic recovery coordination
- Rate limiting and throttling for DoS protection
- Comprehensive error handling with graceful degradation

## [1.2.1] - 2025-08-14

### Fixed
- Version bump to resolve crates.io publishing conflict

## [1.2.0] - 2025-08-14

### 🏭 Toyota Way Quality Excellence & PMAT Integration

This release implements systematic quality improvements using Toyota Way principles and PMAT (Pragmatic Modular Analysis Toolkit) integration for zero-defect development.

### Added
- **Toyota Way Implementation**: Complete zero-defect development workflow
  - Jidoka (Stop the Line): Quality gates prevent defective code from advancing
  - Genchi Genbutsu (Go and See): Direct code quality observation with PMAT analysis
  - Kaizen (Continuous Improvement): Systematic quality improvement processes
  - Pre-commit quality hooks enforcing complexity and formatting standards
  - Makefile targets for quality gate checks and continuous improvement
- **PMAT Quality Analysis Integration**: Comprehensive code quality metrics
  - TDG (Technical Debt Gradient) scoring: 0.76 (excellent quality)
  - Quality gate enforcement with complexity limits (≤25 cyclomatic complexity)
  - SATD (Self-Admitted Technical Debt) detection and resolution
  - Automated quality badges with GitHub Actions
  - Daily quality monitoring and trend analysis
- **Quality Badges System**: Real-time quality metrics visibility
  - TDG Score badge with color-coded quality levels
  - Quality Gate pass/fail status with automated updates
  - Complexity violations tracking and visualization
  - Technical debt hours estimation (436h managed debt)
  - Toyota Way quality report generation
- **SIMD Module Refactoring**: Reduced complexity while maintaining performance
  - Extracted `validate_utf8_simd` helper functions (34→<25 cyclomatic complexity)
  - Added `is_valid_continuation_byte` and `validate_multibyte_sequence` helpers
  - Separated SIMD fast-path from scalar validation logic
  - Maintained 10-50x performance improvements
- **Enhanced Security Documentation**: Comprehensive PKCE and OAuth guidance
  - Converted SATD comments to proper RFC-referenced documentation
  - Added security recommendations with clear do's and don'ts
  - Enhanced OAuth examples with GitHub, Google, and generic providers
  - PKCE security validation with SHA-256 recommendations

### Changed
- **Quality Standards**: Elevated to Toyota Way and PMAT-level excellence
  - Zero tolerance for clippy warnings and formatting issues
  - All functions maintain ≤25 cyclomatic complexity
  - Comprehensive error handling without unwrap() usage
  - 100% documentation with practical examples
- **CI/CD Pipeline**: Enhanced with quality gates and race condition fixes
  - Fixed parallel test execution with `--test-threads=1`
  - Added pre-commit hooks for immediate quality feedback
  - Quality gate enforcement before any commit acceptance
  - Toyota Way quality principles integrated throughout development

### Fixed
- **CI/CD Race Conditions**: Resolved intermittent test failures
  - Updated CI configuration to use sequential test execution
  - Fixed formatting inconsistencies across the codebase
  - Resolved all clippy violations with proper allows for test patterns
- **SATD Resolution**: Eliminated self-admitted technical debt
  - Converted security-related TODO comments to comprehensive documentation
  - Enhanced PKCE method documentation with RFC 7636 references
  - Added security warnings and recommendations for OAuth implementations

### Quality Metrics
- **TDG Score**: 0.76 (excellent - lower is better)
- **Quality Gate**: Passing with systematic quality enforcement
- **Technical Debt**: 436 hours estimated (actively managed and tracked)
- **Complexity**: All functions ≤25 cyclomatic complexity
- **Documentation**: 100% public API coverage with examples
- **Testing**: Comprehensive property-based and integration test coverage

### Toyota Way Integration
- **Jidoka**: Quality gates stop development for any quality violations
- **Genchi Genbutsu**: PMAT analysis provides direct quality observation
- **Kaizen**: Daily quality badge updates enable continuous improvement
- **Zero Defects**: No compromises on code quality or technical debt

## [1.1.1] - 2025-08-14

### Fixed
- Fixed getrandom v0.3 compatibility by changing feature from 'js' to 'std'
- Updated wasm target feature configuration for getrandom

### Changed
- Updated dependencies to latest versions:
  - getrandom: 0.2 → 0.3
  - rstest: 0.25 → 0.26
  - schemars: 0.8 → 1.0
  - darling: 0.20 → 0.21
  - jsonschema: 0.30 → 0.32
  - notify: 6.1 → 8.2

## [1.1.0] - 2025-08-12

### Added
- **Event Store**: Complete event persistence and resumability support for connection recovery
- **SSE Parser**: Full Server-Sent Events parser implementation for streaming responses
- **Enhanced URI Templates**: Complete RFC 6570 URI Template implementation with all operators
- **TypeScript SDK Feature Parity**: Additional features for full compatibility with TypeScript SDK
- **Development Documentation**: Added CLAUDE.md with AI-assisted development instructions

### Changed
- Replaced `lazy_static` with `std::sync::LazyLock` for modern Rust patterns
- Improved code quality with stricter clippy pedantic and nursery lints
- Optimized URI template expansion for better performance
- Enhanced SIMD implementations with proper safety documentation

### Fixed
- All clippy warnings with zero-tolerance policy
- URI template RFC 6570 compliance issues
- SIMD test expectations and implementations
- Rayon feature flag compilation issues
- Event store test compilation errors
- Disabled incomplete macro_tools example

### Performance
- Optimized JSON batch parsing
- Improved SSE parsing efficiency
- Better memory usage in event store

## [1.0.0] - 2025-08-08

### 🎉 First Stable Release!

PMCP has reached production maturity with zero technical debt, comprehensive testing, and full TypeScript SDK compatibility.

### Added
- **Production Ready**: Zero technical debt, all quality checks pass
- **Procedural Macro System**: New `#[tool]` macro for simplified tool/prompt/resource definitions
- **WASM/Browser Support**: Full WebAssembly support for running MCP clients in browsers
- **SIMD Optimizations**: 10-50x performance improvements for JSON parsing with AVX2 acceleration
- **Fuzzing Infrastructure**: Comprehensive fuzz testing with cargo-fuzz
- **TypeScript Interop Tests**: Integration tests ensuring compatibility with TypeScript SDK
- **Protocol Compatibility Documentation**: Complete guide verifying v1.17.2+ compatibility
- **Advanced Documentation**: Expanded docs covering all new features and patterns
- **Runtime Abstraction**: Cross-platform runtime for native and WASM environments

### Changed
- Default features now exclude experimental transports for better stability
- Improved test coverage with additional protocol tests
- Enhanced error handling with more descriptive error messages
- Updated minimum Rust version to 1.82.0
- All clippy warnings resolved
- All technical debt eliminated

### Fixed
- Resource watcher compilation with proper feature gating
- WebSocket transport stability improvements
- All compilation errors and warnings

### Performance
- 16x faster than TypeScript SDK for common operations
- 50x lower memory usage per connection
- 21x faster JSON parsing with SIMD optimizations
- 10-50x improvement in message throughput

## [0.7.0] - 2025-08-08 (Pre-release)

### Added
- **Procedural Macro System**: New `#[tool]` macro for simplified tool/prompt/resource definitions
- **WASM/Browser Support**: Full WebAssembly support for running MCP clients in browsers
- **SIMD Optimizations**: 10-50x performance improvements for JSON parsing with AVX2 acceleration
- **Fuzzing Infrastructure**: Comprehensive fuzz testing with cargo-fuzz
- **TypeScript Interop Tests**: Integration tests ensuring compatibility with TypeScript SDK
- **Protocol Compatibility Documentation**: Complete guide verifying v1.17.2+ compatibility
- **Advanced Documentation**: Expanded docs covering all new features and patterns
- **Runtime Abstraction**: Cross-platform runtime for native and WASM environments

### Changed
- Default features now exclude experimental transports (websocket, http) for better stability
- Improved test coverage with additional protocol tests
- Enhanced error handling with more descriptive error messages
- Updated minimum Rust version to 1.82.0

### Fixed
- Resource watcher compilation with proper feature gating
- WebSocket transport stability improvements
- Various clippy warnings and code quality issues

### Performance
- 16x faster than TypeScript SDK for common operations
- 50x lower memory usage per connection
- 21x faster JSON parsing with SIMD optimizations
- 10-50x improvement in message throughput

## [0.6.6] - 2025-01-07

### Added
- OIDC discovery support for authentication
- Transport isolation for enhanced security
- Comprehensive documentation updates

## [0.6.5] - 2025-01-06

### Added
- Initial comprehensive documentation
- Property-based testing framework
- Session management improvements

## [0.6.4] - 2025-01-05

### Added
- Comprehensive doctests for the SDK
- Improved examples for all major features
- Better error messages and debugging support

## [0.6.3] - 2025-01-04

### Added
- WebSocket server implementation
- Resource subscription support
- Request cancellation with CancellationToken

## [0.6.2] - 2025-01-03

### Added
- OAuth 2.0 authentication support
- Bearer token authentication
- Middleware system for request/response interception

## [0.6.1] - 2025-01-02

### Added
- Message batching and debouncing
- Retry logic with exponential backoff
- Progress notification support

## [0.6.0] - 2025-01-01

### Added
- Initial release with full MCP v1.0 protocol support
- stdio, HTTP/SSE transports
- Basic client and server implementations
- Comprehensive example suite