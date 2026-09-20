# Phase 92: BundleSource + Served-Tool Toolkit Module - Research

**Researched:** 2026-06-10
**Domain:** MCP served-tool layer extraction (bundle loading, fail-closed integrity, manifest-driven schema projection) into `pmcp-server-toolkit`
**Confidence:** HIGH (the lift source is read in full; the in-repo runtime + toolkit patterns are read directly)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Test bundle**
- **D-01:** Test bundle is a **hand-authored synthetic fixture**, NOT a copy of lighthouse `ufh-quote@1.0.0`. **Hard constraint: no TowelRads customer data or business logic anywhere** (fixtures, code, comments, identifiers, docs).
- **D-02:** Fixture domain = a **realistic common Excel use case, no sensitive data — tax calculation with bracket rules** — a couple of steps from an input sheet to an output sheet. This synthetic bundle is the golden contract every WBSV test runs against.
- **D-03:** Production mechanism = **generator + committed golden.** A test-support Rust generator builds the bundle through `pmcp-workbook-runtime` Serialize types and writes the artifacts to `crates/.../tests/fixtures/<bundle>@<version>/`. Committed files are the golden; regeneration must be byte-identical (CI check). Phase 93 later re-emits the same workbook through the real compiler and diffs against this golden.
- **D-04:** Coverage = **full-surface fixture** — multiple named outputs (no privileged headline), enum + numeric + string inputs across tiers, units, governed data, a v1.0.0→v1.1.0 changelog pair (so `diff_version` is real), and a layout descriptor (so `render_workbook` is real). One bundle exercises all five tools.
- **D-05:** Negative paths (WBSV-06/08): tests **copy the golden to a tempdir and corrupt it programmatically** (flip a byte, delete an artifact, desync `BUNDLE.lock` version). No committed corrupt fixtures; each tamper lives next to its test.

**BundleSource trait**
- **D-06:** The trait + local-dir + embedded impls live in **`pmcp-workbook-runtime`**. The `include_dir` dependency goes **behind a feature flag**. Purity gate keeps covering the crate (reader-free).
- **D-07:** **Sync trait** — bundles load once at boot; no tokio/async_trait in the runtime. S3/registry seam: implementors fetch/cache ahead of boot (or `block_on`).
- **D-08:** Addressing: **one source instance = one `bundle@version`** (e.g. `LocalDirSource::new("bundles/tax-calc@1.0.0")`). Version pinning at construction. Multi-workbook mapping is Phase 94.

**Toolkit integration**
- **D-09:** Registration API = a **`builder_ext` extension method** (e.g. `.with_workbook_bundle(source)`) mirroring SQL/OpenAPI — loads + verifies the bundle, registers all five tools and the `workbook://` resource. Config-file wiring waits for Shape A (Phase 95).
- **D-10:** Feature flag = **`workbook`, NOT in default features** (mirrors `http`). Gates the module + the `pmcp-workbook-runtime` dep. The purity-gate feature matrix gains the toolkit `workbook` combination.
- **D-11:** The toolkit module **re-exports the boot surface** (`BundleSource`, both impls, loader/error types) so Shape A/B consumers depend only on `pmcp-server-toolkit` and never name the runtime crate.
- **D-12 (explicit):** The mandatory runnable example serves over **streamable-HTTP, NOT stdio**. Wire `required-features = ["workbook", "http"]`. One example, all five tools, against the synthetic tax-calc fixture.

**Served layer (lift posture)**
- **D-13:** **Reuse the lighthouse's generic engine code, scrubbed.** Port the served-layer machinery (input validation, manifest-driven schema projection, error envelopes, trace assembly, render plumbing) with the mandated deltas: delete quote-specific tools, kill `build_reference_manifest` from non-test paths, WR-02/WR-05 hardening, no privileged headline output. **Zero customer-named identifiers survive in SDK code, comments, fixtures, or docs.**
- **D-14:** `coil_band` generalization (WBSV-02) = **manifest-declared annotations** — the manifest declares which cells/outputs carry named annotations; `explain` emits a generic `annotations` object keyed by manifest-declared names. The engine knows nothing domain-specific; the tax fixture demonstrates it.
- **D-15:** Provenance stamp (every tool response + error envelope) = **bundle_id + bundle version + `BUNDLE.lock` combined hash** — the minimal set tying any response to one exact verified bundle. Dialect-version field waits for Phase 96.
- **D-16:** The **`workbook://` URI format is a documented public SDK contract** — lift the lighthouse `render_uri` scheme (inputs + provenance encoded, stateless regeneration on `resources/read`) and publish its format in the SDK docs alongside the dialect spec.

### Claude's Discretion
- **Trait surface granularity (D-06/D-07):** "you decide" on raw-bytes accessor vs parsed-aggregate. Leaning: **dumb byte accessor (`read_artifact(name)` / `list_artifacts()`) + a single shared `BundleLoader`** that does parse + `BUNDLE.lock` recomputation + fail-closed checks identically for every source, so no impl can bypass WBSV-08. (Validated below against lighthouse `state.rs`/`lib.rs` — **recommendation: ADOPT.**)
- Bundle-source choice within the example (embedded default vs `--bundle-dir` override).
- Exact synthetic tax-workbook content; fixture/bundle naming; byte-stability CI check mechanics.
- Module file layout inside `pmcp-server-toolkit/src/workbook/`; error-code taxonomy naming; `include_dir` feature name in the runtime crate.

### Deferred Ideas (OUT OF SCOPE)
- **S3/registry BundleSource impls** — documented extension seam only (WBSV-09).
- **Store-style BundleSource API** (`resolve(bundle_id, version)`, `list_versions`) — rejected (D-08).
- **Config-file (ServerConfig) workbook section** — Phase 95 Shape A.
- **Dialect version in the provenance stamp** — Phase 96 (WBDL-02).
- **Extended provenance stamp** (runtime crate version, compile timestamp) — rejected for v1.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| WBSV-01 | `calculate` — typed/tier/dtype/enum-gated inputs → ALL named outputs (`{value,unit}` each) + provenance, no privileged headline | Lift `handler.rs::CalculateHandler` + `input.rs::validate_input`; **scrub `supply_total` headline + `supply_total_cell`** (§ Scrub Deltas) |
| WBSV-02 | `explain` — ordered per-cell business-language trace, with annotations generalized from `coil_band` | Lift `handler.rs::ExplainHandler::render_steps`; **replace `coil_band` with manifest-declared `annotations`** (D-14, § Scrub Deltas) |
| WBSV-03 | `get_manifest` — inputs/outputs/governed/versions/changelog projection | Lift `handler.rs::curated_manifest` + `GetManifestHandler` |
| WBSV-04 | `diff_version` — recorded hash-verified prev→current changelog | Lift `diff_version.rs` verbatim (already generic); runtime `VersionChangelog` exists |
| WBSV-05 | `render_workbook` — provenance-bound `workbook://` URI, stateless regen on `resources/read` | Lift `handler.rs::RenderWorkbookHandler` + `render_uri.rs` + `render_resource.rs`; runtime `render_xlsx` exists |
| WBSV-06 | Structured `isError:true` envelope in `structuredContent` (never protocol `Err`) with `code`/`reason`/`allowed`/`required`/`range` + provenance | Lift `error.rs::WorkbookToolError` + `to_iserror_result` verbatim (already generic) |
| WBSV-07 | Schemas projected entirely from manifest (`additionalProperties:false`, per-column dtype/unit/meaning; mandatory non-empty `outputSchema`) — parity with SQL/OpenAPI `TypedToolWithOutput` | Lift `schema.rs`; **scrub `supply_total` top-level field** (§ Scrub Deltas) |
| WBSV-08 | Recompute `BUNDLE.lock` combined hash at boot; fail closed before serving | Lift `mod.rs::load_bundle` integrity logic; **add a reusable `BundleLoader` so every source verifies identically** (Discretion item) |
| WBSV-09 | `BundleSource` trait with local-dir + embedded (`include_dir!`) impls; S3/registry = documented seam | NEW trait in runtime (D-06/07/08); `include_dir 0.7.4` behind feature flag |
</phase_requirements>

## Summary

Phase 92 is a **direct, near-verbatim lift** of the lighthouse `quote-pricing-server/src/workbook/` served layer (3,766 LOC across 8 modules) into a feature-gated `workbook` module in `pmcp-server-toolkit`, plus a **new `BundleSource` trait** in `pmcp-workbook-runtime`. The compute substrate (executor, DAG, manifest model, `BundleLock` hashing, `render_xlsx`, `VersionChangelog`) already exists in-repo from Phase 91 — this phase wires the *consumer side* of the bundle contract and freezes it.

The lighthouse served code is **already ~95% generic and manifest-driven**. The work is: (1) move it module-for-module into `crates/pmcp-server-toolkit/src/workbook/`, swapping `include_str!` of a hardcoded bundle path for a `BundleSource`-driven load; (2) apply the four mandated scrub deltas (kill the `supply_total` privileged headline → all-named-outputs; replace `coil_band` with manifest-declared `annotations`; rename `ProvStamp.workflow`→`bundle_id`; strip every `ufh`/`quote`/`coil`/`UFH`/`Plot-3` identifier); (3) extract the `load_bundle` integrity logic into a shared `BundleLoader` so every `BundleSource` impl is verified identically (no impl can bypass WBSV-08); (4) build a synthetic tax-calc golden fixture via a Serialize-types generator; (5) ship one streamable-HTTP example over the toolkit `http` feature.

**Primary recommendation:** Lift the eight lighthouse modules **module-for-module** into `crates/pmcp-server-toolkit/src/workbook/{mod,handler,input,schema,error,diff_version,render_resource,render_uri}.rs`; add `bundle_source.rs` + a shared `BundleLoader` in the runtime; mirror the `ServerBuilderExt` pattern for `.with_workbook_bundle(source)`; extend the purity gate by appending the toolkit `--features workbook` combo. Adopt the **dumb-byte-accessor `BundleSource` + shared `BundleLoader`** design — it is the only shape that guarantees WBSV-08 cannot be bypassed.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Bundle byte access (local dir / embedded) | `pmcp-workbook-runtime` (`BundleSource`) | — | Reader-free leaf; sits beside the artifact model + `BUNDLE.lock` hashing it returns/verifies (D-06) |
| Parse + boot integrity (recompute combined hash, fail closed) | `pmcp-workbook-runtime` (`BundleLoader`) | — | One shared verifier so no source impl bypasses WBSV-08; uses runtime's own `build_bundle_lock` |
| Manifest→schema projection (input/outputSchema) | `pmcp-server-toolkit :: workbook` (schema.rs) | runtime `Manifest` | Toolkit owns the MCP `ToolInfo` shape; reads the runtime model |
| Input validation (tier/dtype/enum, fail-closed) | `pmcp-server-toolkit :: workbook` (input.rs) | runtime `Manifest`/`CellMap` | Untrusted-input boundary; consumes runtime model, runs runtime executor |
| Tool execution (calculate/explain/render/diff/manifest) | `pmcp-server-toolkit :: workbook` (handler.rs) | runtime executor + `render_xlsx` | Native `ToolHandler` impls registered via `tool`/`tool_arc` |
| `workbook://` URI codec + stateless regen-on-read | `pmcp-server-toolkit :: workbook` (render_uri/resource.rs) | runtime `render_xlsx` | Resource handler; bytes recomputed per read (Lambda-safe) |
| `isError` envelope (structuredContent) | `pmcp-server-toolkit :: workbook` (error.rs) | — | MCP result-shape concern; rides in `structuredContent` |
| HTTP transport | `pmcp` (streamable-http via toolkit `http` feature) | — | Already forwarded; example needs no new transport plumbing (D-12) |

## Standard Stack

### Core (already in-repo — no new crates for compute)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `pmcp-workbook-runtime` | 0.1.0 (path) | IR/executor/DAG, `Manifest`/`CellMap`/`BundleLock`+hashing, `render_xlsx`, `VersionChangelog` | Phase 91 leaf; the module's entire compute substrate `[VERIFIED: crates/pmcp-workbook-runtime/src/lib.rs]` |
| `pmcp-server-toolkit` | 0.1.0 (path) | Host crate; gains `workbook` feature + module | Mirrors `sql`/`http` feature-gated modules `[VERIFIED: src/lib.rs:43-44]` |
| `pmcp` | 2.9.0 (path, default-features=false) | `ToolHandler`/`ResourceHandler`/`ServerBuilder`/`ToolInfo::with_ui`/`with_output_schema`; `streamable-http` via toolkit `http` | Already the toolkit's core dep `[VERIFIED: toolkit Cargo.toml]` |

### Supporting (new deps for this phase)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `include_dir` | 0.7.4 | Embed a bundle directory tree into the binary for `EmbeddedSource` | Behind a runtime feature flag (D-06); the `EmbeddedSource` impl only `[ASSUMED]` — see Package Legitimacy Audit |
| `base64` | 0.22 (workspace) | `workbook://` URI codec (URL_SAFE_NO_PAD) + `.xlsx` bytes (STANDARD) in `resources/read` | Already a workspace dep `[VERIFIED: Cargo.toml:82]`; runtime/toolkit must add the dep line |
| `url` | 2.5 (workspace) | Not strictly required (the lighthouse codec is hand-rolled string-splitting) | Already a workspace dep `[VERIFIED: Cargo.toml:62]`; optional |
| `sha2` / `hex` | 0.11 / 0.4 | Combined-hash recompute at boot | Already runtime deps `[VERIFIED: runtime Cargo.toml]` |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `include_dir` for embedding | `include_str!` per artifact (lighthouse approach) | `include_str!` requires naming every artifact at compile time (the lighthouse hardcodes 11 `include_str!` consts); `include_dir!` embeds the whole tree → generic, matches the D-08 "embedded = the one baked-in bundle". Use `include_dir!` for the generic `EmbeddedSource`. |
| Parsed-aggregate `BundleSource` (returns `WorkbookBundle`) | dumb byte accessor + shared `BundleLoader` | Parsed-aggregate lets each impl run its own parse/verify → WBSV-08 bypass risk. The byte-accessor + shared loader guarantees identical fail-closed verification. **ADOPT byte accessor.** |
| `base64` 0.22 | `data-encoding` | `base64` already a workspace dep; the lighthouse codec uses it. No reason to switch. |

**Installation:**
```bash
# In crates/pmcp-workbook-runtime/Cargo.toml — gated behind the include_dir feature
include_dir = { version = "0.7.4", optional = true }
base64 = { version = "0.22" }                 # URI codec + xlsx bytes
[features]
embedded = ["dep:include_dir"]                # name is Claude's discretion

# In crates/pmcp-server-toolkit/Cargo.toml
pmcp-workbook-runtime = { version = "0.1.0", path = "../pmcp-workbook-runtime", optional = true }
base64 = { version = "0.22", optional = true }
[features]
workbook = ["dep:pmcp-workbook-runtime", "dep:base64", "pmcp-workbook-runtime/embedded"]
```

**Version verification:** `include_dir = "0.7.4"`, `base64 = "0.22.1"` `[VERIFIED: cargo search]`. Note slopcheck caveat below.

## Package Legitimacy Audit

> slopcheck could not be installed in this session (no `pip`/network guarantee). Per protocol, the one NEW external package is tagged `[ASSUMED]`; the planner must gate its install behind a `checkpoint:human-verify` task.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `include_dir` | crates.io | mature (0.7.x line, years) | high (broadly used) | github.com/Michael-F-Bryan/include_dir | unavailable | **`[ASSUMED]` — gate behind checkpoint:human-verify before install** |
| `base64` | crates.io | already a workspace dep (`Cargo.toml:82`) | — | — | n/a (existing) | Approved (existing workspace dep) |
| `url` | crates.io | already a workspace dep (`Cargo.toml:62`) | — | — | n/a (existing) | Approved (existing; optional, likely unused) |

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none — but `include_dir` is `[ASSUMED]` until the human-verify checkpoint confirms author (`Michael-F-Bryan`), license (MIT/Apache), repo, and `cargo audit` clean. This mirrors the Phase-91 `rust_xlsxwriter` install discipline (a gated human-verify per the runtime Cargo.toml comment).

## Architecture Patterns

### System Architecture Diagram

```
                         BOOT (once, sync — D-07)
  CLI/example ── selects ──► BundleSource (LocalDirSource | EmbeddedSource)
                                  │  read_artifact(name) / list_artifacts()  (dumb bytes)
                                  ▼
                         BundleLoader::load(&source)      ← SHARED, in runtime (WBSV-08)
                                  │
              ┌───────────────────┼───────────────────────────────┐
              │  1. read all members as bytes                       │
              │  2. recompute evidence hash + combined BUNDLE.lock  │  FAIL CLOSED on:
              │     hash-of-hashes via build_bundle_lock            │  - parse error
              │  3. verify recomputed == lock (per-artifact+combined)│  - hash mismatch
              │  4. verify stamp binding (lock↔manifest/layout/changelog)│ - stamp skew
              │  5. parse IR→Cell map, build DAG once                │
              └───────────────────┬───────────────────────────────┘
                                  ▼
                       WorkbookBundle { ir, dag, manifest, cell_map,
                                        layout, annotations, changelog, stamp }   ← Arc, shared
                                  │
       .with_workbook_bundle(source)  (ServerBuilderExt, mirrors SQL/OpenAPI)
                                  ▼
        ServerBuilder ── registers 5 tools + 1 resource ──► pmcp::Server  (streamable-HTTP)

                         SERVE (per request, stateless)
  calculate ─► validate_input (tier/dtype/enum, FAIL CLOSED) ─► run_executor(ir,dag,env)
              ─► project ALL named outputs {value,unit} ─► + provenance stamp
              (domain failure ─► to_iserror_result ─► isError:true IN structuredContent)
  explain    ─► same validate+run ─► ordered per-cell trace + manifest-declared annotations
  get_manifest ─► curated manifest projection (no input)
  diff_version ─► serve recorded VersionChangelog (already hash-verified at boot)
  render_workbook ─► validate ─► encode workbook:// URI (provenance+inputs) ─► return pointer
  resources/read workbook:// ─► decode ─► VERIFY provenance ─► RE-VALIDATE ─► re-run ─► render_xlsx ─► base64
```

### Recommended Project Structure
```
crates/pmcp-workbook-runtime/src/
├── bundle_source.rs   # NEW: BundleSource trait + LocalDirSource + EmbeddedSource (include_dir, feature-gated)
├── bundle_loader.rs   # NEW: BundleLoader (lift mod.rs::load_bundle integrity logic, source-driven)
└── (existing: artifact_model.rs, manifest_model.rs, changelog.rs, render/, sheet_ir/, ...)

crates/pmcp-server-toolkit/src/workbook/   # NEW module, #[cfg(feature = "workbook")]
├── mod.rs             # register_workbook_tools + WorkbookBundle re-export + boot-surface re-exports (D-11)
├── handler.rs         # 5 native ToolHandler impls (lift handler.rs + diff_version.rs)
├── input.rs           # validate_input (tier/dtype/enum, fail-closed WR-01/02/05)
├── schema.rs          # manifest→input/outputSchema projection (WBSV-07)
├── error.rs           # WorkbookToolError + to_iserror_result (WBSV-06)
├── render_uri.rs      # workbook:// codec (WBSV-05, D-16)
└── render_resource.rs # stateless regen-on-read + scheme dispatch

crates/pmcp-server-toolkit/
├── examples/workbook_server_http.rs       # D-12 streamable-HTTP, required-features=["workbook","http"]
├── examples/fixtures/tax-calc@1.0.0/      # the synthetic golden bundle (D-03)
└── tests/fixtures/...                      # committed golden + generator (D-03/D-05)

docs/workbook-uri-spec.md                   # NEW: workbook:// public contract (D-16)
```

### Pattern 1: Shared `BundleLoader` over a dumb-byte `BundleSource` (WBSV-08/09 — Discretion RESOLVED)
**What:** `BundleSource` exposes only `read_artifact(name) -> Result<Vec<u8>>` and `list_artifacts()`. A single `BundleLoader::load(&dyn BundleSource) -> Result<WorkbookBundle, BundleLoadError>` performs parse + combined-hash recompute + fail-closed verification identically for every source.
**When to use:** Always — it is the only shape where no source impl can skip WBSV-08.
**Why the lighthouse validates this:** `state.rs`/`lib.rs` prove the lighthouse already bound the *source* once at boot and kept it in shared state, with **one** `load_bundle()` doing all integrity work (`mod.rs:478-541`). The lighthouse coupled source-selection (`select_source`, `lib.rs:58-72`) to a single load path. The byte-accessor split makes that coupling a *type-level guarantee* rather than a convention.
**Example (sync trait — D-07):**
```rust
// Source: derived from lighthouse mod.rs::load_bundle + state.rs select_source pattern
pub trait BundleSource: Send + Sync {
    /// Read one bundle member's raw bytes (e.g. "manifest.json", "evidence/changelog.json").
    fn read_artifact(&self, name: &str) -> Result<Vec<u8>, BundleSourceError>;
    /// Enumerate member relative paths (sorted) so the loader folds the SAME set the emitter did.
    fn list_artifacts(&self) -> Result<Vec<String>, BundleSourceError>;
}
// LocalDirSource::new("bundles/tax-calc@1.0.0")  — one source = one bundle@version (D-08)
// EmbeddedSource — include_dir!-baked single bundle (feature-gated)
```

### Pattern 2: Fail-closed boot integrity (lift verbatim, source-driven)
**What:** Recompute the evidence-dir hash and the `BUNDLE.lock` combined hash-of-hashes via the runtime's own `build_bundle_lock`; reject on any per-artifact or combined mismatch; cross-check the identity/provenance triple against hash-covered members (`verify_stamp_binding`).
**Source:** `mod.rs:331-541` (lift `evidence_members`, `recompute_evidence_hash`, `recompute_lock`, `verify_stamp_binding`, `load_bundle`). The combined-hash recompute and the `BundleLoadError` variants (`Parse`/`StampMismatch`/`IntegrityMismatch`) carry over unchanged — only the byte source changes from `include_str!` consts to `source.read_artifact(...)`.

### Pattern 3: Manifest-declared annotations (D-14, the `coil_band` generalization — WBSV-02)
**What:** Replace the lighthouse `CoilBandAnnotation` / `coil_band` evidence read with a generic, manifest-declared `annotations` map. The manifest declares which cells/outputs carry named annotations; `explain` emits a generic `annotations` object keyed by those names.
**Why:** `coil_band` is the single most customer-specific construct in the served layer (`mod.rs:148-170`, `handler.rs:242-256` render it as a hardcoded keystone step with "the workbook corrected the code" prose and a `workbook_rule` field). The tax fixture demonstrates the generic mechanism (e.g. bracket-boundary annotations).
**Design note:** This needs a runtime model addition — a manifest-level annotation declaration (the in-repo `manifest_model.rs` does not yet carry one; `GovernedDatum` and `CellRole` exist but no annotation field). Plan a small additive `Manifest` field (e.g. `annotations: Vec<AnnotationDecl>`) in the runtime, populated by the fixture generator. Keep it serde-default-`None`/empty so older manifests deserialize clean (the runtime's `allowed_values` field uses exactly this pattern, `manifest_model.rs:128-132`).

### Pattern 4: `ServerBuilderExt::with_workbook_bundle` (D-09, mirrors SQL/OpenAPI)
**What:** A `builder_ext`-style extension method that loads + verifies a `BundleSource`, then registers all five tools and the `workbook://` resource.
**Source:** `builder_ext.rs:260-318` is the exact pattern (`impl ServerBuilderExt for ServerBuilder`, panicking + `try_` fallible companion). The lighthouse `register_workbook_tools` (`mod.rs:556-578`) is the body to fold in.
**Example:**
```rust
// Source: derived from builder_ext.rs ServerBuilderExt + lighthouse register_workbook_tools
pub trait WorkbookBuilderExt: Sized {
    fn with_workbook_bundle(self, source: &dyn BundleSource) -> Self;       // panics on load failure
    fn try_with_workbook_bundle(self, source: &dyn BundleSource) -> Result<Self>; // fallible companion
}
```
Note the lighthouse registers a *scheme-dispatching* resource because it ALSO had a `value-schema://` resource (`render_resource.rs:138-179`, `lib.rs:127-131`). The toolkit `workbook` module has only the one `workbook://` resource, so a single `RenderWorkbookResource` registered via `.resources(...)` suffices — **drop the `DispatchingResource` wrapper** unless composing with another resource handler.

### Anti-Patterns to Avoid
- **Per-source parse/verify:** never let a `BundleSource` impl return a parsed `WorkbookBundle`; it lets an impl skip the integrity gate (WBSV-08 bypass). Byte accessor + shared loader only.
- **`if let Some(role) = ...` validation skips:** the lighthouse explicitly fails closed when a `cell_map` entry has no manifest role (`input.rs:114-123`, WR-05). Preserve every fail-closed arm; do NOT "simplify" a `?`-or-skip into an `if let Some`.
- **Privileged headline output:** the lighthouse `supply_total` top-level field + `cell_map.supply_total_cell` is the D-13/§5 headline anti-pattern. Project ALL named outputs uniformly (WBSV-01); no single output is special.
- **Customer identifiers in code/comments/tests/docs:** `ufh`, `quote`, `UFH`, `coil_band`, `Plot-3`, `7_Quote!C11`, `2_Constants!Margin`, `first_fix`, `heat_source`, `margin` all appear in the lift source. Scrub every one (D-01/D-13). The fixture uses tax-domain names.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Bundle integrity verification | A new boot hash check | `BundleLoader` over `build_bundle_lock` (runtime) | Must byte-reproduce the emitter's hashing or false-positive (`artifact_model.rs:107-134`); one shared verifier |
| `.xlsx` rendering | A second writer | runtime `render_xlsx(layout, run)` (`render/mod.rs:227`) | Writer-only, purity-gate-blessed; reader never enters served tree |
| Changelog model + serve | A diff computation | runtime `VersionChangelog` + serve `bundle.changelog` (`diff_version.rs`) | Recorded + hash-verified at boot, not a forgeable runtime diff |
| `isError` surfacing | `Err(pmcp::Error)` | `to_iserror_result` → `Value` in `structuredContent` via `with_ui` (`error.rs:171`) | pmcp `tool` dispatch hardcodes `is_error:false`; the only machine-actionable path is widget structuredContent (WBSV-06) |
| Directory embedding | Per-artifact `include_str!` consts | `include_dir!` (the whole tree) | Generic across any bundle; the lighthouse's 11 hardcoded consts are exactly the per-workbook coupling to kill |
| Manifest→JSON-Schema projection | Hand-written per-tool schema | `schema.rs::*_output_schema` + `input_schema_for_manifest` | Already manifest-driven (WBSV-07); the `result_envelope_schema` accepts both success + isError shapes |

**Key insight:** The lighthouse already paid the hard generalization cost — the served layer is manifest-driven, fail-closed, and the integrity hashing is shared with the emitter. The risk in this phase is **re-introducing** customer specifics during the lift, not building anything new.

## Scrub Deltas (D-13 — the mandated, enumerated changes from the lift source)

These are the *only* deliberate divergences from a verbatim lift. The planner should make each a discrete task with a grep-verifiable acceptance:

| # | Lift source site | Delta | WBSV |
|---|------------------|-------|------|
| S-1 | `mod.rs:39-49` `CellMap.supply_total_cell`; `handler.rs:58-77` `read_supply_total`; `handler.rs:134` `"supply_total":`; `schema.rs:96-98` `supply_total` field | **Remove the privileged headline.** `calculate` returns only the all-named-outputs `{value,unit}` map; drop `supply_total` and `supply_total_cell` everywhere. (The runtime `CellMap` in-repo still has `supply_total_cell` — `artifact_model.rs:46-48` — plan a runtime model edit OR leave the field optional/unused.) | WBSV-01/07 |
| S-2 | `mod.rs:148-170` `CoilBandAnnotation`/`GoldenCorpusEnvelope`; `handler.rs:242-256` keystone step | **Replace `coil_band` with manifest-declared `annotations`** (D-14). Generic `annotations` object keyed by manifest names; add an additive `Manifest` annotation field in the runtime. | WBSV-02 |
| S-3 | `mod.rs:174-193` `ProvStamp{workflow,version,workbook_hash}` | **Rename `workflow`→`bundle_id`** (D-15 "bundle_id + version + lock hash"). `BundleLock.workflow` stays the on-disk field name OR also renames — decide for consistency; the served stamp surfaces `bundle_id`. | WBSV-01/06 (stamp) |
| S-4 | every `ufh`/`quote`/`UFH`/`Plot-3`/`7_Quote!`/`first_fix`/`heat_source`/`margin`/`coil`/`reload_catalog` string in code, comments, tests, docs | **Strip every customer identifier.** Fixture + tests use tax-domain names (e.g. `gross_income`, `filing_status`, `tax_owed`, bracket tables). Grep-gate `ufh|quote|coil|towelrad|plot.?3` = 0 hits in `crates/pmcp-server-toolkit/src/workbook/` and the fixture. | D-01/D-13 |

The lighthouse `tools/quote.rs`, `rot.rs`, the `value-schema://` resource (`resource.rs`/`ValueSchemaResource`), `reload_catalog`/`get_catalog_info`, and the whole `quote_pricing_core` catalog layer are **NOT lifted** — they are the customer-specific tools deleted per D-13.

## Runtime State Inventory

> This is a code-extraction phase with a synthetic fixture; there is no live external runtime state to migrate. Inventory completed for completeness.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — the bundle is a static committed fixture; no DB/datastore. Verified by D-03 (generator + committed golden). | None |
| Live service config | None — no external service holds workbook state; the example boots from an embedded/local bundle. | None |
| OS-registered state | None — no scheduled tasks/services; the example is a `cargo run`. | None |
| Secrets/env vars | None new. The example may read `PMCP_ASSETS_DIR`/a `--bundle-dir` arg (mirrors the SQL example's asset resolver) — code-only, no secret. | None |
| Build artifacts | The synthetic golden fixture under `tests/fixtures/<bundle>@<version>/` is a build/test artifact; regeneration must be byte-identical (D-03 CI check). The runtime's `include_dir!` embeds it at compile time for `EmbeddedSource`. | Add the byte-stability CI check (D-03); ensure `EmbeddedSource` embeds the same committed bytes |

**Nothing found requiring data migration** — verified: the only "state" is the committed golden fixture, produced by an in-repo generator.

## Common Pitfalls

### Pitfall 1: Purity-boundary erosion via the toolkit `workbook` feature (THE phase risk)
**What goes wrong:** The toolkit `workbook` feature pulls `pmcp-workbook-runtime`; if the runtime (or a transitively-unified workspace feature) ever pulls `umya`/`quick-xml`/`swc_*`/`pmcp-code-mode`, the served tree is breached.
**Why it happens:** Cargo feature unification in a whole-workspace build; a "harmless" runtime feature that gates a reader; a new dep edge.
**How to avoid:** Extend the Phase-91 purity gate. The Makefile recipe is **explicitly designed to be extended** (`Makefile:492-497`): append the served crate to `PURITY_CRATES` if reader-free, give it a crate-local `deny.toml`. BUT the toolkit is NOT unconditionally reader-free (it has `code-mode`/`sql`/`http`). So add a **toolkit `--features workbook` cargo-tree assertion** (umya/quick-xml/swc_/pmcp-code-mode absent) as a new per-feature-combination check, distinct from the unconditional `PURITY_CRATES` loop. Carries forward Phase 91 D-09 per-feature-combination rule (D-10).
**Warning signs:** `cargo tree -p pmcp-server-toolkit --features workbook` output grows a reader crate after a refactor; a new runtime `[features]` entry gating a compile-time reader.

### Pitfall 2: Evidence hash-fold mismatch (false-positive integrity failure)
**What goes wrong:** The `BundleLoader` folds a different evidence-member set (or different `update_field` tags / sort order) than the generator emitted → boot integrity fails on a valid bundle.
**Why it happens:** The evidence hash is path+length-prefixed over a SORTED member list folding `cell_map.json`/`layout.json`/`changelog.json`/`parser_equivalence.json`/etc. (`mod.rs:331-383`). Generator and loader must fold the identical set with identical tags.
**How to avoid:** The fixture generator and the `BundleLoader` MUST share one `evidence_members()` definition (or a test asserting `recompute_evidence_hash() == lock.artifacts.evidence`, like `mod.rs:717-723`). The lighthouse's `IntegrityMismatch` diagnostic (FOUND vs EXPECTED member set, `mod.rs:262-276`) is the debugging aid — lift it.
**Warning signs:** A freshly-generated golden fails its own boot check.

### Pitfall 3: Numeric-enum fail-open (WR-02) and role-skip fail-open (WR-05)
**What goes wrong:** A skewed manifest (numeric `Dtype` + string `allowed_values`) lets a number bypass the enum gate; or a `cell_map` entry with no manifest role silently seeds the executor, bypassing dtype/enum gates.
**Why it happens:** The naive validation `if let Some(role) = ...` skips when absent.
**How to avoid:** Preserve the lighthouse fail-closed arms verbatim: `input.rs:114-123` (no manifest role → `invalid_input`) and `input.rs:241-259` (enum membership test is string-only and fails closed for a non-string value). These have full negative-path tests (`input.rs:592-611`, `:456-477`) — lift them.
**Warning signs:** A tampered/skewed-manifest test passes when it should reject.

### Pitfall 4: `workbook://` URI is an UNTRUSTED, attacker-controlled payload
**What goes wrong:** `resources/read` decodes a client-supplied URI; without a size guard + provenance verification + re-validation, it's a DoS / spoofing / injection lever.
**Why it happens:** The pointer round-trips through the client.
**How to avoid:** Lift `render_uri.rs` verbatim: size-guard FIRST (`MAX_ENCODED_URI_LEN`, `render_uri.rs:129-136`), total/panic-free decode, then in `render_resource.rs` VERIFY provenance against the bundle stamp (`render_resource.rs:64-86`) and RE-VALIDATE inputs through `validate_input` (`render_resource.rs:88-92`) before re-running. Publish the URI format as a documented contract (D-16) so the size bound + scheme become a versioned decision.
**Warning signs:** An oversized or cross-provenance URI renders instead of erroring.

## Code Examples

### Boot load + verify (the WBSV-08 core, source-driven)
```rust
// Source: lighthouse mod.rs:478-541 (load_bundle), refactored to take a BundleSource
pub fn load(source: &dyn BundleSource) -> Result<WorkbookBundle, BundleLoadError> {
    let lock: BundleLock = parse_member(source, "BUNDLE.lock")?;
    let evidence_hash = recompute_evidence_hash(source)?;          // path+len-prefixed, sorted
    let recomputed = build_bundle_lock(&lock.workflow, &lock.version,
        lock.workbook_hash.clone(), &ir_json, &manifest_json, &evidence_hash);
    if recomputed.artifacts != lock.artifacts || recomputed.combined != lock.combined {
        return Err(BundleLoadError::IntegrityMismatch { /* FOUND vs EXPECTED diagnostic */ });
    }
    let manifest: Manifest = parse_member(source, "manifest.json")?;
    let layout: LayoutDescriptor = parse_member(source, "layout.json")?;
    let changelog: VersionChangelog = parse_member(source, "evidence/changelog.json")?;
    verify_stamp_binding(&lock, &manifest, &layout, &changelog)?;   // WR-02 stamp cross-check
    let dag = build_dag(&ir);                                       // built ONCE at load
    Ok(WorkbookBundle { ir, dag, manifest, /* annotations, */ changelog, stamp, .. })
}
```

### isError envelope (WBSV-06 — lift verbatim, rename `workflow`→`bundle_id` in the stamp)
```rust
// Source: lighthouse error.rs:171-192
pub fn to_iserror_result(err: &WorkbookToolError, stamp: &ProvStamp) -> Value {
    let mut obj = Map::new();
    obj.insert("isError".into(), json!(true));
    obj.insert("code".into(), json!(err.code));
    obj.insert("reason".into(), json!(err.reason));
    obj.insert("provenance".into(), stamp.to_json());   // bundle_id + version + workbook_hash (S-3)
    if let Some(f) = &err.field    { obj.insert("field".into(), json!(f)); }
    if let Some(a) = &err.allowed  { obj.insert("allowed".into(), json!(a)); }
    if let Some((lo,hi)) = &err.range { obj.insert("range".into(), json!([lo,hi])); }
    if let Some(r) = &err.required { obj.insert("required".into(), json!(r)); }
    Value::Object(obj)
}
```

## State of the Art

| Old Approach (lighthouse) | Current Approach (Phase 92) | When Changed | Impact |
|--------------------------|-----------------------------|--------------|--------|
| 11 hardcoded `include_str!` bundle consts | `BundleSource` (local-dir + `include_dir!` embedded) | this phase | Generic across any bundle; one source = one bundle@version |
| `load_bundle()` inline in the served module | shared `BundleLoader` in the runtime | this phase | No source impl can bypass WBSV-08; toolkit re-exports it |
| `supply_total` privileged headline output | all-named-outputs uniform projection | this phase (§5 fix) | No single output is special (WBSV-01) |
| `coil_band` evidence read (customer-specific) | manifest-declared `annotations` | this phase (D-14) | Engine is domain-agnostic (WBSV-02) |
| `ProvStamp.workflow` | `ProvStamp.bundle_id` | this phase (D-15) | Stamp ties response to a bundle, not a customer workflow name |
| `DispatchingResource` (workbook:// + value-schema://) | single `RenderWorkbookResource` | this phase | The value-schema resource is customer-specific; not lifted |

**Deprecated/outdated:** The lighthouse `tools/` (quote/rot), `quote_pricing_core` catalog, `value-schema://` resource, `reload_catalog`/`get_catalog_info` tools — all customer-specific, NOT lifted.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `include_dir 0.7.4` is the legitimate embedding crate (author Michael-F-Bryan) | Standard Stack / Legitimacy | Low — widely used; gated behind human-verify checkpoint per protocol |
| A2 | A small additive `Manifest.annotations` field can carry D-14 annotations without breaking existing serde | Pattern 3 / S-2 | Medium — if the runtime model can't take an additive field cleanly, the annotation generalization needs a different carrier (e.g. reuse `GovernedDatum`-style); validate against `manifest_model.rs` during planning |
| A3 | Dropping `DispatchingResource` is safe because the toolkit `workbook` module has only one resource | Pattern 4 | Low — verified the dispatcher exists only to compose with `value-schema://` which is not lifted |
| A4 | The runtime `CellMap.supply_total_cell` field can be removed or left unused without breaking the executor | S-1 | Medium — `read_supply_total`/`project_outputs` reference it; the all-outputs path (`project_outputs`, `handler.rs:88-108`) already iterates `cell_map.outputs` independently, so removing the headline is mechanical |
| A5 | The toolkit `workbook` feature can be cargo-tree-asserted reader-free as a distinct check (not via `PURITY_CRATES`) | Pitfall 1 | Low — the Makefile recipe structure supports adding a separate per-feature assertion |

## Open Questions (RESOLVED)

1. **Where does the on-disk `BUNDLE.lock.workflow` field rename land (S-3)?**
   - What we know: the served stamp surfaces `bundle_id` (D-15). The runtime `BundleLock.workflow` is the on-disk artifact field (`artifact_model.rs:69`).
   - What's unclear: whether to rename the on-disk field too (cleaner, but a fixture/artifact contract change) or keep `workflow` on disk and map it to `bundle_id` only in the served `ProvStamp`.
   - Recommendation: keep the on-disk artifact field name as-is for Phase-93 producer/consumer agreement (the compiler emits it), and rename only at the served `ProvStamp` boundary. Confirm with the user — it touches the frozen contract.
   - **RESOLVED:** rename on-disk per D-17 — the SDK's frozen contract renames `BUNDLE.lock.workflow` → `bundle_id` end-to-end (fixture generator emits it, BundleLoader expects it, Phase-93 compiler emits it; no SDK consumer sees the lighthouse legacy naming). This SUPERSEDES the recommendation above. Plans 01 (artifact_model rename + loader) and 02 (golden BUNDLE.lock carries `bundle_id`) implement this.

2. **Does the synthetic manifest need a new annotation declaration type, or can D-14 reuse an existing field?**
   - What we know: `manifest_model.rs` has `CellRole.notes`, `GovernedDatum`, but no cell-annotation declaration.
   - Recommendation: add an additive `Manifest.annotations: Vec<AnnotationDecl>` (serde-default-empty). Validate the exact shape during planning against the tax-fixture's bracket-boundary annotation needs.
   - **RESOLVED:** additive `Vec<AnnotationDecl>` per D-18 — annotations land as an additive serde-default-empty `Manifest.annotations` field (name + target cell/output + meaning); old manifests deserialize unchanged. Plan 01 Task 3 implements the field + type; Plan 02 Task 1 populates bracket-boundary annotations in the tax fixture.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain (stable) | build/test | ✓ (CI uses dtolnay stable) | per `rustup` | — |
| `include_dir` (crates.io) | `EmbeddedSource` (WBSV-09) | ✗ (not yet added) | 0.7.4 target | `include_str!` per-artifact (loses genericity) |
| `pmcp/streamable-http` (via toolkit `http`) | D-12 example | ✓ (already forwarded) | — | none needed |
| `rust_xlsxwriter` (runtime, writer) | `render_workbook` | ✓ (Phase 91) | 0.95 | — |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** `include_dir` (fallback `include_str!`, but loses the generic embedding D-08 wants). Add `include_dir 0.7.4` behind the runtime `embedded` feature after the human-verify checkpoint.

## Validation Architecture

> Nyquist validation is ENABLED (`workflow.nyquist_validation` absent in config.json = enabled).

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + property tests via `proptest`/`quickcheck` (project ALWAYS-requirement) + doctests |
| Config file | none — workspace `cargo test`; CLAUDE.md mandates `--test-threads=1` in CI |
| Quick run command | `cargo test -p pmcp-server-toolkit --features workbook,http <module>::` |
| Full suite command | `make quality-gate` (fmt/clippy/build/test/audit/purity-check) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WBSV-01 | `calculate` returns ALL named outputs + provenance, no headline | unit + integration | `cargo test -p pmcp-server-toolkit --features workbook calculate` | ❌ Wave 0 |
| WBSV-02 | `explain` ordered trace + manifest-declared annotations | unit | `... explain` | ❌ Wave 0 |
| WBSV-03 | `get_manifest` projection | unit | `... get_manifest` | ❌ Wave 0 |
| WBSV-04 | `diff_version` serves recorded changelog | unit | `... diff_version` | ❌ Wave 0 (lift `diff_version.rs` tests) |
| WBSV-05 | `render_workbook` URI round-trip + stateless regen | unit + property | `... render_uri` (lift round-trip + determinism props) | ❌ Wave 0 |
| WBSV-06 | isError envelope shape for all codes | unit | `... error` (lift `all_six_..._codes` grep test) | ❌ Wave 0 |
| WBSV-07 | manifest→schema projection (additionalProperties:false, non-empty outputSchema) | unit | `... schema` | ❌ Wave 0 |
| WBSV-08 | boot recompute fails closed on tamper (byte-flip / delete / version desync) | integration (tempdir tamper) | `... bundle_loader::tamper` (D-05 corrupt-the-golden) | ❌ Wave 0 |
| WBSV-09 | `BundleSource` local-dir + embedded both load the golden | integration | `... bundle_source` | ❌ Wave 0 |
| (all) | byte-stable golden regeneration | CI check | generator re-emit + `diff`/hash equality | ❌ Wave 0 (D-03) |
| (all) | streamable-HTTP example boots + serves 5 tools | example | `cargo run --example workbook_server_http --features workbook,http` | ❌ Wave 0 (D-12) |
| (purity) | toolkit `--features workbook` is reader-free | CI gate | `make purity-check` (extended) | ⚠️ extend existing |

### Sampling Rate
- **Per task commit:** `cargo test -p pmcp-server-toolkit --features workbook,http <module>::`
- **Per wave merge:** `cargo test -p pmcp-workbook-runtime && cargo test -p pmcp-server-toolkit --features workbook,http`
- **Phase gate:** `make quality-gate` (includes `purity-check`) green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/pmcp-server-toolkit/tests/fixtures/<bundle>@<version>/` — the synthetic tax-calc golden (D-03)
- [ ] Fixture generator (test-support Rust, Serialize-types → 7 artifacts, byte-stable)
- [ ] `crates/pmcp-server-toolkit/tests/workbook_*.rs` — integration tests (tamper/source/example)
- [ ] Extend `Makefile` `purity-check` with a toolkit `--features workbook` cargo-tree assertion
- [ ] Property tests (ALWAYS-requirement): URI codec round-trip/determinism; validation invariants
- [ ] `cargo run --example workbook_server_http` (ALWAYS-requirement: working example)

## Security Domain

> `security_enforcement` not set to `false` in config → enabled.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | served bundle is curated config; no per-tool auth in this phase (Shape A/B layer auth later) |
| V3 Session Management | no | stateless tools; `resources/read` recomputes from URI (no server-side session) |
| V4 Access Control | partial | strict-constant override rejection (`strict_constant_override`, `input.rs:133-138`) — BA-governed constants cannot be overridden per-call |
| V5 Input Validation | **yes** | `deny_unknown_fields` DTO + manifest tier/dtype/enum gates, fail-closed (`input.rs`); `additionalProperties:false` schemas (`schema.rs`) |
| V6 Cryptography | **yes** | SHA-256 combined hash-of-hashes via `sha2` (`build_bundle_lock`); never hand-rolled |
| V12 Files/Resources | **yes** | `workbook://` URI size guard + total/panic-free decode + provenance verification (`render_uri.rs`/`render_resource.rs`) |

### Known Threat Patterns for the served workbook layer

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Tampered bundle artifact (single-file swap) | Tampering | Boot recompute of per-artifact + combined hash, fail closed (WBSV-08, `mod.rs:478-501`) |
| Tampered `BUNDLE.lock` provenance triple | Spoofing/Tampering | `verify_stamp_binding` cross-checks lock↔hash-covered members (`mod.rs:441-476`) |
| Spoofed `workbook://` URI (cross-provenance) | Spoofing | `render_resource` verifies decoded provenance == bundle stamp before rendering (`render_resource.rs:64-86`) |
| Oversized `workbook://` URI (DoS) | DoS | `MAX_ENCODED_URI_LEN` size guard checked FIRST (`render_uri.rs:129-136`) |
| Untrusted URI inputs injection | Tampering | RE-VALIDATE through `validate_input` on `resources/read` (`render_resource.rs:88-92`) |
| Numeric-enum / role-skip fail-open | Tampering | string-only enum gate + no-manifest-role rejection, fail closed (WR-02/WR-05, `input.rs`) |
| NaN/Infinity output → JSON null masquerading as success | Tampering | finiteness check on every numeric output (WR-06, `handler.rs:58-108`) |

## Project Constraints (from CLAUDE.md)

- **Zero clippy warnings** (pedantic + nursery via `make lint`); `make quality-gate` before any commit/PR.
- **Cognitive complexity ≤25 per function** (PMAT CI gate); the lift's `load_bundle`/`validate_input` may need the lighthouse's existing helper decomposition preserved.
- **Zero SATD comments.**
- **ALWAYS requirements for new features:** fuzz + property + unit tests + a working `cargo run --example`. The URI codec and validation path are natural fuzz/property targets (the lighthouse already has property-style round-trip tests).
- **Crate-level `#![deny(clippy::unwrap_used, expect_used, panic)]`** on value paths (kept from Phase 91; the lighthouse served code already honors it with `cfg(test)` allow).
- **Contract-first** (provable-contracts) for new features/fixes.
- **Publish order:** runtime (2a) → dialect (2b) → toolkit (5) already accommodates the new `pmcp-server-toolkit →(feature workbook)→ pmcp-workbook-runtime` edge.

## Sources

### Primary (HIGH confidence)
- `crates/pmcp-workbook-runtime/src/{lib.rs,artifact_model.rs,manifest_model.rs,render/layout.rs,changelog.rs}` — in-repo runtime API + existing model
- `crates/pmcp-server-toolkit/src/{lib.rs,builder_ext.rs,sql/mod.rs}` + `Cargo.toml` — toolkit feature-gate + extension-method pattern + `http` example
- `crates/pmcp-server-toolkit/examples/sql_server_http.rs` + Cargo `[[example]]` — streamable-HTTP `required-features` precedent (D-12)
- `Makefile:460-545` + `deny.toml` — purity gate (extensible by design)
- Lighthouse `quote-pricing-server/src/workbook/{mod,handler,input,schema,error,diff_version,render_uri,render_resource}.rs` + `src/{state,lib}.rs` + `bundles/ufh-quote@1.0.0/` shape — the lift source (read in full)
- `.planning/REQUIREMENTS.md` (WBSV-01..09 verbatim), `.planning/ROADMAP.md` (Phase 92 entry), `.planning/phases/91-.../91-CONTEXT.md` (carried-forward decisions)

### Secondary (MEDIUM confidence)
- `.planning/research/{PITFALLS,ARCHITECTURE}.md` — served-cone + purity-erosion guidance
- `.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md` — validated toolkit-lift patterns (the SQL/OpenAPI lift this mirrors)
- `cargo search include_dir|base64|url` — version verification

### Tertiary (LOW confidence)
- `include_dir` author/legitimacy (`Michael-F-Bryan`) — `[ASSUMED]`, gate behind human-verify checkpoint

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — compute substrate exists in-repo; only `include_dir` is new (gated)
- Architecture: HIGH — the lift source + toolkit patterns read directly; scrub deltas enumerated against exact line numbers
- Pitfalls: HIGH — each is grounded in a lighthouse line number or the Phase-91 purity recipe
- BundleSource design: HIGH — validated against lighthouse `state.rs`/`lib.rs`; recommendation is the byte-accessor + shared loader

**Research date:** 2026-06-10
**Valid until:** 2026-07-10 (stable — in-repo + private-reference sources; `include_dir`/`base64` versions stable)
