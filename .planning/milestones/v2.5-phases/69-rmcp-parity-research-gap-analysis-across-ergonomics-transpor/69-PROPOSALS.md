# Phase 69: Follow-on Phase Proposals

**Generated:** 2026-04-16
**Derived from:** `69-RESEARCH.md` (same phase directory)
**Proposal count:** 3 (per D-15 expected range 2–5)
**Traceability model:** One PARITY-<SURFACE>-01 requirement ID per proposal (not per success criterion); the ID maps to the proposal as a whole.
**Severity bar:** Only High-severity gaps become proposals; Medium/Low gaps are noted in RESEARCH.md for possible future work (D-16).

## Summary

| # | Title | Row IDs addressed | Target slot | Plans | REQ-ID |
|---|-------|-------------------|-------------|-------|--------|
| 1 | Enrich `RequestHandlerExtra` with typemap extensions and peer back-channel | HANDLER-02, HANDLER-05 | v2.2 | 4 | PARITY-HANDLER-01 |
| 2 | Typed client call helpers and auto-paginating list-all convenience methods | CLIENT-02 | late v2.1 | 3 | PARITY-CLIENT-01 |
| 3 | Rustdoc fallback for `#[mcp_tool]` tool descriptions | MACRO-02 | late v2.1 | 3 | PARITY-MACRO-01 |

---

## Proposal 1: Enrich `RequestHandlerExtra` with typemap extensions and peer back-channel

**Derived from:** `69-RESEARCH.md` Row ID(s): HANDLER-02, HANDLER-05
**Severity source:** High
**Suggested phase number:** TBD — slotted by /gsd-add-phase
**Target slot:** v2.2

### Goal

Extend `RequestHandlerExtra` with two independently-motivated capabilities that share one edit site: (1) a typed-key `Extensions` map for request-scoped user data crossing middleware and handler boundaries, and (2) a peer back-channel exposing server-to-client RPCs (`sample`, `list_roots`, `progress_notify`) directly from inside tool/prompt/resource handlers. Today pmcp handlers have neither — middleware-to-handler state transfer requires out-of-band plumbing, and tools that need LLM sampling or root listings mid-execution must route through separate registration-time callbacks rather than the handler body. Both additions are drop-in: `Extensions` defaults empty, and the peer handle is `Option<_>`, preserving backwards compatibility for every existing `RequestHandlerExtra` constructor call.

### Scope

**In scope:**
- Add `extensions: http::Extensions`-compatible typemap field (or equivalent `anymap`) to `RequestHandlerExtra` in `src/server/cancellation.rs` and `src/shared/cancellation.rs` (both native and WASM variants)
- Add `peer: Option<Arc<dyn PeerHandle>>` field (non-wasm only — WASM retains the existing path) with a `PeerHandle` trait exposing `sample(params) -> Result<CreateMessageResult>`, `list_roots() -> Result<ListRootsResult>`, `progress_notify(token, progress) -> Result<()>`
- Wire both fields through `ServerCoreBuilder`'s request-dispatch path so they populate per-request rather than statically
- Add feature-gated `extensions` feature flag (default-on inside the existing `server` feature) that users can disable for minimal builds
- Ship a typed-extension example (`examples/s22_handler_extensions.rs`) and a peer-from-handler example (`examples/s23_handler_peer_sample.rs`) following the Phase 65 role-prefix convention
- Update `src/server/cancellation.rs` rustdoc with a full walkthrough of the three new mechanisms, plus a migration note on `RequestHandlerExtra` constructor evolution
- Property tests verifying: `Extensions` key-collisions return the existing value (typemap semantics), `peer.sample` routes through the correct session, `peer.progress_notify` no-ops when the request has no progress token

**Out of scope:**
- Tower / service integration (already shipped in Phase 56 — not revisited)
- Transport construction API shape (deferred per D-12)
- Client-side `RequestHandlerExtra` equivalent (pmcp's `Client` has no handler-extra today and does not need one for this scope)
- Changing the existing auth-context/session-id fields — they remain as first-class typed fields, not migrated into `Extensions`

### Success Criteria

- [ ] Every existing `RequestHandlerExtra::new(...)` and `::with_session(...)` call-site in `src/` and `examples/` compiles unchanged after the field additions — backwards compatible
- [ ] `examples/s22_handler_extensions.rs` and `examples/s23_handler_peer_sample.rs` both compile and run, demonstrating one cross-middleware extension insert/retrieve and one in-handler `peer.sample()` round-trip
- [ ] `cargo test --features "server"` passes the new property-test module `handler_extensions_properties` with ≥100 proptest cases for typemap insertion/retrieval and peer-handle routing
- [ ] `make doc-check` returns zero rustdoc warnings for the updated `src/server/cancellation.rs` and `src/shared/cancellation.rs`
- [ ] `make quality-gate` passes with zero clippy warnings and zero fmt drift across both changes

### Suggested Requirement ID

- **PARITY-HANDLER-01**: Enrich `RequestHandlerExtra` with a typed-key extensions map and an optional peer back-channel, so middleware state transfer and in-handler server-to-client RPCs work without out-of-band plumbing.

### Estimated Plan Count

4 plans — one for the `Extensions` typemap field + property tests, one for the `PeerHandle` trait + ServerCore wiring, one for the two examples + rustdoc migration notes, and one for CI + migration guide finalization. Multi-day phase sizing similar to Phase 66 (macros doc rewrite) and Phase 67 (docs.rs pipeline) per D-17.

### Rationale / Evidence

Addresses HANDLER-02 and HANDLER-05. Gap cell for HANDLER-02 reads: "pmcp lacks a general-purpose `Extensions` bag for request-scoped state (rmcp's `context.extensions` is an `http::Extensions`-style typemap for cross-middleware state transfer). pmcp has specific typed fields (auth, session) but no extension mechanism for user-defined per-request data." Gap cell for HANDLER-05 reads: "pmcp lacks a peer handle in `RequestHandlerExtra`, so tools cannot initiate server-to-client RPCs from within the handler body." pmcp evidence at `src/shared/cancellation.rs:38-51 [v2.3.0]` confirms the current struct carries only `cancellation_token`, `request_id`, `session_id`, `auth_info`, `auth_context` — no typemap, no peer. rmcp evidence at `crates/rmcp/src/service.rs#L651-L665` (for extensions) and `crates/rmcp/src/service.rs#L382-L390` (for peer) shows both are first-class in `RequestContext<RoleServer>`. Bundling these two High rows into one proposal is justified because they share the exact same edit site (the `RequestHandlerExtra` struct) and benefit from a single coordinated design review; splitting them would force two overlapping reviews of the same struct evolution. Slotting v2.2 rather than late-v2.1 because v2.1 is documentation-polish-focused and this is new runtime surface area — which is the v2.2 vs v2.1 boundary defined in PROJECT.md.

---

## Proposal 2: Typed client call helpers and auto-paginating list-all convenience methods

**Derived from:** `69-RESEARCH.md` Row ID(s): CLIENT-02
**Severity source:** High
**Suggested phase number:** Phase 70
**Target slot:** late v2.1

### Goal

Add typed-input and auto-pagination convenience methods to the pmcp `Client` so writing a correct client is a one-liner per operation instead of a hand-rolled loop. Today `Client::call_tool(name, arguments)` takes an untyped `serde_json::Value`, forcing every caller to manually serialize their typed arg struct and losing compile-time schema coupling, and `Client::list_tools(cursor)` exposes manual cursor pagination with no `list_all_*` convenience, pushing boilerplate onto every client author who needs a full listing. This proposal ships `call_tool_typed<T: Serialize>(name, args: T)`, `call_prompt_typed<T: Serialize>(name, args: T)`, and `list_all_tools` / `list_all_prompts` / `list_all_resources` helpers that internally loop on the cursor, matching the first-hour user expectation of "call a tool with my typed struct" and "give me every tool" as one call each.

### Scope

**In scope:**
- Add `Client::call_tool_typed<T: Serialize>(&mut self, name: impl Into<String>, args: &T) -> Result<CallToolResult>` that serializes `T` via `serde_json::to_value` internally and delegates to the existing `call_tool`
- Add `Client::call_tool_typed_with_task<T: Serialize>` paralleling the existing `call_tool_with_task` so the typed path covers MCP 2025-11-25 task-aware tool calls
- Add `Client::list_all_tools() -> Result<Vec<ToolInfo>>`, `list_all_prompts() -> Result<Vec<PromptInfo>>`, `list_all_resources() -> Result<Vec<ResourceInfo>>` — each internally loops on `next_cursor` with a configurable page-size cap (default 1000, overridable via `ClientOptions`) and a safety limit on total iterations to prevent infinite loops on malformed servers
- Update `examples/c02_client_tools.rs` to showcase both typed call_tool and `list_all_tools`; add an `examples/c08_client_list_all.rs` dedicated to auto-pagination
- Rustdoc on each new method with a `rust,no_run` doctest matching the Phase 66 macro-doc convention

**Out of scope:**
- Tower / service integration (already shipped in Phase 56)
- Transport construction API shape (deferred per D-12)
- Replacing or deprecating the existing `call_tool` / `list_tools` methods — the new helpers are additive; low-level cursor access stays available
- A `ClientNotificationHandler` trait (that addresses CLIENT-03, which is Medium severity and explicitly future work per D-16)
- A client-side `ProgressDispatcher` (CLIENT-04, Medium severity)

### Success Criteria

- [ ] `Client::call_tool_typed::<MyArgs>(...)` compiles in `examples/c02_client_tools.rs` and round-trips against the in-process test server
- [ ] `Client::list_all_tools()` returns all tools across ≥3 paginated pages in an integration test with a server configured to emit small page sizes
- [ ] `list_all_*` helpers enforce a configurable max-iteration safety limit; the test suite documents and verifies the cap (default 100 pages, returns `Error::Validation` on exceed)
- [ ] Both new examples (`c02_client_tools.rs` updated, `c08_client_list_all.rs` new) compile under `cargo check --examples` and are listed in `examples/README.md`
- [ ] `make quality-gate` passes with zero clippy warnings and zero rustdoc warnings on the new `Client` methods

### Suggested Requirement ID

- **PARITY-CLIENT-01**: Ship typed-input `call_tool_typed` / `call_prompt_typed` helpers and auto-paginating `list_all_tools` / `list_all_prompts` / `list_all_resources` convenience methods on `Client`, reducing client boilerplate to one call per operation.

### Estimated Plan Count

3 plans — one for the typed-call helpers and their property tests, one for the `list_all_*` pagination helpers and their integration tests (multi-page fixture server), and one for examples + rustdoc + `examples/README.md` wiring. Multi-day phase sizing similar to Phase 66 (macros doc rewrite) and Phase 67 (docs.rs pipeline) per D-17.

### Rationale / Evidence

Addresses CLIENT-02. Gap cell for CLIENT-02 reads: "pmcp's `Client::call_tool` takes `serde_json::Value` for arguments rather than a typed `CallToolRequestParams`. pmcp also lacks `list_all_*` auto-pagination convenience methods. Clean fix: (a) add typed `call_tool_typed<T: Serialize>(name, args: T)` that serializes internally; (b) add `list_all_tools`, `list_all_prompts`, `list_all_resources` helpers that internally loop on cursor." pmcp evidence at `src/client/mod.rs:416-441 [v2.3.0]` confirms `call_tool(name: String, arguments: Value)`; `src/client/mod.rs:339-340 [v2.3.0]` confirms `list_tools` is manual-cursor; `examples/c02_client_tools.rs:35 [v2.3.0]` demonstrates the current manual-serialization pattern. rmcp evidence at `crates/rmcp/src/service/client.rs#L378-L395` shows `list_all_tools` auto-paginates. Slotting late-v2.1 because this is an additive, non-breaking extension to an existing API — it fits the v2.1 "close DX gaps" charter without introducing new surface that would belong in v2.2.

---

## Proposal 3: Rustdoc fallback for `#[mcp_tool]` tool descriptions

**Derived from:** `69-RESEARCH.md` Row ID(s): MACRO-02
**Severity source:** High
**Suggested phase number:** Phase 71
**Target slot:** late v2.1

### Goal

Enable `#[mcp_tool]` to harvest the attached function's rustdoc as the tool description when the `description = "..."` attribute is omitted, eliminating the forced duplication where a well-documented tool fn must repeat its description in both the rustdoc block and the macro attribute. Today pmcp-macros 0.5.0 rejects `#[mcp_tool]` at compile time with "mcp_tool requires at least `description = \"...\"` attribute" even when the function carries a multi-paragraph rustdoc — a first-hour friction point for users migrating from idiomatic Rust (where rustdoc is the single source of truth for descriptions). This proposal adds a rustdoc-harvest fallback while preserving the explicit-attribute path, with a clear precedence rule (attribute wins over rustdoc), a new `trybuild` compile-fail snapshot confirming the error message when neither is present, and a migration note in the pmcp-macros README pointing existing users to the shorter form.

### Scope

**In scope:**
- Extend `#[mcp_tool]` attribute parsing in `pmcp-macros/src/mcp_tool.rs` to inspect `#[doc = "..."]` attributes on the decorated `fn`/`async fn` when `description` is absent
- Concatenate multi-line rustdoc blocks into a single description string with normalized whitespace (trim leading `/// `, join with `\n`, strip trailing blank lines)
- Preserve precedence: explicit `description = "..."` wins over rustdoc when both are present; neither present → compile-fail with a clear message naming both options ("provide `description = \"...\"` attribute or a rustdoc comment")
- Add `trybuild` compile-fail snapshot `pmcp-macros/tests/ui/mcp_tool_missing_description_and_rustdoc.rs` for the "neither present" case, paralleling the existing `mcp_tool_missing_description.rs`
- Update `pmcp-macros/README.md` MACR-02 migration guide with a new section "Rustdoc-derived descriptions (pmcp-macros 0.6.0+)" showing the before/after pattern
- Ship one new compiling `rust,no_run` doctest in the pmcp-macros README demonstrating rustdoc-only usage
- Version bump plan: pmcp-macros 0.6.0 (minor — new capability, backwards-compatible), and the downstream pmcp crate dep bump only if pmcp needs to pin the new minor for doctest/example use

**Out of scope:**
- Tower / service integration (already shipped in Phase 56)
- Transport construction API shape (deferred per D-12)
- Rustdoc-harvest for other macros — `#[mcp_prompt]` / `#[mcp_resource]` / `#[mcp_server]` stay attribute-required in this phase (separate follow-on if the user demand materializes; not a High row in 69-RESEARCH.md)
- Schema-override attribute (MACRO-04, Medium severity — explicit future work per D-16)
- Changing the input-argument schema derivation path (out of scope for a description-only change)

### Success Criteria

- [ ] A `#[mcp_tool]` fn with a rustdoc block and no `description` attribute compiles and produces a tool whose description equals the normalized rustdoc text
- [ ] A `#[mcp_tool]` fn with both a rustdoc block and an explicit `description = "..."` uses the attribute (verified by a unit test asserting the emitted description field byte-for-byte)
- [ ] A `#[mcp_tool]` fn with neither rustdoc nor `description` fails with the updated error message; the new `trybuild` snapshot locks the message text against regression
- [ ] Every existing `#[mcp_tool]` usage site in `examples/` and `pmcp-macros/tests/` compiles unchanged with pmcp-macros 0.6.0 — backwards compatible (≥100 call-sites verified via `cargo check --workspace --examples`)
- [ ] `make quality-gate` passes with zero clippy warnings and all `trybuild` UI tests green, including the new compile-fail case

### Suggested Requirement ID

- **PARITY-MACRO-01**: Support rustdoc as a fallback source for `#[mcp_tool]` descriptions, so well-documented tool functions do not have to repeat themselves in the macro attribute.

### Estimated Plan Count

3 plans — one for the attribute-parsing change + unit tests (covering precedence and whitespace normalization), one for the `trybuild` compile-fail snapshot + README migration guide, and one for version bump (pmcp-macros 0.6.0 + dependent pmcp pin update if needed) + CI verification. Multi-day phase sizing similar to Phase 66 (macros doc rewrite) and Phase 67 (docs.rs pipeline) per D-17.

### Rationale / Evidence

Addresses MACRO-02. Gap cell for MACRO-02 reads: "pmcp requires explicit `description`; rmcp derives from rustdoc when omitted — pmcp users writing rustdoc must also repeat the description in the attribute. Clean fix: accept rustdoc fallback when `description` is absent and fail only if neither is present." pmcp evidence at `pmcp-macros/src/mcp_tool.rs:72-75 [v2.3.0]` confirms the current hard rejection when `description` is missing. rmcp evidence at `crates/rmcp-macros/src/lib.rs#L19-L22` documents the rustdoc-harvesting fallback: the attribute table states "A description of the tool. The document of this function will be used." This is the narrowest, highest-leverage macro fix in the High-severity set: a single attribute-parsing change with clear precedence semantics closes a first-hour user friction without altering any existing compile-fail snapshot. Slotting late-v2.1 because it lives entirely inside pmcp-macros (already the subject of Phase 66's rewrite) and qualifies as additional DX polish consistent with the v2.1 charter.

---

**Validated:** All 3 proposals cross-checked against 69-RESEARCH.md High Row IDs on 2026-04-16 (Task 2 validation sweep).

- Template completeness: every proposal has all 6 required subsections (Goal, Scope with In/Out, Success Criteria, Suggested Requirement ID, Estimated Plan Count, Rationale / Evidence).
- Goal sentence-1 verbs: Proposal 1 "Extend", Proposal 2 "Add", Proposal 3 "Enable" — all functional-capability verbs per D-18.
- Single REQ-ID per proposal: PARITY-HANDLER-01, PARITY-CLIENT-01, PARITY-MACRO-01 — unique across the document, each appearing exactly twice (Summary table row + Suggested Requirement ID block).
- Row-ID bijection: all 4 High Row IDs in 69-RESEARCH.md (MACRO-02, HANDLER-02, HANDLER-05, CLIENT-02) appear in at least one proposal's Derived-from line AND Rationale subsection. No fabricated Row IDs. Non-High Row IDs (e.g., CLIENT-03, CLIENT-04, MACRO-04) appear only in Out-of-scope bullets as explicit exclusions referencing the Medium-severity future-work classification per D-16.
- Plan counts: 4, 3, 3 — all within {3, 4, 5} per D-17.
- Success Criteria bullet counts: 5, 5, 5 — all within the 3–5 range.
- No forbidden phrasing ("adopt rmcp's", "copy rmcp's") — every fix is framed in pmcp-native terms per REQUIREMENTS.md Out-of-Scope rule.

**Note to Plan 03:** The Task 2 `<verify>` Python block in this plan has a regex bug — its RESEARCH.md row matcher `^\| ... \| (High|Medium|Low) *$` misses the trailing `|` delimiter on markdown pipe-table rows (actual line endings are `| High |`). When Plan 03 re-runs this bijection check, it must allow an optional trailing `|` (e.g., `\| (High|Medium|Low) \|?\s*$`) to correctly extract the matrix rows. A manual run with the corrected regex confirms 4/4 High Row IDs cited → bijection OK.
