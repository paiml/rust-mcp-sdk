# Phase 92: BundleSource + Served-Tool Toolkit Module - Context

**Gathered:** 2026-06-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Freeze the compiled-bundle contract from the consumer side: a `BundleSource`
trait (local-dir + embedded `include_dir!` impls; S3/registry is a documented
extension seam only) plus a generic, **fully manifest-driven** `workbook`
feature module in `pmcp-server-toolkit` that registers all five tools
(`calculate` / `explain` / `get_manifest` / `diff_version` / `render_workbook`)
against a synthetic test bundle, fails closed on every integrity or validation
gap (boot `BUNDLE.lock` hash recomputation, WR-02 / WR-05 hardening), and emits
the same `TypedToolWithOutput` → `outputSchema` → `structuredContent`
discipline as the SQL/OpenAPI toolkits — with **zero per-workbook Rust**.

**Requirements:** WBSV-01 … WBSV-09.

**Explicitly NOT in this phase:** the compiler / umya ingest / linter execution
(Phase 93), CLI subcommands + `pmcp.toml` (Phase 94), Shape A binary (Phase 95),
Shape B scaffold + dialect-version declaration + second-workbook gate
(Phase 96). Does not touch `pmcp-code-mode`.
</domain>

<decisions>
## Implementation Decisions

### Test bundle (chicken-and-egg with the Phase 93 compiler)
- **D-01:** The test bundle is a **hand-authored synthetic fixture** — NOT a
  copy of the lighthouse `ufh-quote@1.0.0` bundle. **Hard constraint from the
  user: no TowelRads customer data or business logic may appear anywhere in
  this repo** (fixtures, code, comments, identifiers, docs).
- **D-02:** Fixture domain (user's choice): a **realistic, common Excel use
  case with no sensitive data — e.g. tax calculation with bracket rules — a
  couple of steps from an input sheet to an output sheet.** This synthetic
  workbook bundle is the golden contract every WBSV test runs against.
- **D-03:** Production mechanism: **generator + committed golden.** A
  test-support Rust generator builds the bundle through `pmcp-workbook-runtime`
  Serialize types and writes the seven artifacts to
  `crates/.../tests/fixtures/<bundle>@<version>/`. The committed files are the
  golden contract; regeneration must be byte-identical (CI check). Phase 93
  later re-emits the same workbook through the real compiler and diffs against
  this golden to prove producer/consumer agreement.
- **D-04:** Coverage: **full-surface fixture** — multiple named outputs (no
  privileged headline), enum + numeric + string inputs across tiers, units,
  governed data, a v1.0.0→v1.1.0 changelog pair (so `diff_version` is real),
  and a layout descriptor (so `render_workbook` is real). One bundle exercises
  all five tools.
- **D-05:** Negative paths (WBSV-06/08): tests **copy the golden to a tempdir
  and corrupt it programmatically** (flip a byte, delete an artifact, desync
  `BUNDLE.lock` version). No committed corrupt fixtures — one golden only;
  each tamper lives next to the test that asserts it.

### BundleSource trait
- **D-06:** The trait + local-dir + embedded impls live in
  **`pmcp-workbook-runtime`** (beside the artifact model and `BUNDLE.lock`
  hashing it returns/verifies). The `include_dir` dependency for the embedded
  impl goes **behind a feature flag** to keep the leaf lean. Purity gate keeps
  covering the crate (reader-free).
- **D-07:** **Sync trait** — bundles load once at boot; no tokio/async_trait in
  the runtime. The documented S3/registry seam states implementors fetch/cache
  ahead of boot (or `block_on`).
- **D-08:** Addressing: **one source instance = one `bundle@version`** (e.g.
  `LocalDirSource::new("bundles/tax-calc@1.0.0")`; embedded = the one baked-in
  bundle). Version pinning happens at construction. Multi-workbook mapping is
  Phase 94's `pmcp.toml` concern.

### Toolkit integration
- **D-09:** Registration API: a **`builder_ext` extension method** (e.g.
  `.with_workbook_bundle(source)`) mirroring the SQL/OpenAPI precedent — loads
  + verifies the bundle, registers all five tools and the `workbook://`
  resource. Config-file (ServerConfig) wiring waits for Shape A (Phase 95).
- **D-10:** Feature flag: **`workbook`, NOT in default features** (mirrors
  `http`). Gates the module + the `pmcp-workbook-runtime` dep. The purity-gate
  feature matrix gains the toolkit `workbook` combination (carries forward
  Phase 91 D-09 per-feature-combination rule).
- **D-11:** The toolkit module **re-exports the boot surface**
  (`BundleSource`, both impls, loader/error types) so Shape A/B consumers
  depend only on `pmcp-server-toolkit` and never name the runtime crate in
  their Cargo.toml.
- **D-12 (user, explicit):** The mandatory runnable example serves over
  **streamable-HTTP, NOT stdio** — the target audience is business people
  consuming a remotely-hosted workbook server, not installing binaries
  locally. One example, all five tools, against the synthetic tax-calc fixture.
  *Refinement (2026-06-10, orchestrator, within delegated discretion):* after
  the D-06 checker fix split the toolkit feature into `workbook` (LocalDirSource
  only) + `workbook-embedded` (adds `include_dir`), the example — which defaults
  to the embedded source per the bundle-source discretion below — wires
  `required-features = ["workbook-embedded", "http"]`. The user's decision
  (streamable-HTTP transport, remote business-user story) is unchanged; the
  original `["workbook", "http"]` wording predated the feature split.

### Served layer (lift posture)
- **D-13:** **Reuse the lighthouse's generic engine code, scrubbed.** Port the
  served-layer machinery (input validation, manifest-driven schema projection,
  error envelopes, trace assembly, render plumbing from
  `quote-pricing-server/src/workbook/`) — it is validated by ~730 lighthouse
  tests — with the mandated deltas: delete quote-specific tools
  (`tools/quote.rs`, `rot.rs`, etc.), kill `build_reference_manifest` from all
  non-test paths, WR-02 (enum string-only) / WR-05 (fail-closed role check)
  hardening, no privileged headline output. **Zero customer-named identifiers
  survive in SDK code, comments, fixtures, or docs.** The customer's business
  logic lives in their bundle, which is not copied (D-01).
- **D-14:** `coil_band` generalization (WBSV-02): **manifest-declared
  annotations** — the manifest declares which cells/outputs carry named
  annotations; the `explain` trace emits a generic `annotations` object keyed
  by those manifest-declared names. The engine knows nothing domain-specific;
  the tax fixture demonstrates it (e.g. bracket-boundary annotations).
- **D-15:** Provenance stamp (every tool response + error envelope):
  **bundle_id + bundle version + `BUNDLE.lock` combined hash** — the minimal
  set tying any response to one exact verified bundle, matching what the
  WBSV-08 boot gate computes. Dialect-version field waits for Phase 96.
- **D-16:** The **`workbook://` URI format is a documented public SDK
  contract** — lift the lighthouse `render_uri` scheme (inputs + provenance
  encoded, stateless regeneration on `resources/read`) and publish its format
  in the SDK docs alongside the dialect spec. Format changes become versioned
  decisions, not silent edits.

### Post-research contract decisions (user, 2026-06-10)
- **D-17:** The SDK's frozen on-disk bundle contract **renames the lighthouse
  `BUNDLE.lock` `workflow` field to `bundle_id`** — the fixture generator
  emits `bundle_id`, the BundleLoader expects `bundle_id`, and Phase 93's
  compiler will emit `bundle_id`. No SDK consumer ever sees the lighthouse's
  legacy naming; lighthouse migration is the lighthouse's own concern.
- **D-18:** D-14's manifest annotations land as an **additive
  `Vec<AnnotationDecl>` field on `Manifest`, serde-default empty** (name +
  target cell/output + meaning). Old manifests without it deserialize fine;
  the tax fixture declares bracket-boundary annotations to prove the path.

### Claude's Discretion
- **Trait surface granularity (from D-06/D-07 discussion):** user said "you
  decide" on raw-bytes accessor vs parsed-aggregate return. Leaning: **dumb
  byte accessor (`read_artifact(name)` / `list_artifacts()`) + a single shared
  `BundleLoader`** that does parse + `BUNDLE.lock` recomputation + fail-closed
  checks identically for every source, so no impl can bypass WBSV-08.
  Researcher validates against the lighthouse loading code
  (`quote-pricing-server/src/state.rs` + `lib.rs`).
- Bundle-source choice **within the example** (embedded default vs
  `--bundle-dir` override) — pick what reads best for a remote-deploy story.
- Exact synthetic tax-workbook content (bracket counts, enum inputs like
  filing status, governed rate table shape), fixture/bundle naming, and the
  byte-stability CI check mechanics.
- Module file layout inside `pmcp-server-toolkit/src/workbook/`, error-code
  taxonomy naming, `include_dir` feature name in the runtime crate.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

> ⚠ The lighthouse lives at
> `/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/`
> (absolute path; not resolvable as a repo-relative path). It is **private
> reference material** — lift generic engine code only, per D-13's scrub rule.

### Phase contract
- `.planning/ROADMAP.md` — Phase 92 entry (goal, 5 success criteria, WBSV mapping)
- `.planning/REQUIREMENTS.md` — WBSV-01 … WBSV-09 verbatim
- `.planning/phases/91-workbook-runtime-purity-gate-dialect-spec/91-CONTEXT.md` — Phase 91 decisions carried forward (crate split, purity gate D-09, finding model D-03)

### v2.3 research (in-repo)
- `.planning/research/SUMMARY.md` — phase-cut synthesis
- `.planning/research/ARCHITECTURE.md` — two dependency cones; served cone = served-binary → toolkit[workbook] → runtime → pmcp
- `.planning/research/PITFALLS.md` — purity-boundary erosion + served-layer pitfalls

### Lighthouse served layer (lift source — scrub per D-13)
- `/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/crates/quote-pricing-server/src/workbook/` — generic engine modules: `handler.rs`, `input.rs`, `schema.rs`, `error.rs`, `diff_version.rs`, `render_resource.rs`, `render_uri.rs`
- `/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/crates/quote-pricing-server/src/state.rs` + `lib.rs` — current bundle loading/boot verification (informs BundleSource + BundleLoader design)
- `/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/crates/quote-pricing-server/bundles/ufh-quote@1.0.0/` — the seven-member bundle **shape** reference (artifact names, BUNDLE.lock structure). **Do NOT copy contents** (D-01)
- `/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/docs/sdk-issue-excel-workbook-compiler-extraction.md` §5 — served-side generalization gaps (coil_band, headline output, WR-02/WR-05)
- `/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/docs/Excel-as-Configuration-Architecture-Brief.md` — two-surface model

### SDK patterns to mirror
- `crates/pmcp-server-toolkit/src/builder_ext.rs` — extension-method registration precedent (D-09)
- `crates/pmcp-server-toolkit/src/sql/` + `src/http/` — feature-gated module precedent + `TypedToolWithOutput` discipline (D-10)
- `crates/pmcp-workbook-runtime/src/` — runtime types the module consumes (`manifest_model.rs`, `artifact_model.rs` CellMap/BundleLock + hashing, `changelog.rs`, `scalar_eval.rs`, `render/`)
- `docs/workbook-dialect-spec.md` — the published-contract precedent the `workbook://` URI doc follows (D-16)
- `Makefile`/`justfile` `purity-check` + `.github/workflows` purity job — gate to extend with the toolkit `workbook` feature combo (D-10)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `pmcp-workbook-runtime` (Phase 91): all model/IR types, deterministic
  executor + per-cell traces, writer-only renderer, `BundleLock` integrity
  hashing — the module's entire compute substrate already exists in-repo.
- `pmcp-server-toolkit` `builder_ext.rs` (16.8K) — the SQL extension-method
  pattern to clone for `.with_workbook_bundle(...)`.
- Toolkit `http` feature — already forwards `pmcp/streamable-http`, so the
  D-12 example needs no new transport plumbing.
- Toolkit `resources.rs` — existing resource-registration machinery for the
  `workbook://` resource.

### Established Patterns
- Feature-gated toolkit modules (`#[cfg(feature = "http")] pub mod http;`) —
  the `workbook` module follows identically.
- `TypedToolWithOutput` + mandatory non-empty `outputSchema` →
  `structuredContent` — locked v2.2 discipline (WBSV-07 parity requirement).
- Phase 91 purity gate runs per feature-combination — adding the toolkit
  `workbook` combo is an extension, not a new mechanism.
- Crate-level `#![deny(clippy::unwrap_used, expect_used, panic)]` on value
  paths (lighthouse convention, kept in Phase 91).

### Integration Points
- New dep edge: `pmcp-server-toolkit` →(feature `workbook`)→
  `pmcp-workbook-runtime`. Publish order: runtime (2a) → dialect (2b) →
  toolkit (5) already accommodates this.
- Served cone for the purity gate: served-binary → toolkit[workbook] →
  runtime → pmcp (reader-free end to end).
</code_context>

<specifics>
## Specific Ideas

- **"Tax calculation with brackets"** is the user's example of the fixture
  domain — any realistic common Excel use case qualifies, but it must be a
  couple of steps from input sheet to output sheet and contain zero customer
  material. Bracket rules conveniently exercise enums (filing status), tiers,
  governed tables (rate table), and annotation-worthy boundaries.
- The audience framing for the example (D-12): business people calling a
  **remotely hosted** workbook server — the example should read like the
  deploy story, not a developer-local toy.
</specifics>

<deferred>
## Deferred Ideas

- **S3/registry BundleSource impls** — documented extension seam only
  (WBSV-09, roadmap-locked); async consideration noted under D-07 if/when a
  remote source ships.
- **Store-style BundleSource API** (`resolve(bundle_id, version)`,
  `list_versions`) — rejected for Phase 92 (D-08); revisit with Phase 94
  `pmcp.toml` multi-workbook mapping if construction-time pinning proves
  insufficient.
- **Config-file (ServerConfig) workbook section** — Phase 95 Shape A (D-09).
- **Dialect version in the provenance stamp** — Phase 96 (WBDL-02) per D-15.
- **Extended provenance stamp** (runtime crate version, compile timestamp) —
  rejected for v1; revisit if auditors ask.

None of the above is lost — each is roadmapped or explicitly parked.
</deferred>

---

*Phase: 92-bundlesource-served-tool-toolkit-module*
*Context gathered: 2026-06-10*
