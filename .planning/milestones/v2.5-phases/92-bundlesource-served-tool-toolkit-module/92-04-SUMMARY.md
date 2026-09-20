---
phase: 92-bundlesource-served-tool-toolkit-module
plan: 04
subsystem: toolkit
tags: [workbook, render-workbook, workbook-uri, stateless-regen, resource-handler, dos-guard, spoofing-guard, provenance, proptest, published-contract]

# Dependency graph
requires:
  - phase: 92-bundlesource-served-tool-toolkit-module
    plan: 01
    provides: "pmcp_workbook_runtime — render::render_xlsx(layout, run), run_executor/build_dag, WorkbookBundle { layout, manifest, cell_map, stamp:BundleLock }"
  - phase: 92-bundlesource-served-tool-toolkit-module
    plan: 03
    provides: "workbook/{error,schema,input,handler}.rs — ProvStamp (combined_hash), validate_input (fail-closed), to_iserror_result, shared run_bundle/with_provenance/render_at_boundary helpers, result_envelope_schema"
provides:
  - "workbook/render_uri.rs — workbook:// URI codec: size-guard-first (MAX_ENCODED_URI_LEN=64KiB) total panic-free decode + deterministic encode over { dto, provenance }; WORKBOOK_XLSX_MIME"
  - "workbook/handler.rs — RenderWorkbookHandler (5th ToolHandler): validate -> encode -> return workbook:// POINTER (not bytes) + ProvStamp; render_workbook_output_schema in schema.rs"
  - "workbook/render_resource.rs — RenderWorkbookResource (ResourceHandler): stateless regen-on-read (decode -> verify provenance -> re-validate -> re-run -> render_xlsx -> base64 STANDARD); single resource (no DispatchingResource, A3)"
  - "docs/workbook-uri-spec.md — the published, versioned workbook:// URI public contract (D-16) with privacy warning + render-cost note"
affects: [92-05-builder-ext, workbook-server-scaffold, phase-93-compiler]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "render_workbook returns a provenance-bound POINTER, never the bytes: the .xlsx is recomputed per resources/read from the decoded URI alone (stateless, Lambda-safe, V3 — no session, no render cache)"
    - "The workbook:// URI is treated as an attacker-controlled payload on read: size-guard checked FIRST (T-92-14, before any base64), then total panic-free decode (T-92-17), then provenance == bundle stamp (T-92-15 spoofing guard, BEFORE rendering), then RE-validate decoded inputs (T-92-16 injection guard) — only then re-run + render"
    - "encode is deterministic + render_xlsx pins doc properties => reading the SAME URI twice is byte-identical (stateless determinism, proven by test)"
    - "URI payload uses base64url (URL_SAFE_NO_PAD) for a clean path segment; the rendered xlsx bytes on resources/read use STANDARD base64 (Codex MEDIUM #6 — base64 is a TOOLKIT dep under the workbook feature, not the runtime)"
    - "render_resource read decomposed into regenerate() (Result) + RegenError -> into_protocol() so each fn stays under cognitive complexity 25 and the protocol-error mapping happens once at the boundary"
    - "codec proptest pair: prop_encode_decode_identity (round-trip + determinism over arbitrary input maps) + prop_decode_total (decode TOTAL — Ok|Err, never panic — over arbitrary/adversarial/oversized strings); the CLAUDE.md ALWAYS-fuzz requirement via proptest"

key-files:
  created:
    - crates/pmcp-server-toolkit/src/workbook/render_uri.rs
    - crates/pmcp-server-toolkit/src/workbook/render_resource.rs
    - docs/workbook-uri-spec.md
  modified:
    - crates/pmcp-server-toolkit/src/workbook/handler.rs
    - crates/pmcp-server-toolkit/src/workbook/schema.rs
    - crates/pmcp-server-toolkit/src/workbook/mod.rs

key-decisions:
  - "WORKBOOK_XLSX_MIME lives in render_uri.rs (not render_resource.rs): Task 1's render_workbook handler advertises it, so it lands with the codec so Task 1 is self-contained — both the tool (mime_type field) and the resource read (Content MIME) reference the one constant"
  - "The decoded provenance triple is its own private ProvenanceWire serde struct mirroring ProvStamp (combined_hash, never the source-workbook hash), with From conversions both ways — keeps the on-wire JSON shape decoupled from the public ProvStamp type while preserving the Codex HIGH #3 field-naming contract"
  - "A workbook:// READ failure (bad/oversized/cross-provenance/invalid URI) is a protocol Error (Error::protocol INVALID_PARAMS / INTERNAL_ERROR), NOT the isError:true tool envelope: resources/read has no structuredContent channel, and a bad resource URI is an infrastructure fault (the client handed us a bad pointer), distinct from a tool DOMAIN failure"
  - "The resources/list entry is the scheme ROOT workbook://render/ (no payload) — a stable listable handle; the concrete payload-bearing URIs are minted per render_workbook call (A3: exactly one resource, no DispatchingResource wrapper)"

patterns-established:
  - "Pointer-not-bytes + stateless regen-on-read: the served tool returns a self-contained provenance-bound URI; the resource handler recomputes the artifact from the URI on every read with the full size-guard/provenance/re-validate hardening pipeline — no server-side render state to leak (T-92-18 accept)"
  - "A published, versioned URI contract doc (docs/workbook-uri-spec.md) sits beside the dialect spec: a public SDK surface (the URI clients store/replay) is documented with its security properties, privacy warning, render-cost note, and a versioning-decision clause"

requirements-completed: [WBSV-05]

# Metrics
duration: ~10min
completed: 2026-06-10
---

# Phase 92 Plan 04: render_workbook + the workbook:// resource Summary

**The fifth served tool and its resource land as the pointer-not-bytes render surface: `render_workbook` validates inputs then returns a provenance-bound `workbook://` URI (the bytes are NOT in the response), and a stateless `RenderWorkbookResource` regenerates the `.xlsx` per `resources/read` by decoding the URI — size-guard-FIRST (T-92-14), total panic-free decode (T-92-17), provenance-verified against the bundle stamp BEFORE rendering (T-92-15 spoofing guard), inputs RE-validated through the same fail-closed `validate_input` (T-92-16 injection guard), then re-run + `render_xlsx` + base64 — with the byte-identical-across-reads determinism, the single-resource (no `DispatchingResource`, A3) shape, and the published, versioned `workbook://` URI contract (`docs/workbook-uri-spec.md`, D-16) carrying the inputs-are-logged privacy warning (Codex MEDIUM #10) and the per-read render-cost note (Codex LOW).**

## Performance
- **Duration:** ~10 min
- **Completed:** 2026-06-10
- **Tasks:** 3 (Task 1 + Task 2 `type="auto" tdd="true"`, Task 3 `type="auto"`; no checkpoints)
- **Files:** 6 (3 created, 3 modified)

## Accomplishments
- **Task 1 (render_uri.rs + render_workbook handler):** Created the `workbook://` codec — `MAX_ENCODED_URI_LEN = 64 KiB`, `encode(dto, provenance) -> String` (base64url over `{ dto, provenance }`), and `decode(uri) -> Result<DecodedRender, _>` that checks the **size guard FIRST** (reject oversized before any base64 — T-92-14/V12), then does a TOTAL, panic-free decode (scheme-prefix → base64 → JSON, each arm an `Err`, never a panic — T-92-17). Added `RenderWorkbookHandler` (the 5th `ToolHandler`): `validate_input` → `render_uri::encode` → return the `workbook://` POINTER + `ProvStamp` in `structuredContent` (the bytes are NOT in the response); invalid input → `isError:true`. Added `render_workbook_output_schema` (non-empty, WBSV-07) and the codec proptest pair (`prop_encode_decode_identity` round-trip + determinism; `prop_decode_total` decode-totality fuzz over arbitrary/adversarial/oversized strings — CLAUDE.md ALWAYS-fuzz).
- **Task 2 (render_resource.rs):** Created `RenderWorkbookResource` (`#[async_trait] impl ResourceHandler`): `list` returns the SINGLE `workbook://render/` entry (A3 — no `DispatchingResource`); `read` runs the stateless regen pipeline — `decode` (size-guard inside) → **verify** decoded provenance == the live bundle stamp (cross-provenance → error BEFORE rendering, T-92-15) → **re-validate** decoded inputs through `validate_input` (out-of-range/injected → error, not render, T-92-16) → re-run the executor → `render_xlsx(&bundle.layout, &run)` → base64-STANDARD → `ReadResourceResult` via the MIME-typed-wire `Content::resource_with_text` idiom carrying the OOXML xlsx MIME. `read` is decomposed into `regenerate()` + `RegenError::into_protocol()` (each fn under cognitive complexity 25). Tests prove byte-identical-across-reads determinism, the cross-provenance rejection, the re-validation rejection, the oversized rejection, and the single-resource list.
- **Task 3 (docs/workbook-uri-spec.md):** Published the `workbook://` URI contract following the dialect-spec precedent — scheme/authority/path layout, the encoded payload (`dto` + `provenance` triple with `combined_hash`), base64url encoding, the `MAX_ENCODED_URI_LEN` (64 KiB) size bound, the stateless-regen-on-read semantics + the §5 security properties (size-guard-first, provenance + re-validation gates), a **PRIVACY WARNING** (the URI encodes inputs and may be logged by clients/proxies; opaque-handle noted as a future versioned evolution — Codex MEDIUM #10), a **RENDER-COST** note (per-read re-run + re-render, no cache; rate-limiting as an operator extension point — Codex LOW), and a versioning-decision clause (D-16). Tax-domain examples only (S-4).

## Task Commits
1. **Task 1: workbook:// URI codec + render_workbook pointer handler** — `fdf1948a` (feat).
2. **Task 2: stateless regen-on-read workbook:// resource handler** — `69036741` (feat).
3. **Task 3: publish the workbook:// URI public contract** — `863e3230` (docs).

_Tasks 1 and 2 are `tdd="true"`; tests and implementation were written and committed atomically per task (the Plan-01/02/03 runtime + compute substrate is the pre-built oracle each test runs against — the golden tax-calc@1.1.0 bundle boots fail-closed, renders a real xlsx ZIP, and round-trips through the codec)._

## Files Created/Modified
- `crates/pmcp-server-toolkit/src/workbook/render_uri.rs` — workbook:// codec (size-guard-first decode, deterministic encode, WORKBOOK_XLSX_MIME) + codec proptests (created).
- `crates/pmcp-server-toolkit/src/workbook/render_resource.rs` — RenderWorkbookResource stateless regen-on-read ResourceHandler + RegenError boundary mapping (created).
- `docs/workbook-uri-spec.md` — the published versioned workbook:// URI contract (created).
- `crates/pmcp-server-toolkit/src/workbook/handler.rs` — RenderWorkbookHandler (5th ToolHandler) + tests (modified).
- `crates/pmcp-server-toolkit/src/workbook/schema.rs` — render_workbook_output_schema (modified).
- `crates/pmcp-server-toolkit/src/workbook/mod.rs` — render_uri/render_resource module decls + re-exports (modified).

## Decisions Made
- **WORKBOOK_XLSX_MIME lives in render_uri.rs (Task 1), not render_resource.rs:** Task 1's `render_workbook` handler advertises the mime in its result and Task 1 must compile self-contained, so the one MIME constant lands with the codec and both the tool (the `mime_type` field) and the read handler (the `Content` MIME) reference it.
- **A private ProvenanceWire serde struct mirrors ProvStamp on the wire:** with `From` conversions both ways, keeping the JSON payload shape decoupled from the public `ProvStamp` type while preserving the `combined_hash`-never-source-hash field-naming contract (Codex HIGH #3).
- **A workbook:// READ failure is a protocol Error, not the isError:true tool envelope:** `resources/read` has no `structuredContent` channel, and a bad/oversized/cross-provenance/invalid resource URI is an infrastructure fault (the client handed us a bad pointer) — distinct from a tool DOMAIN failure. `RegenError::into_protocol` maps to `Error::protocol(INVALID_PARAMS | INTERNAL_ERROR, ..)` exactly as `resources.rs` does.
- **The resources/list entry is the scheme root (no payload):** `workbook://render/` is a stable listable handle; the payload-bearing URIs are minted per `render_workbook` call. Exactly one resource (A3 — the `DispatchingResource` wrapper and the value-schema:// resource were NOT lifted).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Test/Doc] `workbook_hash` grep-gate literal in a render_uri.rs doc comment reworded**
- **Found during:** Task 1 acceptance grep (`grep -rn "workbook_hash" render_uri.rs` must be 0).
- **Issue:** A doc comment on `ProvenanceWire` stated the field carries `combined_hash`, "never `workbook_hash`" — a load-bearing Codex HIGH #3 CONTRACT statement, but the literal `workbook_hash` tripped the acceptance grep (the same situation Plan 03 hit on `supply_total`/`Err(pmcp::Error)`).
- **Fix:** Reworded the doc comment to "the `combined_hash` integrity anchor and NEVER the source-workbook content hash (Codex HIGH #3)". The asserted contract (the wire carries the combined hash, never the source-workbook hash) is unchanged.
- **Files modified:** crates/pmcp-server-toolkit/src/workbook/render_uri.rs
- **Verification:** `grep -rn "workbook_hash" render_uri.rs` = 0; all 6 render_uri tests still green.
- **Committed in:** fdf1948a (folded into the Task 1 commit before it was made).

---

**Total deviations:** 1 auto-fixed (doc literal hygiene to keep an acceptance grep green without weakening the contract).
**Impact on plan:** No scope change.

## Issues Encountered
- `cargo fmt -p pmcp-server-toolkit` reformats three **Plan-02** test-support files (`tests/support/fixture_gen.rs`, `tests/support/tamper.rs`, `tests/fixture_byte_stability.rs`) — they were committed with a slightly older rustfmt formatting and a current-toolchain `fmt --check` flags them. They are NOT touched by Plan 04 (whitespace-only drift). Per the executor scope boundary they were reverted to keep Plan-04 commits scoped and logged to the phase `deferred-items.md`; they should be picked up by the workspace-wide `make quality-gate` run in Plan 05 (which wires the builder-ext + purity gate + example). My own new code is fmt-clean and clippy-clean (verified via `cargo clippy --no-default-features --features workbook --lib --tests`, which excludes the default `code-mode` file carrying a pre-existing unused-import warning logged by Plan 03).

## Threat Flags
None — all new surface is enumerated in the plan's `<threat_model>` (T-92-14…T-92-18) and mitigated: the oversized URI is rejected before any decode (T-92-14, size-guard-first); a cross-provenance/forged URI errors before rendering (T-92-15, provenance == bundle stamp); injected inputs are re-validated through `validate_input` on every read (T-92-16); decode is total + panic-free, proptest-proven via `prop_decode_total` (T-92-17); and the regen-on-read pipeline is stateless with no server-side render state to leak (T-92-18 accept). The privacy property (the URI encodes inputs) is documented as a warning in the published contract (Codex MEDIUM #10), not a code surface.

## Known Stubs
None — `render_workbook` + the `workbook://` resource are fully wired and tested against the tax-calc golden (a real xlsx ZIP renders, the codec round-trips, the four hardening gates each reject). The builder-ext wiring (registering the 5 handlers + the resource into a server builder), the example, and the purity gate land in Plan 05 — that is planned phasing, not a stub.

## Next Phase Readiness
- Plan 05 can register all 5 handlers (`Calculate`/`Explain`/`GetManifest`/`DiffVersion`/`RenderWorkbook`) plus the single `RenderWorkbookResource` into a builder extension + example; every handler/resource is constructed from `Arc<WorkbookBundle>` and re-exported from `workbook::`.
- Plan 05's workspace `make quality-gate` run should sweep up the three Plan-02 fmt-drift test-support files logged in `deferred-items.md`.
- Phase 93's compiler re-emits the tax-calc bundle; this plan's `render_resource` byte-identity test pins the determinism contract (fixed doc properties → byte-identical render) the compiler's emitted layout must keep satisfying.

## Self-Check: PASSED
- FOUND: crates/pmcp-server-toolkit/src/workbook/render_uri.rs
- FOUND: crates/pmcp-server-toolkit/src/workbook/render_resource.rs
- FOUND: docs/workbook-uri-spec.md
- FOUND commit: fdf1948a (Task 1 — render_uri codec + render_workbook handler)
- FOUND commit: 69036741 (Task 2 — render_resource stateless regen-on-read)
- FOUND commit: 863e3230 (Task 3 — published workbook:// URI contract)

---
*Phase: 92-bundlesource-served-tool-toolkit-module*
*Completed: 2026-06-10*
