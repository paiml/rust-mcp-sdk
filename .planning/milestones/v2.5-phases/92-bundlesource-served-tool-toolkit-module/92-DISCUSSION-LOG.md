# Phase 92: BundleSource + Served-Tool Toolkit Module - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-10
**Phase:** 92-bundlesource-served-tool-toolkit-module
**Areas discussed:** Test bundle chicken-and-egg, BundleSource trait design, Toolkit integration shape, Served-layer lift vs redesign

---

## Test Bundle Chicken-and-Egg

| Option | Description | Selected |
|--------|-------------|----------|
| Copy lighthouse bundle | Vendor `ufh-quote@1.0.0` as fixture — real producer output, but carries TowelRads pricing data into a public repo | |
| Hand-author synthetic fixture | Small neutral bundle built against runtime types; no business data | ✓ |
| Both: synthetic + gated parity | Synthetic committed + env-gated lighthouse parity test | |

**User's choice:** Hand-author synthetic fixture
**Notes:** Later in the discussion the user made the constraint explicit: no TowelRads customer data or business logic may be revealed anywhere; fixture domain = a realistic common Excel use case such as tax calculation with bracket rules, a couple of steps from input sheet to output sheet.

| Option | Description | Selected |
|--------|-------------|----------|
| Generator + committed golden | Test-support Rust generator (runtime Serialize types) writes seven artifacts to tests/fixtures; regeneration byte-identical (CI check); Phase 93 re-emits via compiler and diffs | ✓ |
| Static hand-written JSON files | Commit artifacts directly; BUNDLE.lock hashes hand-maintained | |
| In-memory only | Built per-run; no durable golden for Phase 93 | |

**User's choice:** Generator + committed golden (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Full-surface fixture | One domain exercising all five tools: multiple named outputs, enum/numeric/string tiers, units, governed data, v1.0.0→1.1.0 changelog pair, render layout | ✓ |
| Minimal core + per-tool micro-fixtures | Small main bundle + tiny per-tool fixtures | |
| Start minimal, grow as tools land | Extend plan-by-plan; golden re-hashes repeatedly | |

**User's choice:** Full-surface fixture (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Mutate copies at test time | Copy golden to tempdir, corrupt programmatically; no committed corrupt files | ✓ |
| Commit corrupted fixture variants | Generator emits tampered variants on disk | |
| You decide | Planner/researcher choice | |

**User's choice:** Mutate copies at test time (recommended)

---

## BundleSource Trait Design

| Option | Description | Selected |
|--------|-------------|----------|
| pmcp-workbook-runtime | Contract beside artifact model + lock hashing; lightest dep for implementors; include_dir behind feature flag | ✓ |
| Toolkit workbook module | Trait lives with consumer; S3 implementors need toolkit dep | |
| New crate pmcp-workbook-bundle | Dedicated contract crate; 3rd workbook crate for ~one trait | |

**User's choice:** pmcp-workbook-runtime (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Raw bytes + shared loader | Dumb byte accessor + one shared BundleLoader doing parse + hash verification (WBSV-08 structural) | |
| Parsed CompiledBundle per source | Each impl returns verified aggregate; verification depends on implementor discipline | |
| You decide | Researcher decides against lighthouse loading code | ✓ |

**User's choice:** You decide — recorded as Claude's discretion with stated leaning toward raw bytes + shared loader

| Option | Description | Selected |
|--------|-------------|----------|
| Sync | Boot-time load; no tokio/async_trait in runtime; S3 seam documented as fetch-ahead | ✓ |
| Async (async_trait) | SDK-wide convention; first-class remote seam; async plumbing cost | |
| You decide | Researcher checks boot paths | |

**User's choice:** Sync (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| One source = one bundle@version | Source instance points at exactly one compiled bundle; version pinned at construction | ✓ |
| Store API (resolve id + version) | Bundle-store model; speculative for Phase 92 | |
| You decide | Researcher checks lighthouse + Phase 94 needs | |

**User's choice:** One source = one bundle@version (recommended)

---

## Toolkit Integration Shape

| Option | Description | Selected |
|--------|-------------|----------|
| builder_ext method | `.with_workbook_bundle(source)` mirroring SQL/OpenAPI; config wiring waits for Shape A (95) | ✓ |
| Config-driven from day one | [workbook] ServerConfig section now | |
| Both ext method + config | Maximum coverage, broader scope | |

**User's choice:** builder_ext method (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| `workbook`, not in default | Mirrors `http`; opt-in module + runtime dep; purity matrix gains the combo | ✓ |
| `workbook` in default | Every toolkit consumer compiles workbook stack | |
| You decide | Researcher checks consumers/forwarding | |

**User's choice:** `workbook`, not in default (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| One example, both sources (stdio) | Embedded default + --bundle-dir flag, stdio | |
| Minimal embedded-only example | Smallest demo, stdio | |
| Two examples (embedded + local-dir/http) | Separate per source | |

**User's choice (Other, freeform):** One example with **streamable-HTTP as transport (not stdio)** — "our target is business people who will use it remotely and not install it locally on their machines." Bundle-source choice within the example left to Claude's discretion.

| Option | Description | Selected |
|--------|-------------|----------|
| Re-export the boot surface | Toolkit re-exports BundleSource/impls/loader so Shape A/B dep only on toolkit | ✓ |
| No re-exports | Consumers dep on runtime directly; two deps to sync | |
| You decide | Researcher checks Shape A needs | |

**User's choice:** Re-export the boot surface (recommended)

---

## Served-Layer Lift vs Redesign

| Option | Description | Selected |
|--------|-------------|----------|
| Near-verbatim + named deltas | Port handler/input/schema/error/render with explicit delta list | |
| Redesign surface, port algorithms | Restructure to toolkit conventions first | |
| You decide | Researcher judges module-by-module | |

**User's choice (Other, freeform):** Raised the data-sensitivity constraint — the lighthouse example is a specific customer whose data/business logic must not be revealed; build a completely new example (tax calculation with brackets or similar common Excel use case). A clarifying exchange separated engine *code* (generic, no customer data) from bundle *data* (customer-specific, not copied); user may not have initially registered the code/data split — clarified that the bundle, not the server code, is the conversion of the spreadsheet.

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse engine code, scrubbed | Port generic machinery (~730 lighthouse tests) with mandated deltas; zero customer-named identifiers | ✓ |
| Rewrite engine from scratch | Lighthouse as private reference only; longer phase | |
| You decide | Module-by-module judgment | |

**User's choice:** Reuse engine code, scrubbed (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Manifest-declared annotations | Manifest declares annotated cells/outputs; explain emits generic annotations object | ✓ |
| Free-form reconciliation notes only | Runtime-populated notes; authors can't surface domain annotations | |
| You decide | Researcher designs from trace code | |

**User's choice:** Manifest-declared annotations (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Bundle identity + lock hash | bundle_id + version + BUNDLE.lock combined hash | ✓ |
| Extended stamp | + dialect version, crate version, timestamp | |
| You decide | Match lighthouse stamp, trim/extend | |

**User's choice:** Bundle identity + lock hash (recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Documented contract | Publish workbook:// URI format in SDK docs alongside dialect spec | ✓ |
| Internal detail, lift as-is | Opaque URI; format free to change | |
| You decide | Researcher reads render_uri.rs | |

**User's choice:** Documented contract (recommended)

---

## Claude's Discretion

- BundleSource trait surface granularity (raw-bytes accessor + shared BundleLoader vs parsed aggregate) — leaning raw bytes + shared loader; validate against lighthouse `state.rs`/`lib.rs`
- Bundle-source choice within the streamable-HTTP example (embedded default vs `--bundle-dir`)
- Synthetic tax-workbook content details, fixture naming, byte-stability CI check mechanics
- Module file layout, error-code taxonomy naming, `include_dir` feature name

## Deferred Ideas

- S3/registry BundleSource impls — documented extension seam (roadmap-locked)
- Store-style BundleSource API — revisit with Phase 94 `pmcp.toml`
- Config-file workbook section — Phase 95 Shape A
- Dialect version in provenance stamp — Phase 96 (WBDL-02)
- Extended provenance stamp fields — revisit if auditors ask
