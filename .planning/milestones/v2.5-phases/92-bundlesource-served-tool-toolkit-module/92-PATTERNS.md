# Phase 92: BundleSource + Served-Tool Toolkit Module - Pattern Map

**Mapped:** 2026-06-10
**Files analyzed:** 16 new + 5 modified
**Analogs found:** 16 / 16 (every new file has an in-repo analog — this is a module-for-module lift, not greenfield)

> **Lift posture (D-13):** The 8 toolkit `workbook/*.rs` modules are a near-verbatim
> port of the private lighthouse `quote-pricing-server/src/workbook/` tree
> (`/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/`).
> The lighthouse is the **structural** analog (read in full per RESEARCH); the
> **in-repo** analogs below give the planner the SDK-side conventions (imports,
> feature-gating, builder-ext shape, ToolInfo discipline, purity gate) the lifted
> code must be reshaped into. Where a pattern is "lift verbatim from lighthouse,"
> the in-repo analog is the SDK idiom the lifted code must match.

## File Classification

| New/Modified File | Role | Data Flow | Closest In-Repo Analog | Match Quality |
|-------------------|------|-----------|------------------------|---------------|
| `crates/pmcp-workbook-runtime/src/bundle_source.rs` (NEW) | provider/trait | file-I/O | `crates/pmcp-server-toolkit/src/sql/mod.rs` (`SqlConnector` trait + enum + impls) | role-match |
| `crates/pmcp-workbook-runtime/src/bundle_loader.rs` (NEW) | service | transform | `crates/pmcp-workbook-runtime/src/artifact_model.rs` (`build_bundle_lock` integrity) | role-match |
| `crates/pmcp-workbook-runtime/src/manifest_model.rs` (MODIFY — add `annotations`) | model | — | same file, `CellRole::allowed_values` additive-field precedent | exact |
| `crates/pmcp-server-toolkit/src/workbook/mod.rs` (NEW) | module/builder-ext | request-response | `crates/pmcp-server-toolkit/src/builder_ext.rs` + `src/sql/mod.rs` | exact |
| `crates/pmcp-server-toolkit/src/workbook/handler.rs` (NEW) | controller/handler | request-response | lighthouse `handler.rs`; SDK idiom: `tools.rs` `ToolInfo` discipline | role-match |
| `crates/pmcp-server-toolkit/src/workbook/input.rs` (NEW) | middleware/validator | transform | lighthouse `input.rs`; SDK idiom: `tools.rs::build_input_schema` (`additionalProperties:false`) | role-match |
| `crates/pmcp-server-toolkit/src/workbook/schema.rs` (NEW) | utility | transform | `crates/pmcp-server-toolkit/src/sql/schema.rs` + `tools.rs::build_input_schema` | role-match |
| `crates/pmcp-server-toolkit/src/workbook/error.rs` (NEW) | model/utility | transform | `crates/pmcp-server-toolkit/src/error.rs` + `sql/mod.rs::ConnectorError` | role-match |
| `crates/pmcp-server-toolkit/src/workbook/render_uri.rs` (NEW) | utility/codec | transform | lighthouse `render_uri.rs` (verbatim); no SDK analog (new codec) | partial |
| `crates/pmcp-server-toolkit/src/workbook/render_resource.rs` (NEW) | resource handler | request-response | `crates/pmcp-server-toolkit/src/resources.rs` (`ResourceHandler` impl) | role-match |
| `crates/pmcp-server-toolkit/examples/workbook_server_http.rs` (NEW) | example | request-response | `crates/pmcp-server-toolkit/examples/sql_server_http.rs` | exact |
| `crates/pmcp-server-toolkit/tests/fixtures/<bundle>@1.0.0/*` (NEW) | test fixture | — | lighthouse `bundles/ufh-quote@1.0.0/` (SHAPE only, no content — D-01) | partial |
| `crates/pmcp-server-toolkit/tests/` fixture generator (NEW) | test (generator) | file-I/O | `crates/pmcp-workbook-runtime/src/artifact_model.rs` Serialize types | role-match |
| `crates/pmcp-server-toolkit/tests/workbook_*.rs` (NEW integration) | test | request-response | `crates/pmcp-server-toolkit/src/builder_ext.rs` `#[cfg(test)]` block | role-match |
| `docs/workbook-uri-spec.md` (NEW) | docs | — | `docs/workbook-dialect-spec.md` | role-match |
| `crates/pmcp-server-toolkit/src/lib.rs` (MODIFY — `pub mod workbook` + re-exports) | config/module-root | — | same file, `#[cfg(feature="http")] pub mod http;` + re-export block | exact |
| `crates/pmcp-server-toolkit/Cargo.toml` (MODIFY — `workbook` feature + `[[example]]`) | config | — | same file, `http` feature + `[[example]] sql_server_http` | exact |
| `crates/pmcp-workbook-runtime/Cargo.toml` (MODIFY — `embedded` feature + `include_dir`/`base64`) | config | — | same file, `rust_xlsxwriter` gated-dep precedent | exact |
| `Makefile` (MODIFY — toolkit `--features workbook` purity assertion) | config/CI | — | same file, `purity-check` recipe (`PURITY_CRATES`) | exact |
| `crates/pmcp-server-toolkit/deny.toml` (NEW, if PURITY_CRATES path) OR per-feature tree-assert | config/CI | — | `crates/pmcp-workbook-runtime/deny.toml` | role-match |

## Pattern Assignments

### `crates/pmcp-workbook-runtime/src/bundle_source.rs` (provider/trait, file-I/O)

**Analog:** `crates/pmcp-server-toolkit/src/sql/mod.rs` (the `SqlConnector` trait shape — Send+Sync, `#[non_exhaustive]` error enum, doctest with a local dummy impl, compile-only object-safety test).

**Critical difference from the SQL analog:** `BundleSource` is **SYNC, no `#[async_trait]`** (D-07). Do NOT copy the `#[async_trait]` line from `SqlConnector`. The trait bound is `Send + Sync` (lighthouse `state.rs` keeps the source in shared state; `'static` is not required for a borrowed `&dyn BundleSource` passed to the loader at boot).

**Trait shape to write** (dumb byte accessor — Discretion RESOLVED, RESEARCH Pattern 1):
```rust
pub trait BundleSource: Send + Sync {
    /// Read one bundle member's raw bytes (e.g. "manifest.json", "evidence/changelog.json").
    fn read_artifact(&self, name: &str) -> Result<Vec<u8>, BundleSourceError>;
    /// Enumerate member relative paths (sorted) so the loader folds the SAME set the emitter did.
    fn list_artifacts(&self) -> Result<Vec<String>, BundleSourceError>;
}
```

**Error-enum pattern** — copy the `#[non_exhaustive]` + `thiserror::Error` + `#[error("...")]` shape from `sql/mod.rs:218-272` (`ConnectorError`). One `BundleSourceError` enum (Io / NotFound / variants).

**Object-safety + Send+Sync compile-test** — copy verbatim from `sql/mod.rs:369-382`:
```rust
#[cfg(test)]
mod object_safety { fn assert_send_sync<T: Send + Sync>() {}
    #[test] fn bundle_source_object_is_send_sync() { assert_send_sync::<Box<dyn BundleSource>>(); } }
```

**Doctest with a LOCAL dummy impl** — mirror `sql/mod.rs:81-99` (the doc example defines a local struct, never references a downstream crate, to avoid circular-doctest dep).

**`include_dir` feature-gating** — the `EmbeddedSource` impl goes behind a runtime `embedded` feature (D-06). Mirror the gated-module pattern: `#[cfg(feature = "embedded")] pub mod embedded;` or `#[cfg(feature = "embedded")]` on the `EmbeddedSource` struct, exactly like `sql/mod.rs:46-49` gates `SqliteConnector` behind `sqlite`.

---

### `crates/pmcp-workbook-runtime/src/bundle_loader.rs` (service, transform)

**Analog:** `crates/pmcp-workbook-runtime/src/artifact_model.rs` (the integrity substrate it consumes) + lighthouse `mod.rs:478-541` (`load_bundle`, lifted source-driven).

**Reuse, never re-implement (RESEARCH "Don't Hand-Roll"):** Call the runtime's existing `build_bundle_lock` / `sha256_hex` / `update_field` (`artifact_model.rs:83-134`) — do NOT write a second hasher. The loader recomputes per-artifact + combined hash and compares against the on-disk `BUNDLE.lock`:
```rust
// artifact_model.rs:107 — the SINGLE hashing fn the loader must reuse (FOUND vs EXPECTED on mismatch)
pub fn build_bundle_lock(
    workflow: &str, version: &str, workbook_hash: String,
    ir_json: &str, manifest_json: &str, evidence_hash: &str,
) -> BundleLock { /* h_exec, h_manifest, h_evidence, combined */ }
```

**Boot-load skeleton (lift lighthouse `mod.rs:478-541`, swap `include_str!` → `source.read_artifact(...)`):**
```rust
pub fn load(source: &dyn BundleSource) -> Result<WorkbookBundle, BundleLoadError> {
    let lock: BundleLock = parse_member(source, "BUNDLE.lock")?;
    let evidence_hash = recompute_evidence_hash(source)?;      // path+len-prefixed, sorted (update_field)
    let recomputed = build_bundle_lock(&lock.bundle_id, &lock.version,
        lock.workbook_hash.clone(), &ir_json, &manifest_json, &evidence_hash);
    if recomputed.artifacts != lock.artifacts || recomputed.combined != lock.combined {
        return Err(BundleLoadError::IntegrityMismatch { /* FOUND vs EXPECTED */ });
    }
    verify_stamp_binding(&lock, &manifest, &layout, &changelog)?;  // WR-02 cross-check
    let dag = build_dag(&ir);                                      // built ONCE (runtime::build_dag)
    Ok(WorkbookBundle { ir, dag, manifest, changelog, stamp, .. })
}
```

**`#[deny(unwrap/expect/panic)]` discipline** — the crate already has `#![deny(clippy::unwrap_used, expect_used, panic)]` at `lib.rs:18` with a `cfg(test)` allow at `:19`. New loader code MUST honor it (use `?` and `BundleLoadError`, never `.unwrap()` on a value path).

**Pitfall 2 guard (RESEARCH):** the generator and the loader MUST fold the identical `evidence_members()` set with identical `update_field` tags/sort order, or a valid bundle false-positives. Add a test asserting `recompute_evidence_hash() == lock.artifacts.evidence` (lighthouse `mod.rs:717-723`).

---

### `crates/pmcp-workbook-runtime/src/manifest_model.rs` (model — ADD `annotations` field, D-18/S-2)

**Analog:** the **same file** — `CellRole::allowed_values` (`manifest_model.rs:119-132`) is the exact additive-field precedent.

**Copy this pattern verbatim** for the new `Manifest.annotations: Vec<AnnotationDecl>`:
```rust
// manifest_model.rs:119-132 — the additive serde-default pattern to clone for D-18
#[serde(default)]
pub tier: Option<InputTier>,
// ...
#[serde(default, skip_serializing_if = "Option::is_none")]
pub allowed_values: Option<Vec<String>>,
```
For `annotations` (D-18: a `Vec`, default-empty so old manifests deserialize clean and byte-stable snapshots are preserved):
```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub annotations: Vec<AnnotationDecl>,   // name + target cell/output + meaning
```
Derive the same trait set every model type uses: `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]` (see `CellRole`, `InputTier`).

---

### `crates/pmcp-server-toolkit/src/workbook/mod.rs` (module + builder-ext, request-response)

**Analog:** `crates/pmcp-server-toolkit/src/builder_ext.rs` (the `ServerBuilderExt` trait, **lines 34, 260-318**) is the exact registration pattern for `.with_workbook_bundle(...)` (D-09).

**Builder-ext shape — panicking convenience + `try_` fallible companion (review R7 discipline kept):**
```rust
// builder_ext.rs:34, 260-284 — the trait + panicking/try_ pair to mirror
pub trait WorkbookBuilderExt: Sized {
    /// Panics on load/verify failure. # Panics ... prefer try_with_workbook_bundle.
    fn with_workbook_bundle(self, source: &dyn BundleSource) -> Self;
    /// Fallible companion. # Errors ... returns ToolkitError on integrity/validation gap.
    fn try_with_workbook_bundle(self, source: &dyn BundleSource) -> Result<Self>;
}
impl WorkbookBuilderExt for ServerBuilder {
    fn with_workbook_bundle(self, source: &dyn BundleSource) -> Self {
        self.try_with_workbook_bundle(source)
            .expect("with_workbook_bundle: bundle load/verify failed — prefer try_with_workbook_bundle")
    }
    fn try_with_workbook_bundle(mut self, source: &dyn BundleSource) -> Result<Self> {
        let bundle = Arc::new(BundleLoader::load(source)?);   // WBSV-08 fail-closed at boot
        // register all 5 tools via self.tool_arc(name, handler) (builder_ext.rs:280-282 loop)
        // register the workbook:// resource via self.resources_arc(...)
        Ok(self)
    }
}
```
The `for (name, _info, handler) in synthesized { self = self.tool_arc(name, handler); }` loop at `builder_ext.rs:280-282` is the exact tool-registration idiom. Resource registration uses `.resources_arc(Arc::new(...))` — see the example analog (`sql_server_http.rs:66`).

**Drop the `DispatchingResource` wrapper** (RESEARCH Pattern 4 / A3): the toolkit `workbook` module has only ONE resource (`workbook://`), so register a single `RenderWorkbookResource` directly. The lighthouse dispatcher existed only to compose with `value-schema://`, which is NOT lifted.

**`tracing::warn!` visibility idiom** — copy the empty-state warning shape from `builder_ext.rs:273-279` if a bundle declares zero outputs/tools (operator-visibility, T-83-08-02 parity).

---

### `crates/pmcp-server-toolkit/src/workbook/handler.rs` (controller/handler, request-response)

**Analog (structural):** lighthouse `handler.rs` (5 `ToolHandler` impls — lift). **Analog (SDK idiom):** `crates/pmcp-server-toolkit/src/tools.rs` `ToolInfo` discipline.

**ToolInfo construction discipline (WBSV-07) — copy from `tools.rs:176-186`:** Use `ToolInfo::with_annotations` / `ToolInfo::new` constructors, NEVER struct-literals (`#[non_exhaustive]` discipline):
```rust
// tools.rs:176-186 — the constructor-not-literal ToolInfo idiom
fn build_tool_info(decl: &ToolDecl) -> ToolInfo {
    let schema = build_input_schema(&decl.parameters);
    match build_annotations(decl.annotations.as_ref()) {
        Some(ann) => ToolInfo::with_annotations(name, desc, schema, ann),
        None => ToolInfo::new(name, desc, schema),
    }
}
```

**Mandatory non-empty `outputSchema` → `structuredContent`** (WBSV-07, the locked v2.2 discipline): each of the 5 tools attaches an `outputSchema` and returns its result in `structuredContent` via the `with_ui` / widget-meta path (RESEARCH "Don't Hand-Roll": `error.rs::to_iserror_result` rides in `structuredContent`; pmcp `tool` dispatch hardcodes `is_error:false`).

**Reuse runtime compute, never re-implement** (RESEARCH "Don't Hand-Roll"):
- executor: `pmcp_workbook_runtime::run_executor` / `build_dag` (`lib.rs:97-101`)
- render: `pmcp_workbook_runtime::render::render_xlsx(layout, run)` (`render/mod.rs:227`)
- changelog: serve `bundle.changelog` (`VersionChangelog`, `lib.rs:93`) — already hash-verified at boot

**Scrub deltas (S-1, S-2, S-3) land HERE** — see Shared Patterns ▸ Scrub Deltas below.

---

### `crates/pmcp-server-toolkit/src/workbook/input.rs` (validator/middleware, transform)

**Analog (structural):** lighthouse `input.rs::validate_input` (lift verbatim with its fail-closed arms). **Analog (SDK idiom):** `tools.rs::build_input_schema` (`tools.rs:193-208`) for the `additionalProperties:false` envelope.

**Preserve every fail-closed arm — do NOT "simplify" (RESEARCH Anti-Patterns + Pitfall 3):**
- WR-05: a `cell_map` entry with no manifest role → `invalid_input` (lighthouse `input.rs:114-123`). NEVER rewrite a `?`-or-reject into `if let Some(role) = ...` (that fails open).
- WR-02: enum membership test is **string-only**, fails closed for a non-string value against a string `allowed_values` (lighthouse `input.rs:241-259`). The runtime's `allowed_values: Option<Vec<String>>` (`manifest_model.rs:131`) is the field this gate reads.
- V4 strict-constant override rejection (lighthouse `input.rs:133-138`): BA-governed constants cannot be overridden per-call. The runtime helper `is_strict_constant` (`lib.rs:78`) is the predicate.

Lift the negative-path tests too (lighthouse `input.rs:592-611`, `:456-477`).

---

### `crates/pmcp-server-toolkit/src/workbook/schema.rs` (utility, transform)

**Analog:** `crates/pmcp-server-toolkit/src/sql/schema.rs` (manifest/columns → JSON-Schema projection) + `tools.rs:193-208` (`build_input_schema`).

**`additionalProperties:false` + `required` envelope — copy the exact shape from `tools.rs:202-207`:**
```rust
// tools.rs:202-207 — the strict input-schema envelope every projected schema mirrors
json!({
    "type": "object",
    "properties": props,
    "required": required,
    "additionalProperties": false,
})
```
Per-column dtype/unit/meaning projection: lift lighthouse `schema.rs::*_output_schema` + `input_schema_for_manifest`. The `result_envelope_schema` must accept BOTH the success and `isError` shapes (WBSV-06/07).

**Scrub S-1:** drop the top-level `supply_total` field from the projected output schema (lighthouse `schema.rs:96-98`); project ALL named outputs uniformly.

---

### `crates/pmcp-server-toolkit/src/workbook/error.rs` (model/utility, transform)

**Analog:** `crates/pmcp-server-toolkit/src/error.rs` (`ToolkitError` + `Result<T>` alias) and `sql/mod.rs::ConnectorError` (`#[non_exhaustive]` + `thiserror`).

**Lift `to_iserror_result` verbatim (RESEARCH Code Examples / lighthouse `error.rs:171-192`)** — but rename the stamp field `workflow` → `bundle_id` (S-3):
```rust
// lighthouse error.rs:171 — isError envelope IN structuredContent (never protocol Err) — WBSV-06
pub fn to_iserror_result(err: &WorkbookToolError, stamp: &ProvStamp) -> Value {
    let mut obj = Map::new();
    obj.insert("isError".into(), json!(true));
    obj.insert("code".into(), json!(err.code));
    obj.insert("reason".into(), json!(err.reason));
    obj.insert("provenance".into(), stamp.to_json());   // bundle_id + version + workbook_hash (S-3)
    if let Some(f) = &err.field   { obj.insert("field".into(), json!(f)); }
    if let Some(a) = &err.allowed { obj.insert("allowed".into(), json!(a)); }
    if let Some((lo,hi)) = &err.range { obj.insert("range".into(), json!([lo,hi])); }
    if let Some(r) = &err.required { obj.insert("required".into(), json!(r)); }
    Value::Object(obj)
}
```
Keep the `WorkbookToolError` enum `#[non_exhaustive]` (mirror `ConnectorError`, `sql/mod.rs:218-219`).

---

### `crates/pmcp-server-toolkit/src/workbook/render_uri.rs` (codec, transform)

**Analog:** lighthouse `render_uri.rs` (verbatim — no in-repo codec analog). SDK idiom: `base64 0.22` (URL_SAFE_NO_PAD for the URI, STANDARD for `.xlsx` bytes) — already a workspace dep, add the toolkit dep line.

**Lift verbatim — size-guard FIRST (RESEARCH Pitfall 4 / Security V12):**
```rust
// lighthouse render_uri.rs:129-136 — the size guard checked BEFORE any decode (DoS mitigation)
if encoded.len() > MAX_ENCODED_URI_LEN { return Err(/* oversized */); }
// then total / panic-free decode
```
The URI is an **untrusted, attacker-controlled payload**. Decode must be total and panic-free (`#![deny(panic)]` already enforces it). Publish the format as a documented SDK contract (D-16) in `docs/workbook-uri-spec.md`.

---

### `crates/pmcp-server-toolkit/src/workbook/render_resource.rs` (resource handler, request-response)

**Analog:** `crates/pmcp-server-toolkit/src/resources.rs` — the `#[async_trait] impl ResourceHandler` block (`resources.rs:336-365`) is the exact `list` + `read` signature shape.

**ResourceHandler trait shape to mirror (`resources.rs:336-365`):**
```rust
#[async_trait]
impl ResourceHandler for RenderWorkbookResource {
    async fn list(&self, _cursor: Option<String>, _extra: pmcp::RequestHandlerExtra)
        -> pmcp::Result<ListResourcesResult> { /* ... */ }
    async fn read(&self, uri: &str, _extra: pmcp::RequestHandlerExtra)
        -> pmcp::Result<ReadResourceResult> {
        // 1. decode workbook:// URI (render_uri.rs)  2. VERIFY provenance == bundle stamp
        // 3. RE-VALIDATE inputs through validate_input  4. re-run executor  5. render_xlsx → base64
    }
}
```
Error path uses `pmcp::Error::protocol(pmcp::ErrorCode::..., msg)` exactly as `resources.rs:359-362`.

**Stateless regen on read (WBSV-05, V3):** bytes are recomputed per `resources/read` from the decoded URI — no server-side session. **VERIFY provenance** against the bundle stamp (lighthouse `render_resource.rs:64-86`) and **RE-VALIDATE** inputs (lighthouse `render_resource.rs:88-92`) before re-running. Content shape uses `Content::resource_with_text` / blob — see `resources.rs:190-196` for the MIME-typed-wire idiom.

---

### `crates/pmcp-server-toolkit/examples/workbook_server_http.rs` (example, request-response)

**Analog:** `crates/pmcp-server-toolkit/examples/sql_server_http.rs` — the **exact** streamable-HTTP wiring (D-12).

**Copy the serve-helper + main skeleton from `sql_server_http.rs:42-72`:**
```rust
// sql_server_http.rs:42-50 — the inline StreamableHttpServer serve helper to clone
async fn serve(server: Server) -> Result<(SocketAddr, JoinHandle<()>), Box<dyn std::error::Error>> {
    let shared = Arc::new(Mutex::new(server));
    let cfg = StreamableHttpServerConfig::default();
    Ok(StreamableHttpServer::with_config("127.0.0.1:0".parse()?, shared, cfg).start().await?)
}
```
**main body shape (`sql_server_http.rs:52-72`):** build via `Server::builder().name(...).version(...)`, chain the workbook builder-ext (`.try_with_workbook_bundle(&source)?` in place of `.try_tools_from_config_with_connector(...)`), `.build()?`, then `serve(...)`, then `println!("..._ADDR=http://{addr}")` (machine-readable bound addr).

**Source selection (Discretion):** the remote-deploy story (D-12) reads best with the `EmbeddedSource` as default + an optional `--bundle-dir` override (mirrors the example's `PMCP_ASSETS_DIR` harness seam, `sql_server_http.rs:36,54`).

**Cargo `[[example]]` wiring — copy `Cargo.toml:128-130` exactly, swap features:**
```toml
[[example]]
name = "workbook_server_http"
required-features = ["workbook", "http"]   # http already forwards pmcp/streamable-http
```
**dev-deps already present:** `tokio` with `rt-multi-thread` (`Cargo.toml:140`) is required for `#[tokio::main]` examples — no new dev-dep needed for the runtime.

---

### `crates/pmcp-server-toolkit/tests/fixtures/<bundle>@1.0.0/` + generator (test fixture, file-I/O)

**Analog:** `crates/pmcp-workbook-runtime/src/artifact_model.rs` Serialize types (`BundleLock`, `CellMap`, `CellEntry`) — the generator constructs and serializes these. Bundle **shape** reference: lighthouse `bundles/ufh-quote@1.0.0/` (artifact names + `BUNDLE.lock` structure ONLY — **NO content**, D-01).

**Generator pattern:** build the 7 artifacts through the runtime's `Serialize` types, then call `build_bundle_lock(...)` (`artifact_model.rs:107`) so the golden's `BUNDLE.lock` is computed by the same fn the loader recomputes against. Byte-identical regeneration is a CI check (D-03).

**Tax-domain identifiers ONLY** (S-4 / D-02): `gross_income`, `filing_status`, `tax_owed`, bracket tables. Grep-gate `ufh|quote|coil|towelrad|plot.?3` = 0 hits.

**Negative-path tests (D-05, WBSV-06/08):** copy the golden to a tempdir and corrupt programmatically (flip a byte / delete an artifact / desync `BUNDLE.lock` version). No committed corrupt fixtures.

---

### `crates/pmcp-server-toolkit/tests/workbook_*.rs` (integration test, request-response)

**Analog:** `crates/pmcp-server-toolkit/src/builder_ext.rs` `#[cfg(test)] mod tests` (`builder_ext.rs:381-448`) — build a server via the builder-ext, assert `server.get_tool("...").is_some()`:
```rust
// builder_ext.rs:404-416 — the build-and-assert-registered idiom for the 5 workbook tools
let server = Server::builder().name("test").version("0.1.0")
    .with_workbook_bundle(&source).build().expect("build");
assert!(server.get_tool("calculate").is_some());
```
ALWAYS-requirements (CLAUDE.md): add proptest round-trip/determinism for the URI codec + validation invariants (the project mandates fuzz + property + unit + a working example).

---

### `docs/workbook-uri-spec.md` (docs)

**Analog:** `docs/workbook-dialect-spec.md` — the published-contract precedent the `workbook://` URI doc follows (D-16). Same structure: format definition, size bound, scheme, versioning-decision note.

---

### `crates/pmcp-server-toolkit/src/lib.rs` (MODIFY — module decl + re-exports)

**Analog:** the **same file** — `#[cfg(feature = "http")] pub mod http;` (`lib.rs:43-44`) is the exact feature-gated module-decl precedent.

**Add (mirror `lib.rs:43-44`):**
```rust
/// Workbook served-tool module (Phase 92). Gated on the opt-in `workbook` feature.
#[cfg(feature = "workbook")]
pub mod workbook;
```

**Re-export the boot surface (D-11)** — mirror the `http` crate-root re-export block (`lib.rs:137-138`) so Shape A/B consumers never name `pmcp-workbook-runtime`:
```rust
#[cfg(feature = "workbook")]
pub use crate::workbook::{WorkbookBuilderExt /* + BundleSource, both impls, loader/error types */};
```
The compile-only `_ROOT_REEXPORT_SMOKE` const (`lib.rs:206-230`) is the pattern for proving the re-export paths resolve — extend it for the `workbook` surface.

---

### `crates/pmcp-server-toolkit/Cargo.toml` (MODIFY — `workbook` feature)

**Analog:** the **same file** — the `http` feature block (`Cargo.toml:98-107`) is the exact precedent (gates a module + forwards a downstream feature).

**Add (mirror the `http` block):**
```toml
pmcp-workbook-runtime = { version = "0.1.0", path = "../pmcp-workbook-runtime", optional = true }
base64 = { version = "0.22", optional = true }   # already present (Cargo.toml:70), reuse
[features]
workbook = ["dep:pmcp-workbook-runtime", "dep:base64", "pmcp-workbook-runtime/embedded"]
```
`workbook` is **NOT in `default`** (D-10) — `default = ["code-mode"]` stays unchanged (`Cargo.toml:74`).

---

### `crates/pmcp-workbook-runtime/Cargo.toml` (MODIFY — `embedded` feature + `include_dir`)

**Analog:** the **same file** — the `rust_xlsxwriter` gated-dep block (`Cargo.toml:29-38`) is the precedent, including the **human-verify install discipline** for a new external crate.

**Add (mirror the rust_xlsxwriter comment+gate discipline):**
```toml
include_dir = { version = "0.7.4", optional = true }   # EmbeddedSource — gated behind human-verify (A1)
base64 = { version = "0.22" }                           # workbook:// URI codec + xlsx bytes
[features]
embedded = ["dep:include_dir"]                          # name is Claude's discretion
```
> `include_dir` is `[ASSUMED]` (RESEARCH Legitimacy Audit) — the planner MUST gate its install behind a `checkpoint:human-verify` task, exactly as `rust_xlsxwriter` was gated (the `Cargo.toml:29-38` comment documents that discipline: author/license/repo/`cargo audit` clean before adding the dep).

---

### `Makefile` (MODIFY — toolkit `workbook` purity assertion)

**Analog:** the **same file** — the `purity-check` recipe (`Makefile:496-545`) is **explicitly designed to be extended** (`Makefile:492-494` comment).

**Critical:** the toolkit is NOT unconditionally reader-free (it has `code-mode`/`sql`/`http`), so do NOT append `pmcp-server-toolkit` to `PURITY_CRATES`. Add a **separate per-feature-combination assertion** (RESEARCH Pitfall 1 / A5): a `cargo tree -p pmcp-server-toolkit --features workbook` check asserting `umya|calamine|quick-xml|swc_|pmcp-code-mode` absent. Reuse the same fail-closed capture idiom (`Makefile:506`):
```sh
status=0; tree=$$(cargo tree -p pmcp-server-toolkit --features workbook 2>&1) || status=$$?;
# fail closed on non-zero status; grep -Ei "$$BAN" → FAIL if reader/JS present
```
The `BAN='umya|calamine|quick-xml|swc_|pmcp-code-mode'` regex (`Makefile:503`) is reused verbatim.

## Shared Patterns

### Provenance stamp (`ProvStamp`) — D-15 / S-3
**Source:** lighthouse `error.rs` / `mod.rs:174-193`; **Apply to:** every tool response AND every error envelope (`handler.rs`, `error.rs`, `render_resource.rs`).
The stamp is **bundle_id + version + `BUNDLE.lock` combined hash** — the minimal triple tying a response to one verified bundle. **Rename the lighthouse `ProvStamp.workflow` → `bundle_id`** at the served boundary. (Open Question 1: keep the on-disk `BundleLock` field name as-is for Phase-93 producer/consumer agreement, OR rename to `bundle_id` per D-17 — D-17 says the SDK frozen contract renames it; confirm the on-disk vs served-only scope during planning.)

### Fail-closed integrity at boot (WBSV-08)
**Source:** `BundleLoader::load` over `artifact_model.rs::build_bundle_lock`; **Apply to:** the single boot path only (`bundle_loader.rs`), reached by EVERY `BundleSource` impl through the byte-accessor split so no impl can bypass it.

### isError-in-structuredContent (WBSV-06)
**Source:** `error.rs::to_iserror_result` (lift verbatim); **Apply to:** every tool handler's failure path. NEVER return `Err(pmcp::Error)` for a domain failure — the `tool` dispatch hardcodes `is_error:false`, so the only machine-actionable path is `structuredContent` (RESEARCH "Don't Hand-Roll").

### Mandatory non-empty `outputSchema` → `structuredContent` (WBSV-07)
**Source:** `tools.rs:176-208` (`ToolInfo` constructors + `additionalProperties:false` envelope); **Apply to:** all 5 tool handlers + `schema.rs`. This is the locked v2.2 SQL/OpenAPI parity discipline.

### `#![deny(unwrap_used, expect_used, panic)]` on value paths
**Source:** `pmcp-workbook-runtime/src/lib.rs:18-19` (with `cfg(test)` allow); **Apply to:** the new runtime modules (`bundle_source.rs`, `bundle_loader.rs`) and — per CLAUDE.md/Phase-91 convention — the toolkit `workbook` modules. Use `?` + typed errors, never `.unwrap()`/`.expect()` on a value path.

### Scrub Deltas (D-13 — the ONLY deliberate divergences from a verbatim lift)
**Apply to:** ALL lifted `workbook/*.rs` + the fixture. Each is grep-verifiable:

| # | Site (lighthouse) | Delta | WBSV |
|---|-------------------|-------|------|
| S-1 | `CellMap.supply_total_cell`; `handler.rs::read_supply_total`; `schema.rs` `supply_total` field | Remove privileged headline; project ALL named outputs uniformly. (In-repo `artifact_model.rs:46-48` `CellMap.supply_total_cell` may need a runtime edit OR be left optional/unused — A4.) | WBSV-01/07 |
| S-2 | `CoilBandAnnotation`; `handler.rs` keystone step | Replace with manifest-declared `annotations` (D-14/D-18; add `Manifest.annotations` field). | WBSV-02 |
| S-3 | `ProvStamp.workflow` | Rename → `bundle_id`. | WBSV-01/06 |
| S-4 | every `ufh`/`quote`/`UFH`/`coil`/`Plot-3`/`7_Quote!`/`first_fix`/`heat_source`/`margin` string in code/comments/tests/docs | Strip every customer identifier; use tax-domain names. **Grep-gate = 0 hits** in `crates/pmcp-server-toolkit/src/workbook/` + the fixture. NOTE: the in-repo `artifact_model.rs` tests still use `"ufh-quote"` (`:144-247`) — confirm whether those test strings are in scope for the scrub or are runtime-crate-local (they are NOT in the served `workbook/` tree, so likely out of S-4's grep scope, but flag during planning). | D-01/D-13 |

**NOT lifted (customer-specific, deleted per D-13):** lighthouse `tools/quote.rs`, `rot.rs`, `quote_pricing_core` catalog, `value-schema://` resource, `reload_catalog`/`get_catalog_info`.

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `crates/pmcp-server-toolkit/src/workbook/render_uri.rs` | codec | transform | No in-repo URI codec exists; lift the lighthouse `render_uri.rs` verbatim. The only SDK convention to apply is the `base64 0.22` workspace dep + size-guard-first discipline (RESEARCH Pitfall 4). |

Every other new file has a strong in-repo analog (this phase is a module-for-module lift onto established SDK conventions, not greenfield design).

## Metadata

**Analog search scope:** `crates/pmcp-server-toolkit/src/` (builder_ext, lib, sql/, http/, resources, tools, error), `crates/pmcp-workbook-runtime/src/` (lib, artifact_model, manifest_model, render/), `crates/pmcp-server-toolkit/examples/`, `crates/pmcp-server-toolkit/Cargo.toml`, `crates/pmcp-workbook-runtime/Cargo.toml`, `Makefile` purity gate.
**Files scanned:** 11 in-repo (read in full or targeted) + the RESEARCH-summarized lighthouse tree (8 modules, read by the researcher).
**Pattern extraction date:** 2026-06-10
