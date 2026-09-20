---
phase: 117-agents-tester-v1-severability
plan: 14
subsystem: infra
tags: [rust, cargo-features, streamable-http, severability, client-transport, sse-resumability, session-lifecycle, last-event-id, mcp-2026-07-28]

# Dependency graph
requires:
  - phase: 117-12
    provides: "the SERVER's `Last-Event-ID` reader moved into `v1_session.rs`, leaving the client transport holding the LAST reader of `LAST_EVENT_ID` in the crate — this plan's opening move"
  - phase: 117-06
    provides: "`tests/v1_severability_tripwire.rs` with `repo_root()`, `rel()`, `source()` and the comment stripper this plan EXTENDS rather than duplicates"
  - phase: 117-01
    provides: "the `v1-compat` marker feature and the parallel `full-v2` list that make `--no-default-features --features full-v2` a real severance proof"
provides:
  - "a client transport whose v1 session lifecycle and SSE-resumability surface does not exist on `full-v2`: no stored session id, no `session_id()`/`set_session_id()`, no `Mcp-Session-Id` capture, no DELETE construction site, no `Last-Event-ID` writer"
  - "`LAST_EVENT_ID` gated in `http_constants.rs` together with its one remaining reader — nothing in `pmcp` names `Last-Event-ID` on a `full-v2` build, server OR client"
  - "assumption A4 MEASURED FALSE, with the trace recorded in `MCP_SESSION_ID`'s own doc: the const stays UNGATED and that is now a documented decision rather than an oversight"
  - "a DERIVED, self-tested, counter-example-bearing gate-region scanner in `tests/v1_severability_tripwire.rs` (15 tests, up from 11)"
  - "`tests/v2_client_carries_no_session_on_severed_build.rs` — a runtime proof that RAN on the severed build (2 tests, non-zero count)"
  - "`crates/pmcp-code-mode` no longer unifies `pmcp`'s `default` (hence `v1-compat`) back on, which is what made a severed TEST target possible at all"
affects: [117-13, SMPL-F1]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Paired ACCESSORS on a single type, not just paired modules: `outbound_session()`, `capture_session_header()`, `terminate_session()`, `resumption_callback()` each have a `v1-compat` half and a constant-answer twin, so the request pipeline carries ZERO `#[cfg]` at its call sites"
    - "Same-arity parameter gating: `#[cfg(feature = \"v1-compat\")] resumption_token: Option<String>` paired with `#[cfg(not(...))] _ignored_cursor: Option<String>` keeps a public method's ARITY stable across feature sets, so no caller needs a `#[cfg]`"
    - "`#[cfg_attr(feature = \"v1-compat\", doc = r#\"...\"#)]` makes a DOCTEST era-aware. A `#[cfg]` written INSIDE a doctest body is silently always-false (doctests are separate crates with no features), so it strips the example instead of gating it"
    - "A dev-dependency taking a crate's DEFAULT features silently un-severs that crate's own severance tests. The proof build (`cargo build -p pmcp`) never sees dev-deps; the proof TEST does"

key-files:
  created:
    - tests/v2_client_carries_no_session_on_severed_build.rs
  modified:
    - src/shared/streamable_http.rs
    - src/shared/http_constants.rs
    - src/client/mod.rs
    - src/composition/mcp_client.rs
    - crates/pmcp-code-mode/Cargo.toml
    - tests/v1_severability_tripwire.rs

key-decisions:
  - "Assumption A4 is FALSE. `MCP_SESSION_ID` stays UNGATED, with the call-path trace pasted into its own rustdoc. Its readers `extract_session_and_protocol_headers` (every POST, fast path AND middleware path) and `build_middleware_context` are v2-reachable, and the v2 test surface names the constant precisely to assert its ABSENCE. What is severed is the STORING and SENDING of a session id, not the header's name."
  - "`Client::initialize` was NOT gated — DOCUMENTED FALLBACK with the compiler output recorded. Two independent triggers: (1) `src/composition/mcp_client.rs:181` calls it and `composition` is in `full-v2`, so propagating the gate would mean a composition connection reporting `initialized: true` without a handshake — a semantic change to a subsystem this plan has no mandate over; (2) the method is DUAL-era, not v1-only — its `is_v2()` branch is a deliberate Phase-113 no-op affordance, so gating it deletes v2 behaviour. SMPL-01's 'initialize' clause is met on the SERVER side only."
  - "`src/composition/mcp_client.rs` now builds its transport config through `StreamableHttpTransportConfigBuilder` instead of a struct literal. It is the ONLY in-lib construction site and `composition` is in `full-v2`, so a literal naming the gated fields could not compile. A file-scope extension, recorded below."
  - "`crates/pmcp-code-mode/Cargo.toml` now takes `pmcp` with `default-features = false, features = [\"logging\"]`. As a dev-dependency of `pmcp` it was unifying `default = [\"logging\", \"v1-compat\"]` back on for every `cargo test` of `pmcp`, so the severed test target compiled away and reported `0 tests, exit 0` — a severance proof that silently proved nothing. A second file-scope extension, recorded below."
  - "`apply_resumption_header` has NO null twin, deliberately. A twin returning `Ok(())` is indistinguishable from absence and only tempts a later author to 'improve' it by logging the ignored cursor. On `full-v2` the function does not exist and its one call site carries the file's ONLY call-site `#[cfg]`."
  - "The transport's `last_event_id` field and its `last_event_id()` accessor are deliberately left UNGATED — a measured limitation, recorded in the HANDOFF below."

patterns-established:
  - "A tripwire scanner must be unit-tested in BOTH directions on a fixture that includes a `not(feature = ...)` null twin and a `rustfmt`-WRAPPED signature. The wrapped-signature case is not hypothetical: the scanner's first run against the real transport reported three false positives for exactly that reason"
  - "A `0 tests` run of a whole-file-`cfg` test is a FAILURE dressed as a pass. Ship a trivial always-true test in the same file whose only job is to make the count non-zero"

requirements-completed: [SMPL-01, SMPL-02]

# Metrics
duration: 130min
completed: 2026-08-08
---

# Phase 117 Plan 14: Sever the CLIENT half of the v1 session lifecycle and SSE resumability Summary

**The `full-v2` client now stores no session id, echoes none back, sends no DELETE and writes no `Last-Event-ID` — proven by a derived source scan with a live counter-example and by a runtime test that actually RAN on the severed build, with three executed negative controls.**

## Performance

- **Duration:** 130 min
- **Tasks:** 3
- **Files created:** 1 · **Files modified:** 6 (2 of them file-scope extensions, recorded below)
- **Net:** +1612 / −144 across the plan's three commits

## Task Commits

1. **Task 1: derive the client inventory, gate the resumability surface, measure A4** — `dd62342f` (refactor)
2. **Task 2: gate the client session lifecycle, measure `Client::initialize`** — `70d92d16` (refactor)
3. **Task 3: derived client-inventory tripwire plus a severed-build runtime proof** — `40eb06b5` (test)

## Verification

| Command | Result |
|---|---|
| `cargo build -p pmcp --features full` | exit 0 |
| `RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2` | exit 0, **zero `warning:` lines** |
| `cargo build -p pmcp --no-default-features --features "streamable-http"` | exit 0 (T-117-54, asserted separately) |
| `cargo test --lib --features "full"` | **1880 passed** — EXACTLY the pre-plan count |
| `cargo test --test v1_severability_tripwire` | **15 passed**, 0 failed (floor: 13) |
| `cargo test --test v2_client_carries_no_session_on_severed_build --no-default-features --features full-v2` | **2 passed** — NON-ZERO |
| `cargo test --test v1_byte_identity_after_cut --features "full"` | 9 passed — the v1 client wire is unchanged |
| `cargo build -p pmcp-agent --all-features` (absolute rustup cargo) | exit 0 |
| `cargo test -p mcp-tester --test report_compat` | 7 passed |
| `cargo test --doc --features full` | 447 passed, 79 ignored |
| `make doc-check` | exit 0, zero rustdoc warnings |
| `make lint` | exit 0 |
| `make quality-gate` | **exit 0** |

Pre-plan lib baseline recorded BEFORE any edit: `cargo test: 1880 passed`. Post-plan: `cargo test: 1880 passed`. An exact match, so no test was silently dropped.

---

## 1. The derived inventory

### Grep 1 — `src/shared/streamable_http.rs`, run BEFORE the cut

```
$ grep -n 'session_id\|resumption_token\|LAST_EVENT_ID\|Last-Event-ID\|set_session_id' src/shared/streamable_http.rs
3:    ACCEPT, ACCEPT_STREAMABLE, APPLICATION_JSON, CONTENT_TYPE, LAST_EVENT_ID, MCP_METHOD, MCP_NAME,
33:/// assert!(opts.resumption_token.is_none());
38:///     resumption_token: None,
44:///     resumption_token: Some("event-456".to_string()),
52:    pub resumption_token: Option<String>,
68:///     session_id: None,
70:///     on_resumption_token: None,
81:///     session_id: Some("session-123".to_string()),
83:///     on_resumption_token: None,
92:///     session_id: None,
94:///     on_resumption_token: None,
107:    pub session_id: Option<String>,
111:    pub on_resumption_token: Option<Arc<dyn Fn(String) + Send + Sync>>,
122:            .field("session_id", &self.session_id)
124:            .field("on_resumption_token", &self.on_resumption_token.is_some())
162:    session_id: Option<String>,
164:    on_resumption_token: Option<Arc<dyn Fn(String) + Send + Sync>>,
174:            .field("session_id", &self.session_id)
176:            .field("on_resumption_token", &self.on_resumption_token.is_some())
192:            session_id: None,
194:            on_resumption_token: None,
212:    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
213:        self.session_id = Some(session_id.into());
224:    pub fn on_resumption_token(mut self, callback: Arc<dyn Fn(String) + Send + Sync>) -> Self {
225:        self.on_resumption_token = Some(callback);
272:            session_id: self.session_id,
274:            on_resumption_token: self.on_resumption_token,
582:    pub fn session_id(&self) -> Option<String> {
583:        self.config.read().session_id.clone()
587:    pub fn set_session_id(&self, session_id: Option<String>) {
588:        self.config.write().session_id = session_id;
607:    pub async fn start_sse(&self, resumption_token: Option<String>) -> Result<()> {
636:        // Add Last-Event-ID for resumability
637:        if let Some(token) = &resumption_token {
639:                LAST_EVENT_ID,
697:        let on_resumption = self.config.read().on_resumption_token.clone();
785:        let (extra_headers, auth_provider, session_id, middleware_chain) = {
790:                config.session_id.clone(),
821:        if let Some(session_id) = &session_id {
823:                request_builder = request_builder.header(MCP_SESSION_ID, session_id.as_str());
944:            if let Some(session_id) = response.headers().get(MCP_SESSION_ID) {
945:                if let Ok(session_id_str) = session_id.to_str() {
946:                    self.config.write().session_id = Some(session_id_str.to_string());
966:        if let Some(token) = options.resumption_token {
1337:            let on_resumption = self.config.read().on_resumption_token.clone();
1412:        if let Some(_session_id) = self.session_id() {
1428:            self.config.write().session_id = None;
1976:        fn v2_transport(session_id: Option<&str>) -> StreamableHttpTransport {
1981:            config.session_id = session_id.map(str::to_string);
1988:        fn v1_transport(session_id: Option<&str>) -> StreamableHttpTransport {
1993:            config.session_id = session_id.map(str::to_string);
2163:        async fn v2_never_emits_a_stored_session_id() {
2173:        fn v2_does_not_store_a_session_id_from_a_response() {
2182:                transport.session_id(),
2189:        fn v1_still_stores_a_session_id_from_a_response() {
2197:            assert_eq!(transport.session_id().as_deref(), Some("kept"));
```

### Grep 2 — every `LAST_EVENT_ID` reader in the repo

```
$ grep -rn 'LAST_EVENT_ID' --include='*.rs' src/ crates/ tests/ examples/ cargo-pmcp/
src/server/streamable_http_server.rs:492:// `LAST_EVENT_ID` constant in `crate::shared::http_constants`.
src/server/streamable_http_server.rs:503:// that edit belongs to 117-13. `LAST_EVENT_ID` is the same shape: its two readers
src/server/streamable_http_server.rs:4184:    // `LAST_EVENT_ID` is imported HERE rather than at file scope since plan
src/server/streamable_http_server.rs:4189:    use crate::shared::http_constants::LAST_EVENT_ID;
src/server/streamable_http_server.rs:5608:                &v2_post_headers("tools/list", &[(LAST_EVENT_ID, "12345")]),
src/server/streamable_http_server.rs:5631:            (LAST_EVENT_ID, "evt-1"),
src/server/streamable_http_server.rs:5650:            (LAST_EVENT_ID, "evt-1"),
src/server/streamable_http_server/v1_session.rs:75:use crate::shared::http_constants::{LAST_EVENT_ID, MCP_SESSION_ID};
src/server/streamable_http_server/v1_session.rs:681:// build at all, and with them goes the ONLY reader of `LAST_EVENT_ID` in the
src/server/streamable_http_server/v1_session.rs:770:    let Some(last_event_id) = headers.get(LAST_EVENT_ID) else {
src/server/streamable_http_server/v1_session_off.rs:407:/// the `LAST_EVENT_ID` token for exactly that reason.
src/shared/http_constants.rs:34:pub const LAST_EVENT_ID: &str = "Last-Event-ID";
src/shared/streamable_http.rs:3:    ACCEPT, ACCEPT_STREAMABLE, APPLICATION_JSON, CONTENT_TYPE, LAST_EVENT_ID, MCP_METHOD, MCP_NAME,
src/shared/streamable_http.rs:639:                LAST_EVENT_ID,
crates/mcp-tester/src/era_observations.rs:190:pub const HEADER_LAST_EVENT_ID: ObservationId = ObservationId("header.last_event_id");
crates/mcp-tester/src/era_observations.rs:224:    HEADER_LAST_EVENT_ID,
crates/mcp-tester/src/era_observations.rs:335:        HEADER_LAST_EVENT_ID,
tests/v1_byte_identity_after_cut.rs:73:use pmcp::shared::http_constants::{LAST_EVENT_ID, MCP_SESSION_ID};
tests/v1_byte_identity_after_cut.rs:824:const UNKNOWN_LAST_EVENT_ID: &str = "00000000-0000-4000-8000-000000000000";
tests/v1_byte_identity_after_cut.rs:894:            header(LAST_EVENT_ID, UNKNOWN_LAST_EVENT_ID),
tests/v1_severability_tripwire.rs:319:    "LAST_EVENT_ID",
```

### Grep 3 — every session/resumption API site in the repo

```
$ grep -rn 'with_session_id\|set_session_id\|on_resumption_token\|resumption_token' --include='*.rs' src/ crates/ tests/ examples/ cargo-pmcp/
src/server/tool_middleware.rs:114,588,706          (ToolContext::with_session_id — UNRELATED type)
src/server/cancellation.rs:292,917                 (RequestHandlerExtra::with_session_id — UNRELATED type)
src/server/dynamic_resources.rs:154,389            (UNRELATED type)
src/server/observability/types.rs:184,386          (UNRELATED type)
src/server/observability/mod.rs:220                (UNRELATED type)
src/server/observability/middleware.rs:128,130     (UNRELATED type)
src/shared/cancellation.rs:100                     (DEAD ORPHAN module — never compiled)
src/shared/context.rs:31,186,496                   (UNRELATED type)
src/shared/event_store.rs:37,40,189,216,292,299    (create/validate_resumption_token — SERVER event store, plan 117-13)
src/shared/streamable_http.rs:33,38,44,52,70,83,94,111,124,164,176,194,212,224,225,274,587,607,637,697,966,1337
src/composition/mcp_client.rs:158                  (on_resumption_token: None — IN-LIB struct literal)
crates/pmcp-workbook-server/tests/http_smoke.rs:57
crates/mcp-tester/src/tester.rs:135,195,4056
crates/pmcp-sql-server/tests/http_lazy_startup.rs:111
crates/pmcp-openapi-server/tests/http_smoke.rs:88
cargo-pmcp/tests/team_dev.rs:435
tests/streamable_http_spec_compliance.rs:164,241,315,404,452,539,577,624,656
tests/web_channel_long_task_http.rs:362
tests/streamable_http_integration.rs:59,135,231,309
tests/transport.rs:44
tests/tool_output_result_http.rs:119
tests/session_validation_tests.rs:47,83,110,142,174,278,354,460,492,516,547,612
tests/v2_tasks_client.rs:203
tests/workflow_prompt_e2e_test.rs:88
tests/sse_middleware_integration.rs:120,225,315,401
tests/streamable_http_server_tests.rs:44,104,164,239
tests/streamable_http_oauth_integration.rs:89,159,223,277,341
tests/streamable_http_unit_tests.rs:30,37,40,53,64,75,94,102,106,118,138,156,174,193,210,231,251,256,263,269
tests/tool_as_task_lifecycle_http.rs:153
tests/test_cancellation.rs:96
examples/t06_streamable_http_client.rs:58
examples/26-server-tester/src/tester.rs:104,164
examples/s47_task_augmented_result.rs:182
examples/s46_http_tool_as_task.rs:129
examples/25-oauth-basic/test-client.rs:29
```

(Grep 3's raw output is 131 lines; the block above preserves every path and every
line number, grouped by bucket. Nothing was dropped.)

### The three-bucket classification

Every hit from the three greps is in exactly one bucket.

| # | Site(s) | Bucket | Disposition |
|---|---------|--------|-------------|
| 1 | `streamable_http.rs:52` `SendOptions::resumption_token` | (a) v1-only client | GATED |
| 2 | `:107` `StreamableHttpTransportConfig::session_id` | (a) | GATED |
| 3 | `:111` `::on_resumption_token` | (a) | GATED |
| 4 | `:122`, `:124`, `:174`, `:176` the two `Debug` impls | (a) | Moved into paired `debug_v1_fields`, GATED |
| 5 | `:162`, `:164` builder fields | (a) | GATED |
| 6 | `:192`, `:194` builder defaults; `:272`, `:274` `build()` | (a) | GATED (field-level `#[cfg]` in struct expressions) |
| 7 | `:212-213` `with_session_id`; `:224-225` `on_resumption_token` | (a) | GATED |
| 8 | `:582-588` `session_id()` / `set_session_id()` | (a) | GATED (public-API change, `full-v2` only) |
| 9 | `:636-647` the `Last-Event-ID` writer | (a) | Extracted to `apply_resumption_header`, GATED with the const in ONE edit |
| 10 | `:697`, `:1337` `on_resumption_token` reads | (a) | Routed through paired `resumption_callback()`; twin returns `None` |
| 11 | `:785-790`, `:821-823` outbound session header | (a) | Routed through paired `outbound_session()`; twin returns `None` |
| 12 | `:944-946` the `Mcp-Session-Id` capture | (a) | Routed through paired `capture_session_header()`; twin is a no-op `const fn` |
| 13 | `:966` `options.resumption_token` in `send_with_options` | (a) | Routed through paired `SendOptions::resumption_cursor()` |
| 14 | `:1412-1428` the DELETE teardown | (a) | Routed through paired `terminate_session()`; twin has NO DELETE construction site |
| 15 | `:607` `start_sse`'s cursor parameter | (a) | GATED per-half at the SAME position and type (`_ignored_cursor` on `full-v2`) |
| 16 | `:3` the file-scope `LAST_EVENT_ID` import | (a) | Split into a separate GATED `use` |
| 17 | `http_constants.rs:34` `LAST_EVENT_ID` | (a) | GATED |
| 18 | `http_constants.rs:12` `MCP_SESSION_ID` | (b) SHARED with v2 | UNGATED — A4 measured FALSE, trace in §2 |
| 19 | `http_constants.rs:23`, `:31` `MCP_METHOD` / `MCP_NAME` | (b) | UNGATED — v2-REQUIRED (VERS-05). Live counter-example in the tripwire |
| 20 | `server/streamable_http_server.rs:492`, `:503`, `:4184` | (c) comments | Unaffected |
| 21 | `server/streamable_http_server.rs:4189`, `:5608`, `:5631`, `:5650` | (c) `#[cfg(test)]` | Unaffected — lib-only build compiles no `#[cfg(test)]` |
| 22 | `v1_session.rs:75`, `:770` | (b) already severed | Gated BY THE MODULE (117-12). Untouched |
| 23 | `v1_session_off.rs:407` | (c) doc prose | Unaffected |
| 24 | `shared/event_store.rs` `create/validate_resumption_token` | (b) SERVER event store | Out of scope — plan **117-13** owns it (`StreamableHttpServerConfig::event_store`) |
| 25 | `crates/mcp-tester/src/era_observations.rs` `HEADER_LAST_EVENT_ID` | (c) | mcp-tester's OWN `ObservationId`, unrelated to the const |
| 26 | `src/composition/mcp_client.rs:158` | **(a′) IN-LIB struct literal** | **FINDING** — see §5 |
| 27 | `src/server/{tool_middleware,cancellation,dynamic_resources,observability/*}.rs`, `src/shared/context.rs` `with_session_id` | (b) different types entirely | UNGATED — same method NAME on unrelated types |
| 28 | `src/shared/cancellation.rs:100` | (c) | DEAD ORPHAN — not declared in `src/shared/mod.rs`, never compiled on any target |
| 29 | ~110 sites under `tests/`, `examples/`, `crates/`, `cargo-pmcp/` | (c) lib-only irrelevant | Unaffected — `grep -c '^\[\[bin\]\]' Cargo.toml` is **0**, so the severance build compiles the LIBRARY target only. Zero downstream files edited |

**Findings the plan's pre-cut line numbers did NOT list** (the 116-14 failure mode, caught by
deriving rather than enumerating):

1. **`src/composition/mcp_client.rs:158`** — an IN-LIB struct literal of
   `StreamableHttpTransportConfig`. `composition` is in `full-v2`, so this is NOT covered by
   "the severance build is lib-only". Resolved in §5.
2. **`StreamableHttpTransport::last_event_id` (the field and the `pub fn last_event_id()`)** —
   ungated resumability STATE that grep 1's token set did not surface (lowercase, so it does not
   match `LAST_EVENT_ID`). Left UNGATED as a measured limitation; see the HANDOFF.
3. **`src/shared/cancellation.rs`** is a dead orphan — confirming the note already in project
   memory. It compiles on no target, so its `with_session_id` is not surface at all.

---

## 2. Assumption A4: MEASURED, verdict FALSE

The plan's `<interfaces>` pointed at `src/server/streamable_http_server.rs:3629`. That line number
is stale post-117-12; the derived grep found the real reader set:

```
$ grep -rn 'MCP_SESSION_ID' --include='*.rs' src/ | grep -v '_session'
src/server/streamable_http_server.rs:1975:        .get(MCP_SESSION_ID)
src/server/streamable_http_server.rs:3368:        .get_header(MCP_SESSION_ID)
src/server/streamable_http_server.rs:4075:        .get(MCP_SESSION_ID)
src/server/streamable_http_server.rs:4132:        .get(MCP_SESSION_ID)
```

### The call-path trace

```
:1975  fn extract_session_and_protocol_headers(headers: &HeaderMap)
           -> (Option<String>, Option<String>)
       /// Shared by both the fast path and middleware-path POST handlers so the
       /// two entry points read the same two headers in the same way.
       callers:
         :3248  handle_post (fast path)          ← EVERY POST, v1 AND v2
         :3922  handle_post_with_middleware_*    ← EVERY POST, v1 AND v2
       It is also the read that yields MCP-Protocol-Version, which v2 NEEDS.

:3368  fn build_middleware_context(server_request: &ServerHttpRequest)
           -> ServerHttpContext
       caller:
         :3897  (pre-dispatch middleware setup)
           ← handle_post_with_middleware_inner   ← EVERY POST on the middleware
                                                   path, and plan 117-13 keeps
                                                   `http_middleware` a SHARED,
                                                   UNGATED config field

:4075  handle_get_sse        ← after `v2_verb_rejection`, so v1-reachable only
:4132  handle_delete_session ← after `v2_verb_rejection`, so v1-reachable only
```

**Verdict: A4 is FALSE.** Two of the four readers are on the shared v2 POST path, and one of them
(`extract_session_and_protocol_headers`) cannot be gated without also gating the
`MCP-Protocol-Version` read that v2 depends on. A third consideration is decisive on its own: the
v2 test surface (`tests/v2_client.rs:139`, `tests/v2_mrtr.rs:2088`, `tests/common/v2.rs:781`,
`crates/mcp-tester/src/era_observations.rs`) reads the constant precisely to assert its ABSENCE —
gating it would delete the vocabulary v2 needs to state "no session header was sent".

**Decision, following the verdict:** `MCP_SESSION_ID` is left **UNGATED**, and the trace above is
recorded verbatim in the constant's own rustdoc so the next reader does not re-litigate it.
`src/server/streamable_http_server.rs` was therefore **NOT** edited, and no scope extension was
needed for the A4 branch.

---

## 3. Call-site `#[cfg]`s added

Exactly **one**, in `src/shared/streamable_http.rs`:

```rust
        // Add the SSE resumption cursor, on the builds that have one.
        //
        // Why: this is the ONLY `#[cfg]` at a CALL SITE in this file. It is
        // unavoidable because the argument it reads is itself gated (see this
        // method's doc): on `full-v2` the parameter is `_ignored_cursor` and
        // `apply_resumption_header` does not exist. Every other v1 read on this
        // transport goes through a paired accessor with a constant `full-v2`
        // answer instead — do NOT let a second one accumulate here.
        #[cfg(feature = "v1-compat")]
        Self::apply_resumption_header(&mut request, resumption_token.as_deref())?;
```

Every other v1 read (12 sites) is routed through one of five paired accessors, exactly as
117-09/117-12 did on the server:

| Accessor | `v1-compat` half | `full-v2` twin |
|---|---|---|
| `SendOptions::resumption_cursor()` | clones the field | `const fn … { None }` |
| `StreamableHttpTransport::resumption_callback()` | clones the config callback | `const fn … { None }` |
| `StreamableHttpTransport::outbound_session()` | clones the stored session id | `const fn … { None }` |
| `StreamableHttpTransport::capture_session_header()` | `is_v2()` guard, then stores | `const fn … {}` — names no header |
| `StreamableHttpTransport::terminate_session()` | builds + sends the DELETE | `async fn … { Ok(()) }` — no DELETE site |
| `*::debug_v1_fields()` (×2) | renders the two v1 fields | `const fn … {}` |

`grep -c '#[cfg(feature = "v1-compat")]'` in the transport = **24** (all on items, fields or
parameters, except the one above). `grep -c '#[cfg(not(feature = "v1-compat"))]'` = **8** (the
twins).

---

## 4. Per-doctest route taken

| Doctest | Route |
|---|---|
| `SendOptions` — main example (`:26-45`) | **Rewritten** to compile on BOTH configurations, using `..SendOptions::default()` functional-update syntax so no gated field is named |
| `SendOptions` — resumption example | **`#[cfg]`-aware** via `#[cfg_attr(feature = "v1-compat", doc = r#"…"#)]` — a full working example, preserved, compiled only when the feature is on |
| `StreamableHttpTransportConfig` — "minimal" (`:64-73`) | **Rewritten** through `StreamableHttpTransportConfigBuilder`; compiles on both |
| `StreamableHttpTransportConfig` — "JSON instead of SSE" (`:87-97`) | **Rewritten** through the builder; compiles on both. Merged the `X-API-Key` header from the deleted middle example so no coverage was lost |
| `StreamableHttpTransportConfig` — "with session" (`:75-86`) | **`#[cfg]`-aware** via `cfg_attr(…, doc = …)`, now exercising `with_session_id` and asserting the result |
| `StreamableHttpTransportConfigBuilder` (`:220-240`) | **Untouched** — names no gated field |
| `with_http_middleware` (`:334-...`) | **Untouched** |

**No doctest was deleted.** `git diff` adds two ` ``` ` fences net (the two `cfg_attr` examples) and
removes none without a replacement. `cargo test --doc --features full`: **447 passed, 79 ignored**.

A `#[cfg(feature = "v1-compat")]` written INSIDE a doctest body would have been the wrong route and
is worth recording: doctests compile as separate crates that merely LINK `pmcp`, so the predicate
evaluates against the doctest crate's (empty) feature set and is always FALSE — the example would be
silently stripped rather than gated. `cfg_attr` on the `doc` attribute evaluates in `pmcp`'s own
compilation, which is what makes it correct.

---

## 5. `Client::initialize`: MEASURED, fallback taken

The gate was actually applied and the build actually run.

```
$ # (with `#[cfg(feature = "v1-compat")]` applied to src/client/mod.rs:551)
$ RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2
   Compiling pmcp v2.18.0 (/Users/guy/Development/mcp/sdk/rust-mcp-sdk)
error: unused import: `InitializeRequest`
  --> src/client/mod.rs:23:56
   |
23 |     GetPromptRequest, GetPromptResult, Implementation, InitializeRequest, InitializeResult,
   |                                                        ^^^^^^^^^^^^^^^^^
   |
   = note: `-D unused-imports` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(unused_imports)]`

error[E0599]: no method named `initialize` found for struct `client::Client<T>` in the current scope
   --> src/composition/mcp_client.rs:181:16
    |
181 |         client.initialize(capabilities).await.map_err(|e| {
    |                ^^^^^^^^^^ method not found in `client::Client<StreamableHttpTransport>`
    |
   ::: src/client/mod.rs:302:1
    |
302 | pub struct Client<T: Transport> {
    | ------------------------------- method `initialize` not found for this struct
    |
    = help: items from traits can only be used if the trait is implemented and in scope
    = note: the following traits define an item `initialize`, perhaps you need to implement one of them:
            candidate #1: `lazy_static::LazyStatic`
            candidate #2: `tokio_stream::stream_ext::collect::sealed::FromStreamPriv`

For more information about this error, try `rustc --explain E0599`.
error: could not compile `pmcp` (lib) due to 2 previous errors
```

**Verdict: FALLBACK.** Error 1 is trivially resolvable (gate the import). Error 2 is not, for two
independent reasons:

1. **`composition` is in `full-v2`.** Propagating the gate to `src/composition/mcp_client.rs`
   means `create_connection` skips the handshake yet still returns
   `FoundationConnection { initialized: true }` — a connection that reports itself initialized
   without ever having initialized. That is a SEMANTIC change to a subsystem this plan has no
   mandate over, not a `#[cfg]` propagation. Whether `composition` should be v1-only at all is a
   FEATURE-LIST question owned by 117-01 / 117-13.
2. **`Client::initialize` is DUAL-era, not v1-only.** Its first statement is
   `if self.is_v2() { self.initialized = true; return Ok(Self::v2_synthetic_initialize_result()); }`
   — a deliberate Phase-113 affordance that sends nothing and exists "so existing v1-shaped
   application code keeps compiling when it opts into v2". Gating the method deletes v2 behaviour,
   not just v1 behaviour. This reason stands even if reason 1 were resolved.

The `initialize` gate was **reverted**; `git diff --stat src/client/mod.rs` shows doc-only changes
(+24 lines, the `# Severability: this method is NOT gated, and that is measured` section which
records both triggers at the call site a reader will actually reach). `git diff src/client/mod.rs`
contains **zero** additions matching `server_discover`, `probe` or `detect` — A-D08 holds; this task
added no era probe, only documentation.

### Derived `.initialize(` caller set

```
$ grep -rn '\.initialize(' --include='*.rs' src/ crates/ cargo-pmcp/
src/composition/mcp_client.rs:177        ← the BLOCKER (composition is in full-v2)
src/client/mod.rs:536,977,1032,2215,2283,2532,2592,2829,2912,2993,3060,3133,3218,3263,3328,3377   (19 rustdoc examples)
src/types/capabilities.rs:444,477,782    (3 rustdoc examples)
src/client/mod.rs:6140…7537              (21 sites, all inside `#[cfg(test)]`)
crates/mcp-tester/src/tester.rs:1127,1149,4056
crates/pmcp-agent/src/invoker/factory.rs:294                (try_v1 path)
crates/pmcp-agent/{tests,examples}/…:139,149,266,137,118,232
crates/pmcp-team-servers/src/{compose/wiring,team/member,team/server,fs/server,mem/server,approval/server,conformance/runner}.rs
crates/pmcp-team-servers/tests/{dev_binary_smoke,conformance}.rs
cargo-pmcp/{tests/agent_dev,tests/team_dev,src/loadtest/vu,src/loadtest/client,src/commands/loadtest/init}.rs
```

All of the `crates/` and `cargo-pmcp/` consumers build `pmcp` with `full` (or default), so
`v1-compat` is on for them regardless. Both named in-repo consumers were verified green:
`cargo build -p pmcp-agent --all-features` exits 0 and
`cargo test -p mcp-tester --test report_compat` reports 7 passed.

---

## 6. The gated DELETE teardown, in full

```rust
    /// Terminate the HTTP session this transport established, if any.
    ///
    /// The `v1-compat` half: when a session id is stored, DELETE the endpoint
    /// and clear it.
    #[cfg(feature = "v1-compat")]
    async fn terminate_session(&self) -> Result<()> {
        if self.session_id().is_none() {
            return Ok(());
        }
        let url = self.config.read().url.clone();
        let request = self
            .build_request_with_middleware(Method::DELETE, url.as_str(), vec![])
            .await?;

        // Send DELETE request (ignore 405 as per spec)
        let response = self.client.request(request).await;
        if let Ok(resp) = response {
            if !resp.status().is_success() && resp.status() != StatusCode::METHOD_NOT_ALLOWED {
                // Log error but don't fail close operation
                tracing::warn!("Failed to terminate session: {}", resp.status());
            }
        }

        // Clear session ID
        self.config.write().session_id = None;
        Ok(())
    }

    /// The null twin: a `full-v2` build never established a session, so there
    /// is nothing to terminate.
    ///
    /// This is the load-bearing half of T-117-55. The severed build has NO
    /// DELETE construction site at all — not a runtime `if` that is always
    /// false: nothing here names [`Method::DELETE`], builds a request, or
    /// touches `self.client`. A teardown for a session that never existed is
    /// not something this build can emit.
    #[cfg(not(feature = "v1-compat"))]
    #[allow(clippy::unused_self, clippy::unused_async)]
    async fn terminate_session(&self) -> Result<()> {
        Ok(())
    }
```

`Transport::close` is now three lines plus a comment; the whole DELETE construction is inside the
gate. The `// Send DELETE request (ignore 405 as per spec)` comment is preserved verbatim on the
v1 side, as required.

---

## 7. The severed-build runtime proof

```
$ cargo test --test v2_client_carries_no_session_on_severed_build --no-default-features --features full-v2
    Finished `test` profile [unoptimized + debuginfo] target(s)
     Running tests/v2_client_carries_no_session_on_severed_build.rs
running 2 tests
test the_severed_build_predicate_selected_this_file ... ok
test the_severed_client_stores_no_session_and_sends_no_delete ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Test count: 2. NON-ZERO.** That was not free — see Deviation 3.

The test spawns a self-contained stub server on an ephemeral port that records every request's
method and headers and answers every POST with a JSON-RPC result AND a planted
`Mcp-Session-Id: planted-session-id-that-must-not-be-echoed`. It then asserts, in order:

1. the exchange **SUCCEEDS** (a dead client cannot masquerade as a severed one);
2. no request the SERVER saw carries `mcp-session-id` — asserted server-side because a
   client-side assertion cannot prove absence, and on this build the accessor that would report a
   stored id does not exist at all;
3. `close()` produced **zero DELETE** requests, by per-method count over the server's record.

Every await is bounded by `STEP_TIMEOUT` (10 s) with a message saying a hung server must FAIL
rather than hang; the server task is aborted in `Drop`. `tests/common/v2.rs` was NOT used and NOT
edited (`git diff --stat tests/common/v2.rs` is empty) — a self-contained stub gives per-method
counting that the shared harness does not expose.

---

## 8. Negative controls — all three EXECUTED, RECORDED, REVERTED

### Control 1 — ungate `StreamableHttpTransportConfig::session_id`

```
$ cargo test --test v1_severability_tripwire the_client_transport_carries_no_ungated_session_state
running 1 test
test the_client_transport_carries_no_ungated_session_state ... FAILED

---- the_client_transport_carries_no_ungated_session_state stdout ----
ungated `session_id` at src/shared/streamable_http.rs:169 — pub session_id: Option<String>,

thread '…' panicked at tests/v1_severability_tripwire.rs:1111:5:
FAILURE MODE: src/shared/streamable_http.rs names v1 session/resumability surface OUTSIDE any
`#[cfg(feature = "v1-compat")]` region — 1 occurrence(s), first at line 169 (`session_id`):
pub session_id: Option<String>,
…
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out
```

Names the exact line, as required. Reverted.

### Control 2 — ungate `LAST_EVENT_ID` in `http_constants.rs`

```
$ cargo test --test v1_severability_tripwire the_last_event_id_const_and_its_reader_are_co_gated
---- the_last_event_id_const_and_its_reader_are_co_gated stdout ----
thread '…' panicked at tests/v1_severability_tripwire.rs:1187:5:
FAILURE MODE: `LAST_EVENT_ID` in src/shared/http_constants.rs is NOT governed by
`#[cfg(feature = "v1-compat")]`.
CONSEQUENCE: the SSE replay cursor's header name survives into a build whose whole claim is that
it never writes an attacker-influenced cursor onto the wire (T-117-53).
WHAT TO DO: gate the const AND its reader in src/shared/streamable_http.rs in ONE edit — gating
either alone does not compile.

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out
```

Reverted.

### Control 3 — make the severed client send a DELETE on close

`terminate_session`'s `full-v2` twin was given a real DELETE construction:

```
$ cargo test --test v2_client_carries_no_session_on_severed_build --no-default-features --features full-v2
running 2 tests
test the_severed_build_predicate_selected_this_file ... ok
test the_severed_client_stores_no_session_and_sends_no_delete ... FAILED

---- the_severed_client_stores_no_session_and_sends_no_delete stdout ----
thread '…' panicked at tests/v2_client_carries_no_session_on_severed_build.rs:330:5:
FAILURE MODE: closing the severed transport sent 1 DELETE request(s).
CONSEQUENCE: the `full-v2` client is emitting a teardown for a session that never existed
(T-117-55). The severed build is supposed to have no DELETE construction site at all, not a
runtime branch that happens to be false.
WHAT TO DO: keep the whole DELETE construction inside
`StreamableHttpTransport::terminate_session`'s `v1-compat` half.
Observed methods: ["POST", "POST", "DELETE"]

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

The failure carries the **server-observed method count**, exactly as the acceptance criterion
requires. Reverted; `git diff --stat src/shared/` empty afterwards.

---

## Deviations from Plan

### 1. [Rule 3 — blocking] `src/composition/mcp_client.rs` added to the file scope

- **Found during:** Task 1, from derived grep 3.
- **Issue:** `src/composition/mcp_client.rs:158` constructs `StreamableHttpTransportConfig` with a
  struct literal naming `session_id` and `on_resumption_token`. `composition` is in the `full-v2`
  feature list, so this is IN-LIB code that the "the severance build is lib-only, so downstream
  sites are unaffected" argument does not cover. Gating the fields without touching it is a
  hard compile break.
- **Fix:** rewrote the construction to go through `StreamableHttpTransportConfigBuilder`, which
  names neither gated field and compiles identically on both feature sets. This is strictly better
  than adding two field-level `#[cfg]`s to a call site.
- **Scope change:** `src/composition/mcp_client.rs` is NOT in the plan's `files_modified`. Recorded
  here rather than made silently.
- **Commit:** `dd62342f`

### 2. [Deviation from plan instruction] Assumption A4 measured FALSE — `MCP_SESSION_ID` left ungated

Not a defect: the plan explicitly branches on the measurement and this is the branch it predicted
("A4 is expected to measure FALSE"). Recorded because the plan requires the verdict to be stated.
`src/server/streamable_http_server.rs` was NOT edited and no scope extension was needed. See §2.

### 3. [Rule 3 — blocking] `crates/pmcp-code-mode/Cargo.toml` added to the file scope

- **Found during:** Task 3, running the plan's own required verification command.
- **Issue:** `cargo test --test v2_client_carries_no_session_on_severed_build --no-default-features
  --features full-v2` reported **`0 tests`, exit 0** — the whole file was compiled out because
  `v1-compat` was ON. `cargo tree -p pmcp --no-default-features --features full-v2 -e features -i pmcp`
  showed why:

  ```
  ├── pmcp feature "default"
  │   └── pmcp-code-mode v0.5.3 (crates/pmcp-code-mode)
  │       └── pmcp-code-mode feature "default"
  │           [dev-dependencies]
  │           └── pmcp v2.18.0 (*)
  ```

  `pmcp-code-mode` is a DEV-DEPENDENCY of `pmcp` (for the s41 example) and took `pmcp`'s default
  features, so cargo unified `default = ["logging", "v1-compat"]` back on for every `cargo test`
  of `pmcp`. `cargo build -p pmcp` never sees dev-deps, which is why the build proof was green
  while the test proof was silently vacuous. This is the "`--all-features` masks feature-flag
  gaps" trap in a new guise, and it would have made 117-13's `tests/v2_verbs_405_on_severed_build.rs`
  equally vacuous.
- **Fix:** `pmcp = { version = ">=2.2.0", path = "../../", default-features = false, features = ["logging"] }`,
  with a 10-line comment explaining why it must not be "simplified" back. `v1-compat` is a marker
  feature `pmcp-code-mode` uses no item from; `cargo build -p pmcp-code-mode` exits 0 and
  `make quality-gate` exits 0.
- **Scope change:** `crates/pmcp-code-mode/Cargo.toml` is NOT in the plan's `files_modified`.
- **Commit:** `40eb06b5`

### 4. [Rule 1 — bug, found by the new test] The gate scanner's first run found three FALSE POSITIVES in itself

- **Found during:** Task 3, first execution.
- **Issue:** the scanner reported `apply_resumption_header`'s parameters and body as ungated. They
  are not — the function carries `#[cfg(feature = "v1-compat")]`. The scanner consumed the pending
  gate on the signature's FIRST line, but `rustfmt` had wrapped the signature so the `{` that opens
  the body was three lines later.
- **Fix:** the scanner now carries a gate across lines while parenthesis depth is open, and
  `the_gate_region_scanner_distinguishes_gated_from_ungated` gained a wrapped-signature fixture
  asserting both the parameter line and the body line come back gated. Without this the honest
  remedy would have been to un-wrap real source code to appease a test — the exact pressure a bad
  tripwire creates.
- **Commit:** `40eb06b5`

### 5. [Rule 3] `strip_comments` taught to handle raw strings

The `#[cfg_attr(feature = "v1-compat", doc = r#"…"#)]` doc payloads contain
`http://localhost:8080`. The pre-existing stripper had no raw-string case, so it would fall out of
string mode at the first inner `"` and then treat `//` as a line comment — eating source and, worse,
desynchronising badly enough to swallow the attribute's closing `)]` and leave every subsequent
line marked as gated. Fixed with `raw_string_hashes` / `copy_raw_string`, unit-tested by
`the_stripper_handles_raw_strings`. Commit `40eb06b5`.

### Out of scope, logged not fixed

`DEFERRED-117-14-A` in `deferred-items.md`: two PRE-EXISTING `unused_imports` warnings in
`src/server/auth/jwt.rs` and `jwt_validator.rs` under `mcp-tester`'s feature combination. Neither
file was touched by this plan and neither import relates to the client cut.

---

## HANDOFF to plan 117-13 Task 3 (`docs/v1-sunset-policy.md`)

### Client-side items GATED behind `v1-compat` by this plan

1. `SendOptions::resumption_token` (public field)
2. `StreamableHttpTransportConfig::session_id` (public field)
3. `StreamableHttpTransportConfig::on_resumption_token` (public field)
4. `StreamableHttpTransportConfigBuilder::with_session_id` (public method)
5. `StreamableHttpTransportConfigBuilder::on_resumption_token` (public method)
6. `StreamableHttpTransport::session_id()` (public method)
7. `StreamableHttpTransport::set_session_id()` (public method)
8. `StreamableHttpTransport::start_sse`'s cursor PARAMETER (named `_ignored_cursor` on `full-v2`;
   arity and type unchanged, so no caller breaks)
9. `crate::shared::http_constants::LAST_EVENT_ID` (public const)
10. The private surface: `apply_resumption_header` (no twin — absent on `full-v2`), and the
    `v1-compat` halves of `resumption_cursor`, `resumption_callback`, `outbound_session`,
    `capture_session_header`, `terminate_session`, `debug_v1_fields` (×2)
11. The behaviour: on `full-v2` the client stores no `Mcp-Session-Id`, sends no `Mcp-Session-Id`,
    writes no `Last-Event-ID`, and has NO DELETE construction site

**Items 1–9 are a PUBLIC API change under `full-v2` only.** Safe today solely because `full-v2` is a
brand-new feature no published consumer builds with (plan 117-13 Task 3's assumption A7). If
`full-v2` ever enters a published crate's default set, this becomes a semver break.

### Client-side items DELIBERATELY LEFT UNGATED — the policy MUST name these

| Item | Reason |
|---|---|
| **`Client::initialize`** (`src/client/mod.rs:551`) | Fallback taken; trigger in §5. SMPL-01's "initialize" clause is met on the **SERVER side only**. The policy MUST say so explicitly rather than claiming full severance. |
| **`crate::shared::http_constants::MCP_SESSION_ID`** | A4 measured FALSE (§2). Its readers are on the shared v2 POST path, and the v2 test surface needs the name to assert ABSENCE. |
| **`StreamableHttpTransport::last_event_id` field + `last_event_id()` accessor** | A client-LOCAL SSE cursor written inside the two shared SSE-parse closures. On `full-v2` nothing reads it and it never reaches the wire (the writer is gone), but gating it would require threading `#[cfg]` into two shared closures. A residual, non-wire-visible piece of resumability state. |
| **`start_sse`'s cursor parameter on `full-v2`** | Present but INERT (`_ignored_cursor`, never read). Kept at the same arity so no caller needs a `#[cfg]` — the client analogue of 117-12's ungated `session_id: Option<String>` threading the server POST pipeline. |
| **`src/composition/mcp_client.rs`'s handshake** | `composition` is in `full-v2` yet performs a `Client::initialize` handshake unconditionally. Whether `composition` belongs in `full-v2` at all is a FEATURE-LIST question the policy should raise. |
| **`crate::shared::event_store`'s `create/validate_resumption_token`** | SERVER-side; owned by 117-13 together with `StreamableHttpServerConfig::event_store` and `InMemoryEventStore`. |

---

## Threat Flags

None. This plan added zero network endpoints, zero auth paths, zero file access and zero schema
changes; it is conditional compilation plus two test files using existing dependencies (T-117-SC:
zero external packages added).

## Self-Check: PASSED

Files claimed created/modified, verified on disk:

- `FOUND: tests/v2_client_carries_no_session_on_severed_build.rs`
- `FOUND: tests/v1_severability_tripwire.rs`
- `FOUND: src/shared/streamable_http.rs`
- `FOUND: src/shared/http_constants.rs`
- `FOUND: src/client/mod.rs`
- `FOUND: src/composition/mcp_client.rs`
- `FOUND: crates/pmcp-code-mode/Cargo.toml`

Commits claimed, verified in `git log --oneline --all`:

- `FOUND: dd62342f` — refactor(117-14): sever the client SSE-resumability surface behind v1-compat
- `FOUND: 70d92d16` — refactor(117-14): sever the client session lifecycle behind v1-compat
- `FOUND: 40eb06b5` — test(117-14): derived client-inventory tripwire plus a severed-build runtime proof
