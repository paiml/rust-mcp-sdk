# Phase 112: Version Plumbing Spine - Pattern Map

**Mapped:** 2026-07-22
**Files analyzed:** 14 (11 modified, 3 new)
**Analogs found:** 14 / 14

> **Nature of this phase:** This is an *additive plumbing phase against pmcp's own tree*, not a
> greenfield dependency phase. The "closest analog" for most files is the file itself (an
> established pattern already lives there) plus the **Phase-109-00 `request_meta` threading
> precedent**, which wired exactly this kind of per-request field through both dispatch sites and
> the wasm mirror. Copy that precedent verbatim for `ProtocolContext`.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/types/protocol/version.rs` (MOD) | config/constants | transform (classifier) | itself (`negotiate_protocol_version` + consts) | exact |
| `ProtocolContext` NEW (`src/server/` or `src/shared/`) | model (per-request value) | request-response | `RequestMeta` newtype-with-builder (`mod.rs:315`) | exact |
| `Era` enum NEW (in `version.rs`) | model/config | transform | `ProtocolErrorCode` C-style enum (`mod.rs:129`) | role-match |
| `src/server/builder.rs` (MOD) | builder/config | request-response | itself — `capabilities()` / `stateless_mode` field pattern | exact |
| `src/server/cancellation.rs` (MOD, native `RequestHandlerExtra`) | model (per-request carrier) | request-response | itself — `request_meta` field + `with_request_meta` (Phase-109) | exact |
| ~~`src/shared/cancellation.rs` (wasm mirror)~~ **CORRECTED — DO NOT EDIT** | — | — | — | **stale premise** |
<!-- CORRECTION (post-review, verified against live tree): src/shared/cancellation.rs is a DEAD ORPHAN — not declared in src/shared/mod.rs (no `pub mod cancellation`), never compiled on any target. It is NOT a wasm mirror. The wasm32 RequestHandlerExtra is a zero-field stub at src/server/mod.rs:162. This phase's plumbing is NATIVE-ONLY; Plan 02 edits src/server/cancellation.rs only. See 112-02/112-04 scope notes. -->

| `src/shared/http_constants.rs` (MOD) | config/constants | request-response | itself — `MCP_SESSION_ID` / `MCP_PROTOCOL_VERSION` | exact |
| `src/server/streamable_http_server.rs` (MOD) | middleware/transport ingress | request-response | existing header-parse/`stateless()` branch (same file) | role-match |
| `src/server/core.rs` (MOD, dispatch `:1118`) | controller/dispatch | request-response | itself — `handle_request_internal(auth_context)` + `with_request_meta` at `:514` | exact |
| `src/server/mod.rs` (MOD, dispatch `:1245`) | controller/dispatch | request-response | `core.rs` dispatch twin (parity) | exact |
| `src/types/protocol/mod.rs` `ClientRequest` (MOD) | model (protocol enum) | request-response | itself — `TasksGet`/`TasksList` variant additions (`:527`,`:533`) | exact |
| `src/types/capabilities.rs` `extensions` (REUSE) | model | request-response | itself — field `:109` already shipped + locking tests | exact (no change to type) |
| error-code table NEW (`src/types/protocol/error_codes.rs`) | config/constants | transform | frozen `-32002` literal sites + `ProtocolErrorCode` | role-match |
| `ResultType` enum NEW (internal) | model | transform | `Era` enum / `IconTheme` C-style enum (`mod.rs:120`) | role-match |

## Pattern Assignments

### `src/types/protocol/version.rs` — add const + `Era` + `protocol_era()` (VERS-02, D-03)

**Analog:** itself. The file already holds the constant block, the `negotiate_protocol_version`
fallback, and a `#[cfg(test)]` module with the exact tripwire tests that MUST be deliberately
updated.

**Existing constants pattern** (lines 3-20) — copy this shape for the new opt-in const:
```rust
/// Latest protocol version supported by this SDK.
pub const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";   // ← KEEP PINNED (Pitfall 1)
pub const DEFAULT_PROTOCOL_VERSION: &str = "2025-03-26";
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    LATEST_PROTOCOL_VERSION, "2025-06-18", DEFAULT_PROTOCOL_VERSION, "2024-11-05",
];
```
Add `pub const PROTOCOL_VERSION_2026_07_28: &str = "2026-07-28";` and the `Era` enum +
`protocol_era(&str) -> Era` classifier next to these (research Code Examples). **Do NOT flip
`LATEST_PROTOCOL_VERSION`** — `negotiate_protocol_version` returns `LATEST` for any unknown
version (line 33), which would silently upgrade legacy clients.

**Tripwire tests that MUST change** (lines 42-53) — these are locking tests; updating them is
the intended signal, not a regression:
```rust
#[test] fn latest_version_is_2025_11_25() { assert_eq!(LATEST_PROTOCOL_VERSION, "2025-11-25"); }
#[test] fn supports_four_versions_including_2024() { assert_eq!(SUPPORTED_PROTOCOL_VERSIONS.len(), 4); ... }
```
Keep `latest_version_is_2025_11_25` passing (proves the pin held); rename/extend
`supports_four_versions_including_2024` to five and add `protocol_era` unit tests
(`protocol_era("2026-07-28")==V2`, unknown→V1). Note `src/lib.rs:255` has a doctest
`assert_eq!(LATEST_PROTOCOL_VERSION, "2025-11-25")` — it must stay green.

---

### `ProtocolContext` (NEW value type) + `Era`/`ResultType` enums (VERS-01, VERS-07)

**Analog:** `RequestMeta` (`src/types/protocol/mod.rs:312-377`) — the newtype-with-builder shape;
`IconTheme`/`ProtocolErrorCode` C-style enums for `Era`/`ResultType`.

**Newtype + builder pattern to copy** (`RequestMeta`, lines 312-377):
```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]                    // ← REQUIRED on any new public struct (semver de-risk)
#[serde(rename_all = "camelCase")]
pub struct RequestMeta { /* typed fields + #[serde(flatten)] other */ }

impl RequestMeta {
    pub fn new() -> Self { Self::default() }
    pub fn with_progress_token(mut self, ...) -> Self { self.x = Some(...); self }
}
```
`ProtocolContext { era, negotiated_version: ProtocolVersion, client_info: Option<Implementation>,
client_capabilities: Option<ClientCapabilities> }` should be `#[non_exhaustive]` + `Debug + Clone`
with `new()`/`with_*` builders. All four field types already exist (`ProtocolVersion` `mod.rs:28`,
`Implementation` `mod.rs:157`, `ClientCapabilities` `capabilities.rs`).

**C-style enum pattern for `Era`/`ResultType`** (`IconTheme` `mod.rs:120`, `ProtocolErrorCode`
`mod.rs:128`):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolErrorCode { InvalidRequest = -32600, ... }
```
`ResultType` is **internal** (D-08) — do NOT make it a public field on any Result struct; only
`Complete` is used this phase.

---

### `src/server/cancellation.rs` (native) + `src/shared/cancellation.rs` (wasm) — RequestHandlerExtra (VERS-01/03/09)

**Analog:** itself — the `request_meta` field is the Phase-109 precedent for exactly this
additive per-request field, and both structs are already `#[non_exhaustive]` (native `:178`,
wasm `:49`), so adding a field is source-compatible.

**Additive field + builder + accessor triplet to copy:**

Native declaration (`cancellation.rs:208-214`):
```rust
/// The request's `_meta` object as raw JSON (MCP `_meta`).
pub request_meta: Option<serde_json::Value>,
```
`new()` default (`cancellation.rs:264`): `request_meta: None,`
Wasm builder (`shared/cancellation.rs:115-119`):
```rust
#[must_use]
pub fn with_request_meta(mut self, meta: Option<serde_json::Value>) -> Self {
    self.request_meta = meta; self
}
```
Accessor precedent (`shared/cancellation.rs:132-135`):
```rust
#[cfg(not(target_arch = "wasm32"))]
pub fn auth_context(&self) -> Option<&crate::server::auth::AuthContext> { self.auth_context.as_ref() }
```

Add `pub protocol_context: Option<ProtocolContext>` (default `None` in `new()`),
`with_protocol_context(...)`, and typed accessors `era()`, `protocol_version()`, `client_info()`,
`client_capabilities()`, `trace_context()`. **Mirror BOTH files** — the wasm struct gates
`auth_context`/`peer`/`result_meta` behind `#[cfg(not(target_arch = "wasm32"))]`; follow that
cfg discipline. `trace_context()` reads out of the existing `request_meta` JSON (VERS-09), it does
not need its own field. Add a parity test like `test_shared_extensions_parity`
(`shared/cancellation.rs:157`).

---

### `src/server/core.rs` (`:1118`) + `src/server/mod.rs` (`:1245`) — dispatch threading (VERS-01/04/07)

**Analog:** itself. Both dispatch sites share the identical signature
`handle_request_internal(&self, id, request, auth_context: Option<AuthContext>)` — thread
`ProtocolContext` as a NEW parameter right next to `auth_context` (the research-locked pattern).

**Signature to extend** (`core.rs:1118-1123`, twin at `mod.rs:1245-1249`):
```rust
async fn handle_request_internal(
    &self, id: RequestId, request: Request,
    auth_context: Option<AuthContext>,           // ← add protocol_context alongside
) -> JSONRPCResponse {
```

**Extra-construction threading precedent** (`core.rs:502-519`) — the exact place a new
per-request value gets folded into `RequestHandlerExtra` via the `.with_*` chain:
```rust
let mut extra = self.attach_peer(
    RequestHandlerExtra::new(request_id.clone(), token)
        .with_auth_context(auth_context)
        .with_task_request(req.task.clone())
        .with_request_meta(req._meta.as_ref().and_then(|m| serde_json::to_value(m).ok())),
);
```
Add `.with_protocol_context(protocol_context)` to this chain.

**Era-gate arm precedent** (`core.rs:1125-1148`) — the `Initialize` match arm and the
`is_initialized()` `-32002` guard show where the V1/V2 fork lives. `server/discover` is a new
match arm reached only when `era==V2`; a V1 `server/discover` falls through to `-32601`
method-not-found (D-10). `resultType` injection (v2-only, D-07/D-08) happens at the
`success_response` serialization boundary (`core.rs:1134`).

**mod.rs dispatch note** (`mod.rs:1261`): this site already calls
`negotiate_protocol_version(&init_req.protocol_version)` and builds `InitializeResult` — the
natural place to also attach v2 `serverInfo`/`resultType` on the v2 path.

**Wasm mirror parity (Pitfall 3):** whatever you thread here must also be threaded at the wasm
dispatch mirror — `request_meta` is already wired at both native sites + wasm; grep
`with_request_meta` call sites and add `with_protocol_context` beside each.

---

### `src/server/builder.rs` — opt-in accept-list (VERS-02, D-01/D-02)

**Analog:** itself — `capabilities()` setter (`:178`) and the `stateless_mode: Option<bool>`
field (`:89`) are the exact "add a field + default in `new()` + one `with_*` setter" pattern.

**Field + default + setter triplet to copy:**
- Field (`builder.rs:88-89`): `stateless_mode: Option<bool>,`
- `new()` default (`builder.rs:136`): `stateless_mode: None, // Auto-detect by default`
- Setter (`builder.rs:178-181`):
```rust
pub fn capabilities(mut self, capabilities: ServerCapabilities) -> Self {
    self.capabilities = capabilities; self
}
```
Add `supported_protocol_versions: Vec<ProtocolVersion>` (default = v1-only set, i.e. NOT
including 2026-07-28 → D-04 "non-opted-in behaves exactly as today") and
`.with_supported_protocol_versions(impl IntoIterator<Item = ProtocolVersion>)`. Thread the
resolved set into `ServerCore` so ingress can decide whether to run era-detection at all.
`extensions` population (VERS-08) rides the existing `capabilities()` field
(`ServerCapabilities.extensions` `capabilities.rs:109`) — optionally add a
`.with_extension(id, value)` convenience mirroring `RequestMeta::with_meta` (`mod.rs:373-377`).

---

### `src/shared/http_constants.rs` — v2 header constants (VERS-05)

**Analog:** itself. Copy the `MCP_SESSION_ID`/`MCP_PROTOCOL_VERSION` const style verbatim.

**Pattern** (lines 4-8):
```rust
/// MCP session ID header name
pub const MCP_SESSION_ID: &str = "mcp-session-id";
/// MCP protocol version header name
pub const MCP_PROTOCOL_VERSION: &str = "mcp-protocol-version";
```
Add `pub const MCP_METHOD: &str = "mcp-method";` and `pub const MCP_NAME: &str = "mcp-name";`
(lowercase — HTTP header names are case-insensitive and the file already uses lowercase for the
mcp-* family).

---

### `src/server/streamable_http_server.rs` — strict v2 header enforcement (VERS-05, D-05/D-06)

**Analog:** the existing header-parse + `stateless()` branch in the same file (the request path
Phase 113 later gates onto). This phase adds the validation gate only.

**Behavior to add** (no single excerpt — this is new validation logic):
- On the v2 path (request self-identifies via `MCP-Protocol-Version: 2026-07-28` / `_meta`),
  reject with 4xx + structured JSON-RPC error when `Mcp-Method`/`Mcp-Name` are absent (D-05).
- Cross-check `Mcp-Method` against the JSON-RPC body `method`; mismatch = reject, fail closed
  (D-06, anti-smuggling). Use the new `MCP_METHOD`/`MCP_NAME` constants.
- v1 requests: untouched (no era-detection when server not opted-in — D-04).
- HTTP-only: stdio has no headers (D-11); tests MUST route through the HTTP `ConformanceTarget`,
  not in-memory transport (research Wave-0 Pitfall 11).

---

### `src/types/protocol/mod.rs` `ClientRequest` — `ServerDiscover` variant (VERS-04)

**Analog:** itself — the `TasksGet`/`TasksResult`/`TasksList` variants (`:527`,`:530`,`:533`) are
the most recent additive-variant precedent on this exact enum.

**Variant pattern to copy** (`mod.rs:525-533`):
```rust
/// Get task status (MCP 2025-11-25 Tasks).
#[serde(rename = "tasks/get")]
TasksGet(crate::types::tasks::GetTaskRequest),
```
Add `#[serde(rename = "server/discover")] ServerDiscover(...)` (+ `TasksUpdate` scaffold only if
Phase 114 needs the variant early). **Pitfall 4:** `ClientRequest` is NOT `#[non_exhaustive]`
(declared `mod.rs:475-478`) — `cargo-semver-checks` flags variant addition as
`enum_variant_added` (minor, but breaks downstream exhaustive matches). Planning decision
(Open Question 1): consider adding `#[non_exhaustive]` now. Variant round-trip tests exist at
`mod.rs:640-657` — extend them.

---

### error-code table (NEW `src/types/protocol/error_codes.rs`) — VERS-06 structure-first

**Analog:** the frozen `-32002` literal sites (`core.rs:1145`, `task_dispatch.rs:576`) + the
`ProtocolErrorCode` enum (`mod.rs:128-138`). Recommendation (research Open Q2): a module of
`pub const i32`s, NOT new `ProtocolErrorCode` enum variants (keeps the C-style discriminant
surface untouched → avoids semver risk from Pitfall 4).

**Frozen constant to re-export VERBATIM** (do NOT redefine or edit its test):
```rust
// core.rs:1145 / task_dispatch.rs:576 — pmcp's task-pending code, FROZEN.
pub const V1_TASK_PENDING: i32 = -32002;
// V2 resource-not-found (SEP-2164 -32002→-32602, DIFFERENT semantic): TODO from final schema.json
// pub const V2_RESOURCE_NOT_FOUND: i32 = /* leave unfilled until 2026-07-28 schema.json */;
```
**Pitfall 6 (hard rule):** the SEP-2164 `-32002`→`-32602` rename targets *resource-not-found* —
a DIFFERENT semantic from pmcp's frozen *task-pending* `-32002`. Do NOT reconcile them. Fill v2
values ONLY from the published final schema; leave as `TODO` this phase. The locking test
`pending_tasks_result_preserves_minus_32002` (`task_dispatch_tests.rs:355`, asserts `-32002` at
`:398`) MUST NOT be edited.

## Shared Patterns

### Additive-field + typed-accessor (never mutate existing field types)
**Source:** `RequestHandlerExtra.request_meta` (`src/server/cancellation.rs:208`,
`src/shared/cancellation.rs:69`) — Phase-109 precedent.
**Apply to:** `ProtocolContext` on both `RequestHandlerExtra` structs; every new value type.
```rust
// declare additive pub field → default None in new() → with_*() setter → typed accessor()
pub request_meta: Option<serde_json::Value>,        // field
request_meta: None,                                  // new() default
pub fn with_request_meta(mut self, m: Option<serde_json::Value>) -> Self { self.request_meta = m; self }
```
Changing an existing field's TYPE (e.g. overloading `auth_context`) = accidental 3.0 (Pitfall 8).

### `#[non_exhaustive]` on all new public structs
**Source:** `RequestMeta` (`mod.rs:313`), `Implementation` (`mod.rs:155`), both
`RequestHandlerExtra` (`cancellation.rs:178`, `shared/cancellation.rs:49`).
**Apply to:** `ProtocolContext` and any new public struct. Note the documented semver caveat
(`shared/cancellation.rs:76-80`): `#[non_exhaustive]` only breaks POSITIONAL struct-literal
construction; `new()`/`with_*` chains stay source-compatible.

### Dual dispatch-site + wasm mirror parity (Pitfall 3)
**Source:** `request_meta` threaded at `core.rs:514`, `mod.rs` twin, + wasm mirror.
**Apply to:** `ProtocolContext` — thread at `core.rs:1118` AND `mod.rs:1245` AND the wasm
dispatch mirror. Add a cross-site parity test (era resolves identically under HTTP, stdio, wasm).

### Locking-test discipline for frozen/version-pinned constants
**Source:** `latest_version_is_2025_11_25` (`version.rs:42`),
`pending_tasks_result_preserves_minus_32002` (`task_dispatch_tests.rs:355`).
**Apply to:** the new error-code table (frozen `-32002` re-exported + its test untouched) and the
`LATEST_PROTOCOL_VERSION` pin. Some tripwires are MEANT to be updated
(`supports_four_versions_including_2024`); the frozen ones are NOT.

### `serde(rename_all = "camelCase")` + `skip_serializing_if = "Option::is_none"`
**Source:** every protocol type (`RequestMeta` `mod.rs:314`, `Implementation` `mod.rs:156`,
`ServerCapabilities` `capabilities.rs`).
**Apply to:** all new serializable protocol shapes (`ProtocolContext` if serialized,
`server/discover` result, `serverInfo`). Guarantees v1 byte-identity: an empty
`#[serde(flatten)] other` emits no keys (`mod.rs:341-343`), so reusing `RequestMeta.other` for v2
`_meta` keys keeps v1 wire output unchanged (VERS-03/09).

### Semver additive-guarantee gate (phase gate)
**Source:** N/A — new CI/dev gate. `cargo-semver-checks` + `cargo-public-api` (neither installed).
**Apply to:** the whole phase. `cargo install cargo-semver-checks --locked` then
`cargo semver-checks check-release` must classify the phase MINOR (Pitfall 5). Run before commit
alongside `make quality-gate`.

## No Analog Found

None. Every surface has a home in the tree. The two genuinely new value types (`ProtocolContext`,
`Era`/`ResultType`) map cleanly onto the `RequestMeta` newtype-builder and `ProtocolErrorCode`
C-style-enum patterns respectively; the new error-code module maps onto the existing frozen-`-32002`
literal + `ProtocolErrorCode` discipline. If any wire-exact v2 value is needed before the
2026-07-28 `schema.json` publishes, that value is deferred (structure-only), not analog-missing.

## Metadata

**Analog search scope:** `src/types/protocol/`, `src/server/`, `src/shared/`, `src/types/capabilities.rs`
**Files scanned:** version.rs, http_constants.rs, cancellation.rs (native), shared/cancellation.rs (wasm), types/protocol/mod.rs, types/capabilities.rs, server/core.rs, server/mod.rs, server/builder.rs, server/task_dispatch(.rs/_tests.rs)
**Key file:line anchors:** version.rs:3-53 · http_constants.rs:4-8 · cancellation.rs:178/208/264 · shared/cancellation.rs:49/69/115/132 · mod.rs:120/128/312/475/527 · capabilities.rs:109 · core.rs:502/1118/1145 · mod.rs(server):1245 · builder.rs:88/136/178 · task_dispatch_tests.rs:355
**Pattern extraction date:** 2026-07-22
