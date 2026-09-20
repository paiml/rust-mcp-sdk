# Phase 109: Team Reference Servers - Pattern Map

**Mapped:** 2026-07-18
**Files analyzed:** 21 new/modified (1 new crate `crates/pmcp-team-servers/` + contract/workspace edits)
**Analogs found:** 20 / 21 (only the hand-rolled BM25 scorer has no in-repo analog)

This is a ~90% *composition* phase: nearly every new file copies a pattern from
shipped Phase 104–108 code (`pmcp-agent`, `pmcp-tasks`, `pmcp-sql-server`, the
`DuplexTransport` test harness, and the `ToolOutput`/`Client` core surface). The
analogs below are concrete — file path + line numbers — so the planner can point
each plan's action section at exact code to copy.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/pmcp-team-servers/Cargo.toml` | config | — | `crates/pmcp-agent/Cargo.toml` + `crates/pmcp-sql-server/Cargo.toml` | exact (feature-gated crate + `[[bin]]`) |
| `crates/pmcp-team-servers/src/lib.rs` | config/module-root | — | `crates/pmcp-sql-server/src/lib.rs` | exact (lib re-export root + module docs) |
| `src/compose/derive.rs` | service (pure fn) | transform | `crates/pmcp-agent/src/adapter/server.rs::derive_tool_description` | role-match (pure `#[must_use]` fn + proptest) |
| `src/compose/wiring.rs` | service | event-driven | `tests/common/duplex.rs::call_via_server` + `AgentServer::run` | role-match (in-memory wiring over DuplexTransport) |
| `src/fs/backend.rs` | model (trait) | file-I/O | `crates/pmcp-tasks/src/store/backend.rs::StorageBackend` | exact (object-safe async backend trait + error enum) |
| `src/fs/local.rs` | service | file-I/O | `crates/pmcp-tasks/src/store/memory.rs` (dev backend impl) | role-match (dev impl of a backend trait) |
| `src/fs/server.rs` | controller | request-response | `AgentServer::build` (`Server::builder().tool()`) | role-match (tool→backend dispatch) |
| `src/mem/backend.rs` | model (trait) | CRUD | `crates/pmcp-tasks/src/store/backend.rs::StorageBackend` | exact (same trait-seam model) |
| `src/mem/bm25.rs` | utility | transform | — | **NO ANALOG** (hand-rolled scorer; proptest only) |
| `src/mem/server.rs` | controller | request-response | `AgentServer::build` | role-match |
| `src/approval/channels.rs` | service | event-driven / request-response | `crates/pmcp-agent/src/adapter/factory.rs` (trait seam) | partial (notify-transport seam) |
| `src/approval/server.rs` | controller | request-response | `AgentServer::build` (`InMemoryTaskStore` + `.task_store()`) | exact (task-store-backed server) |
| `src/team/member.rs` | service | request-response (MCP hop) | `tests/common/duplex.rs::call_via_server` + `Client::call_tool_with_task_and_meta` (109-00) | exact (pmcp::Client per member) |
| `src/team/guards.rs` | middleware | request-response | `run_agent_tool` arg/`extra` parsing (`adapter/server.rs:272`) | partial (validation from args/`_meta`) |
| `src/team/server.rs` | controller | request-response | `ToolHandler::handle_output` → `ToolOutput::Result` (`src/server/mod.rs:304`) | exact (verbatim-envelope re-emit) |
| `src/conformance/runner.rs` | test-harness | request-response | `tests/team_contracts_conformance.rs` + `tests/common/duplex.rs` | exact (wire-level fixture driver) |
| `src/bin/{team_fs,mem_mcp,approval_mcp,team_mcp}.rs` | route (binary) | request-response | `crates/pmcp-sql-server/src/main.rs` + `src/cli.rs` | exact (thin `#[tokio::main]` + clap `Args`) |
| `examples/doc_review_team.rs` | test/example | event-driven | existing `examples/` numbered convention + `call_via_server` | role-match |
| `tests/{conformance,derive_props,small_team}.rs` | test | request-response | `tests/team_contracts_conformance.rs` | exact |
| `contracts/team-servers/binding.yaml` | config | — | `contracts/binding.yaml` | exact (binding schema) |
| `contracts/team-servers-v1.yaml` (rev) | config | — | itself (additive minor bump) | exact |

## Pattern Assignments

### `crates/pmcp-team-servers/Cargo.toml` (config)

**Primary analog:** `crates/pmcp-agent/Cargo.toml` (deps + wasm-clean `default-features = false` discipline)
**Binary/feature analog:** `crates/pmcp-sql-server/Cargo.toml` (`[[bin]]` + feature-gated backends)

**Core-dep pattern** (`crates/pmcp-agent/Cargo.toml` lines 15-33) — path dep on `pmcp`
with `default-features = false` to stay wasm/reqwest-clean, `reqwest` as an OPTIONAL
feature-gated dep:
```toml
pmcp = { version = "2.17.0", path = "../..", default-features = false }
pmcp-package = { version = "0.1", path = "../pmcp-package" }
serde = { version = "1.0", features = ["derive"] }
serde_json = { version = "1.0", features = ["preserve_order"] }
async-trait = "0.1"
thiserror = "2.0"
uuid = { version = "1.17", features = ["v4"] }
reqwest = { version = "0.13", default-features = false, features = ["json", "rustls"], optional = true }
```
Add `pmcp-agent = { version = "0.1", path = "../pmcp-agent" }` (team-mcp composes it).

**Feature + `[[bin]]` pattern** (`crates/pmcp-sql-server/Cargo.toml` lines 30-32, 51-58,
and RESEARCH.md Pattern 1): per-server features + `webhook = ["dep:reqwest"]` +
`http = ["pmcp/streamable-http"]`, one `required-features`-gated `[[bin]]` per server.
Note `pmcp-agent`'s HTTP feature precedent (lines 44-47): keep `streamable-http` behind
a non-default `http` feature so the default/wasm build never pulls native-only transport.

**Dev-deps pattern** (`crates/pmcp-agent/Cargo.toml` lines 44-53): `pmcp` with
`features = ["full"]` as the client-side test harness, `proptest = "1.7"`,
`pretty_assertions`, `semver` (constructing `TeamPackage`/`AgentPackage` in tests),
plus `tempfile = "3"` (team-fs dir tests, per `pmcp-sql-server` dev-deps).

**Exclude pattern** (`crates/pmcp-sql-server/Cargo.toml` lines 11-18): exclude
`.planning/`, `.pmat/`, `fuzz/`, `tests/` from the published tarball.

---

### `src/fs/backend.rs` + `src/mem/backend.rs` (model — trait, file-I/O / CRUD)

**Analog:** `crates/pmcp-tasks/src/store/backend.rs` (`StorageBackend` trait, lines 179-262)

**Object-safe async trait pattern** (lines 179-201): `Send + Sync` supertrait,
`#[async_trait]`, per-method `# Errors` rustdoc, returns `Result<_, BackendError>`:
```rust
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn get(&self, key: &str) -> Result<VersionedRecord, StorageError>;
    async fn put(&self, key: &str, data: &[u8]) -> Result<u64, StorageError>;
    // ...
}
```
Copy this shape for `TeamFsBackend` (RESEARCH Pattern 2 lists the 11 `fs__*` methods)
and `TeamMemoryBackend` (6 `mem__*` methods).

**Error-enum pattern** (lines 81-147): a `#[derive(Debug)] pub enum FsError`/`MemError`
with `impl fmt::Display` + `impl std::error::Error` (with `source()`), NOT `thiserror`
here — but the connector crates use `thiserror` (`pmcp-sql-server` `DispatchError`); the
planner may pick either. `thiserror` is already a dep (RESEARCH Standard Stack).

**Testing pattern** (lines 319-521): exhaustive unit tests for every Display arm,
`source()` return, and helper round-trips live in a `#[cfg(test)] mod tests` in the
same file. Mirror this density (ALWAYS unit requirement).

**Security note (V5, RESEARCH Security Domain):** `LocalDirBackend` must canonicalize +
assert containment within the workspace/review roots — reject `..`/absolute-path escape.
No analog in `pmcp-tasks` (its keys are opaque); this is new guard code.

---

### `src/fs/server.rs`, `src/mem/server.rs`, `src/approval/server.rs` (controller, request-response)

**Analog:** `AgentServer::build` in `crates/pmcp-agent/src/adapter/server.rs` (lines 211-258)

**Server-builder + tool-registration pattern** (lines 240-250) — build `TypedTool`s with
an explicit input schema and a closure handler, register on `Server::builder()`, and for
task-bearing servers attach an `InMemoryTaskStore`:
```rust
let tool = TypedTool::new_with_schema(tool_name.clone(), input_schema.clone(), handler)
    .with_description(description.clone())
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

let task_store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
let server = Server::builder()
    .name(server_name).version(server_version)
    .tool(tool_name.clone(), tool)
    .task_store(task_store)
    .build()?;
```
- **approval-mcp** (RESEARCH Pattern 3): reuse this `InMemoryTaskStore` + `.task_store()`
  verbatim; register `resolve_approval` + `get_approval` as **UNNAMESPACED** tools plus
  one `team_approval__ask_<member>` per `human_role`, computed ONCE at build (D-07).
- **team-fs / mem-mcp**: no task store; each `fs__*`/`mem__*` tool name maps to one
  backend call; unknown names must ERROR (never panic — contract invariant).

**Handler-closure pattern** (lines 229-238): the `move |args, extra| Box::pin(async move {…})`
closure that clones shared `Arc` collaborators per call — copy this exact shape for
handlers that need the backend `Arc`.

---

### `src/approval/channels.rs` (service, notify-transport seam)

**Analog:** `crates/pmcp-agent/src/adapter/factory.rs` (`CompletionSourceFactory` trait, lines 36-70)

**Notify-transport trait seam** — model `ApprovalChannel` as a small trait with a
console impl (`tracing`/`println`, no stdin — D-10) and a `webhook`-gated `reqwest` POST
impl (D-11). The factory pattern (one trait, two impls, one selected at wiring) mirrors
`SamplingSourceFactory` vs `FixedSourceFactory` (lines 52-88). Resolution is ALWAYS via
the `resolve_approval` tool — channels are pure outgoing notification, never a resolution
path (anti-pattern: stdin prompting).

---

### `src/team/member.rs` + `src/team/server.rs` (team-mcp — the TEAM-05 core)

**Member-hop analog:** `tests/common/duplex.rs::call_via_server` (lines 86-100) +
`Client::call_tool_with_task_and_meta` (the NEW 109-00 forwarding API — NOT the plain
`call_tool_with_task` at `src/client/mod.rs:624`)
**Re-emit analog:** `ToolHandler::handle_output` / `ToolOutput::Result` (`src/server/mod.rs:246-311`)

**In-memory member wiring** (`duplex.rs` lines 86-100): spawn the member `AgentServer` on
one half of a `DuplexTransport::pair()`, hold a `pmcp::Client` on the other, `initialize`
once:
```rust
let (client_t, server_t) = DuplexTransport::pair();
tokio::spawn(async move { let _ = agent_server.run(server_t).await; });
let mut client = Client::new(client_t);
client.initialize(ClientCapabilities::default()).await?;
```

**CRITICAL — task+`_meta`-forwarding dispatch (Pitfall 1 / A2 resolved via prerequisite plan
109-00):** the member `AgentServer` returns the BARE answer for non-task calls
(`adapter/server.rs:323` — `if !extra.is_task_request()`). The plain
`Client::call_tool_with_task` (`src/client/mod.rs:624`) is INSUFFICIENT: it hardcodes
`_meta: None` (so it cannot carry the D-14 guard `_meta`) and may return EITHER
`ToolCallResponse::Result` OR `ToolCallResponse::Task`. team-mcp MUST instead use the NEW
109-00 API `Client::call_tool_with_task_and_meta(tool_name, args, forward_meta)`, which
forwards task augmentation AND the namespaced guard `_meta` (depth+1, caller id, ancestor
chain, built via `RequestMeta::with_meta`) in one call. Guards read the INBOUND guard state
from `RequestHandlerExtra.request_meta` (propagated by 109-00), not from a bespoke channel.

**Explicit forwarding contract (`MemberTaskForwarding`, 109-01 `identity.rs`):** match the
returned `ToolCallResponse`:
- `Result(r)` → re-emit `r` via `ToolOutput::Result`, stripping member `_meta` to
  related-task only (Pitfall 5).
- `Task(t)` → `Client::wait_for_task(t.id, ..)` to terminal, then SYNTHESIZE a
  `CallToolResult` carrying the member's content plus related-task `_meta`.
Either way the surfaced key is the SDK constant `RELATED_TASK_META_KEY` =
`io.modelcontextprotocol/related-task` (`src/types/tasks.rs:9`) — NEVER a bare `related_task`.

**Verbatim/synthesized re-emit** (`src/server/mod.rs:304-311`): override `handle_output` to
return `ToolOutput::Result(call_tool_result)` so the member's related-task `_meta` reaches
the wire. **Read the bypass warning at `src/server/mod.rs:255-278` FIRST** — this variant
skips response middleware; the handler owns its own redaction (keep the re-emit tight —
Pitfall 5).

**Guards** (`src/team/guards.rs`): parse depth/caller-id/ancestor-chain from the namespaced
`_meta` propagated on `RequestHandlerExtra.request_meta` (109-00). Strict integer parse of
`x-pmcp-team-depth` (`str::parse::<i64>()`, error on garbage; ABSENT = root/depth 0);
compare stable `MemberId` identities, not names (Pitfall 4). Property-test all four guard paths.

---

### `src/conformance/runner.rs` + `tests/conformance.rs` (test-harness, wire-level)

**Structural analog:** `tests/team_contracts_conformance.rs` (fixture loader lines 79-108,
schema assertions lines 152-219)
**Wire-drive analog:** `tests/common/duplex.rs::call_via_server` (lines 86-100)

**Fixture schema (already frozen)** — `{ schema_version:"1", case_id, server,
request{name,arguments,_meta}, expect{outcome,match,response} }`, with the
outcome↔response-shape rule (`team_contracts_conformance.rs` lines 208-218): `error` ⇒
`response.error.code` is a number; `success` ⇒ `response.content` is an array. The
`_meta[related_task]` presence check (lines 258-269) is the TEAM-05 fixture assertion.

**Wire-level upgrade (D-19):** where the Phase-107 test only string-matches the contract,
the new runner drives a real `pmcp::Client` (`call_via_server`) → `initialize` →
`tools/list` (assert EXACT advertised set — Pitfall 3: `resolve_approval`/`get_approval`
unnamespaced, initiator never counted) → `tools/call` per fixture → subset-match against
`expect.response`. Export behind the `conformance` feature so the platform imports it as a
dev-dependency (D-17). Fixtures resolve via `CARGO_MANIFEST_DIR` (lines 59-65) so they stay
canonical in `contracts/team-servers/fixtures/`.

---

### `src/bin/*.rs` + CLI `Args` (route — binaries)

**Analog:** `crates/pmcp-sql-server/src/main.rs` (lines 1-14) + `src/cli.rs` (lines 32-52) +
`src/lib.rs::serve` (lines 156-164)

**Thin `#[tokio::main]` shim** (`main.rs` lines 9-14) — no business logic; parse clap `Args`,
delegate to a library `run`:
```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    run(args).await?;
    Ok(())
}
```

**clap `Args` pattern** (`cli.rs` lines 32-52): `#[derive(clap::Parser)]` with `#[arg(long)]`
fields. Per D-03, `--package <PathBuf>` is the primary config; `--port`/`--data-dir` are
override-only; `--stdio` is the fallback-transport flag (RESEARCH Code Examples "binary skeleton").

**HTTP serve pattern** (`lib.rs::serve` lines 156-164): wrap the built `Server` in
`Arc<Mutex<_>>` and hand to `StreamableHttpServer::with_config(addr, shared,
StreamableHttpServerConfig::default())` — the SDK owns DNS-rebinding/CORS/security-headers
(never hand-rolled). The `x-pmcp-team-depth` header → `_meta` edge map (D-14) is new
binary-layer code with no existing helper (RESEARCH Assumption A4).

---

### `contracts/team-servers/binding.yaml` (config — D-18)

**Analog:** `contracts/binding.yaml` (lines 1-18)

**Binding schema:** top-level `version`, `target_crate: pmcp-team-servers`, `bindings[]`
each `{contract, equation, function, module_path, signature, status, notes}`:
```yaml
version: 1.0.0
target_crate: pmcp-team-servers
bindings:
- contract: team-servers-v1.yaml
  equation: fs_tool_surface
  function: <fn>
  module_path: pmcp_team_servers::fs::...
  signature: '...'
  status: implemented
  notes: ...
```
**Pitfall 2 (verified):** no build target runs `pmat comply` today — even the existing
`binding.yaml` is validated by nothing in `make quality-gate`/CI. Treat "author binding.yaml"
and "wire `pmat comply`" as TWO tasks; run `pmat comply` against the EXISTING binding.yaml
first (Wave 0) to learn the schema before authoring, then add a `make comply` target.

## Shared Patterns

### wasm-clean / reqwest-clean dependency discipline
**Source:** `crates/pmcp-agent/Cargo.toml` lines 9-14, 34-47
**Apply to:** the crate `Cargo.toml` and every module
`pmcp` is pinned `default-features = false`; `reqwest` and `pmcp/streamable-http` are ONLY
reachable through non-default features (`webhook`, `http`). The default + wasm32 build must
never pull reqwest or native-only transport. This is a hard workspace rule.

### Object-safe async backend trait (the TaskStore model)
**Source:** `crates/pmcp-tasks/src/store/backend.rs` lines 179-262
**Apply to:** `src/fs/backend.rs`, `src/mem/backend.rs`
`#[async_trait] pub trait X: Send + Sync` with `Result<_, XError>` methods, a dev impl in
the SDK, the operated impl left platform-side. Contract in SDK, backend in SDK.

### `InMemoryTaskStore` + `.task_store()` reuse
**Source:** `crates/pmcp-agent/src/adapter/server.rs` lines 244-250
**Apply to:** approval-mcp (D-03..D-12) — do NOT build a bespoke approval store; the
create→working→completed lifecycle is already observable via `tasks/get`/`tasks/result`.

### Task+`_meta`-forwarding client call for related-task
**Source:** `Client::call_tool_with_task_and_meta` (the NEW 109-00 API), contrast the plain
`call_tool_with_task` (`src/client/mod.rs:624`, hardcodes `_meta: None`, may return `Task`) and
`call_tool` (`src/client/mod.rs:577`, drops related-task entirely)
**Apply to:** `src/team/member.rs` — forwards task augmentation AND the guard `_meta` in one
call, then applies the `MemberTaskForwarding` contract (re-emit a `Result` / `wait_for_task`+
synthesize a `Task`) to surface related-task under `RELATED_TASK_META_KEY`. Plain
`call_tool` drops it (Pitfall 1); plain `call_tool_with_task` cannot carry the D-14 guard `_meta`.

### `ToolOutput::Result` verbatim envelope (owns its redaction)
**Source:** `src/server/mod.rs` lines 246-311 (bypass warning lines 255-278)
**Apply to:** `src/team/server.rs` (and `fs__complete_task` if it carries `related_task`).
Overriding `handle_output` → `ToolOutput::Result` bypasses response middleware; the handler
is responsible for its own `content`/`_meta` sanitization.

### DuplexTransport in-memory wiring
**Source:** `tests/common/duplex.rs` lines 37-52 (`pair()`), 86-100 (`call_via_server`)
**Apply to:** `src/compose/wiring.rs`, `src/team/member.rs`, `src/conformance/runner.rs`,
`tests/small_team.rs` — deterministic, socket-free, CI-sandbox-safe (D-04). NOTE: this lives
in `tests/common/` today; the phase promotes the convention into crate `src/` for the
exportable wiring API + runner.

### Thin binary → testable library split
**Source:** `crates/pmcp-sql-server/src/main.rs` + `src/lib.rs`
**Apply to:** all four `src/bin/*.rs` — keep binaries to clap-parse + delegate so the
assembly/serve logic stays unit-testable and Phase 110's `cargo pmcp team dev` CLI stays thin.

### `CARGO_MANIFEST_DIR`-anchored fixtures + `#[cfg(test)] mod tests`
**Source:** `tests/team_contracts_conformance.rs` lines 59-65; `pmcp-tasks/.../backend.rs` lines 319-521
**Apply to:** every test/conformance file — location-independent fixture resolution and
in-file exhaustive unit tests (ALWAYS unit + property requirements).

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `src/mem/bm25.rs` | utility | transform | No keyword/BM25/TF-IDF scorer exists anywhere in the workspace. RESEARCH recommends a ~80-line hand-rolled, zero-dep, fully proptest-able scorer (the `bm25` crate is explicitly NOT recommended — it bundles an embedder the milestone forbids). Planner should use RESEARCH "Don't Hand-Roll" note + proptest invariants, not a codebase analog. |

Partial-only analogs (planner should lean on RESEARCH patterns for the novel parts):
- `src/compose/derive.rs` — the pure `derive_attachment(&TeamPackage) -> AttachmentSet` fn
  has the *shape* of `derive_tool_description` (pure `#[must_use]` + in-file tests) but its
  rule (RESEARCH Code Examples lines 410-423) is novel; property-test the N/M matrix.
- `src/team/guards.rs` — path traversal + strict-depth + ancestor-cycle guards are new
  security code (RESEARCH Security Domain); only the arg/`_meta`-parse mechanics have an analog.
- HTTP `x-pmcp-team-depth` → `_meta` edge map — no existing helper (Assumption A4), new
  binary-layer code.

## Metadata

**Analog search scope:** `crates/pmcp-agent/`, `crates/pmcp-tasks/`, `crates/pmcp-sql-server/`,
`crates/pmcp-package/`, `src/server/mod.rs`, `src/client/mod.rs`, `tests/common/`,
`tests/team_contracts_conformance.rs`, `contracts/`
**Files scanned:** ~14 read in full/targeted + directory listings of four crate `src/` trees
**Key verification wins:** the member hop is resolved via the NEW 109-00
`Client::call_tool_with_task_and_meta` API — the plain `call_tool_with_task` at
`src/client/mod.rs:624` is INSUFFICIENT (hardcodes `_meta: None`, may return
`ToolCallResponse::Task`); Assumption A2 is discharged via prerequisite plan 109-00 + the
109-05 `MemberTaskForwarding` contract (see RESEARCH Open Question 2); `ToolOutput`
bypass semantics confirmed at `src/server/mod.rs:246-311`; `InMemoryTaskStore` reuse confirmed
at `adapter/server.rs:244`.
**Pattern extraction date:** 2026-07-18
