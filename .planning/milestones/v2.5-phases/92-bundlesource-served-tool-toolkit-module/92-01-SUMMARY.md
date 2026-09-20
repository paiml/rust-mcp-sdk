---
phase: 92-bundlesource-served-tool-toolkit-module
plan: 01
subsystem: infra
tags: [workbook-runtime, bundle-source, integrity, sha256, include_dir, serde, fail-closed]

# Dependency graph
requires:
  - phase: 91-workbook-runtime-purity-gate
    provides: "reader-free pmcp-workbook-runtime crate (artifact_model build_bundle_lock/sha256_hex/update_field, manifest_model, render LayoutDescriptor, changelog VersionChangelog, sheet_ir build_dag/Cell, dag Dag)"
provides:
  - "BundleSource trait (sync, dumb-byte, object-safe, Send+Sync) + LocalDirSource + feature-gated EmbeddedSource + BundleSourceError"
  - "BundleLoader::load — the single shared fail-closed verifier (WorkbookBundle + BundleLoadError) for any BundleSource"
  - "Runtime on-disk contract scrub: BundleLock.bundle_id (was workflow), CellEntry.json_key (was plot3_json_key), no CellMap.supply_total_cell"
  - "Manifest.annotations additive serde-default-empty field + AnnotationDecl type"
  - "embedded feature gating include_dir 0.7.4; runtime stays reader-free, no base64"
affects: [92-02-served-tool-toolkit, 92-03-provstamp, workbook-server-scaffold]

# Tech tracking
tech-stack:
  added: [include_dir 0.7.4 (optional, behind embedded feature)]
  patterns:
    - "Dumb-byte source trait + single shared loader = type-level integrity-bypass impossibility"
    - "Fail-closed frozen-member allow-set before parsing"
    - "Loader reuses the runtime's own build_bundle_lock/update_field (no re-hashing)"
    - "Additive serde-default-empty field (skip_serializing_if) for backward-compatible manifest evolution"

key-files:
  created:
    - crates/pmcp-workbook-runtime/src/bundle_source.rs
    - crates/pmcp-workbook-runtime/src/bundle_loader.rs
    - crates/pmcp-workbook-runtime/tests/fixtures/embedded_bundle/manifest.json
    - crates/pmcp-workbook-runtime/tests/fixtures/embedded_bundle/evidence/changelog.json
  modified:
    - crates/pmcp-workbook-runtime/src/artifact_model.rs
    - crates/pmcp-workbook-runtime/src/manifest_model.rs
    - crates/pmcp-workbook-runtime/src/lib.rs
    - crates/pmcp-workbook-runtime/Cargo.toml

key-decisions:
  - "BundleSource is SYNC (no async_trait, D-07): a boot-path byte accessor needs no executor and stays object-safe + Send+Sync"
  - "Loader does NOT re-implement hashing — it calls the runtime's own build_bundle_lock + update_field so it byte-reproduces the emitter (no integrity false-positives)"
  - "BundleLock.workbook_hash KEPT (Codex HIGH #3): it is the SOURCE-workbook hash, not the combined stamp — only ProvStamp (Plan 03) renames its combined-hash field"
  - "Folded Task 4's Cargo.toml embedded-feature/include_dir edit into the Task 2 commit because Task 2's --features embedded tests require the feature to exist; Task 4 became verification-only"
  - "Used a self-cleaning std::env::temp_dir TempBundle helper instead of adding a tempfile dev-dependency — keeps the runtime crate lean and avoids a new package vet"

patterns-established:
  - "Frozen member allow-set (ALLOWED_MEMBERS) checked before any parse — UnexpectedMember fail-closed (threat T-92-22)"
  - "verify_stamp_binding cross-checks the lock's identity triple against independently hash-covered members (threat T-92-02)"
  - "include_dir gated-dep comment-discipline block mirroring the Phase-91 rust_xlsxwriter precedent (author/license/repo/audit)"

requirements-completed: [WBSV-08, WBSV-09, WBSV-02]

# Metrics
duration: ~45min
completed: 2026-06-10
---

# Phase 92 Plan 01: BundleSource + fail-closed BundleLoader + runtime model scrub Summary

**A dumb-byte BundleSource trait (local-dir + embedded include_dir impls) plus one shared fail-closed BundleLoader that recomputes the BUNDLE.lock hash-of-hashes via the runtime's own hasher — rejecting tampered, desynced, malformed, or unexpected-member bundles before boot — with the D-17/S-1/S-4 contract scrub and the D-18 additive annotations field.**

## Performance

- **Duration:** ~45 min
- **Completed:** 2026-06-10
- **Tasks:** 4 (Task 1 checkpoint resolved by human approval; Tasks 2–4 executed)
- **Files modified:** 8 (4 created, 4 modified)

## Accomplishments
- `BundleSource` trait: sync, object-safe, `Send + Sync`, exactly `read_artifact` + `list_artifacts` (raw bytes only — no impl can return a parsed bundle, so WBSV-08 cannot be bypassed; threat T-92-03 is type-level impossible).
- `LocalDirSource` (recursive sorted member walk) + feature-gated `EmbeddedSource` over `include_dir::Dir` (WBSV-09); a committed fixture proves embedded bytes are byte-identical to local-dir bytes.
- `BundleLoader::load`: the single shared verifier. Fail-closed on an unexpected/extra member (UnexpectedMember, T-92-22) BEFORE parsing; recomputes the evidence hash + per-artifact/combined lock via the runtime's own `build_bundle_lock`/`update_field` and returns IntegrityMismatch on a byte-flip (T-92-01); cross-checks the identity triple (StampMismatch, T-92-02); total panic-free parse (Parse, T-92-04); builds the DAG once.
- Runtime on-disk contract scrub: `BundleLock.workflow`→`bundle_id` (D-17), `CellEntry.plot3_json_key`→`json_key` (S-4), dropped `CellMap.supply_total_cell` (S-1); KEPT `BundleLock.workbook_hash` (Codex HIGH #3); scrubbed customer-name test strings/doc-comments.
- `Manifest.annotations: Vec<AnnotationDecl>` additive serde-default-empty field (D-18) cloning the `allowed_values` precedent — old manifests without the key deserialize to an empty Vec; empty Vecs are skipped from serialization.
- `include_dir 0.7.4` gated behind `embedded = ["dep:include_dir"]`; runtime stays reader-free (cargo-tree shows no umya/calamine/quick-xml/swc_/pmcp-code-mode), carries no base64 (Codex MEDIUM #6), and `cargo audit` is clean.

## Task Commits

1. **Task 1: Vet + install include_dir 0.7.4 (package legitimacy gate)** — checkpoint resolved out-of-band (human typed "approved" after crates.io verification: author Michael-F-Bryan, MIT OR Apache-2.0, repo github.com/Michael-F-Bryan/include_dir, 53.4M downloads, v0.7.4 latest, not yanked). No standalone commit — the authorized Cargo.toml edit landed in the Task 2 commit (it is a Task 2 test prerequisite).
2. **Task 2: BundleSource trait + LocalDirSource + EmbeddedSource** — `963374fc` (feat) — also carries the Task 1-authorized `include_dir` optional dep + `embedded` feature.
3. **Task 3: fail-closed BundleLoader + runtime model scrub** — `33633d44` (feat).
4. **Task 4: wire embedded feature + deps + reader-free/audit verification** — Cargo.toml edit committed in `963374fc` (prerequisite for Task 2); Task 4's remaining work was verification-only (build with/without embedded, reader-free cargo-tree, cargo audit clean) — all passed, no additional file changes.

_Note: Tasks 2 and 3 are `tdd="true"`; tests and implementation were written together and committed atomically per task (single feat commit each, RED verified by running new tests before/after wiring)._

## Files Created/Modified
- `crates/pmcp-workbook-runtime/src/bundle_source.rs` — BundleSource trait + LocalDirSource + EmbeddedSource + BundleSourceError (created).
- `crates/pmcp-workbook-runtime/src/bundle_loader.rs` — load() single shared fail-closed verifier + WorkbookBundle + BundleLoadError (created).
- `crates/pmcp-workbook-runtime/src/artifact_model.rs` — bundle_id/json_key renames, supply_total_cell removal, customer-name scrub (modified).
- `crates/pmcp-workbook-runtime/src/manifest_model.rs` — AnnotationDecl + additive Manifest.annotations (modified).
- `crates/pmcp-workbook-runtime/src/lib.rs` — module decls + re-exports for the new surfaces (modified).
- `crates/pmcp-workbook-runtime/Cargo.toml` — include_dir optional dep + embedded feature with discipline comment (modified).
- `crates/pmcp-workbook-runtime/tests/fixtures/embedded_bundle/{manifest.json,evidence/changelog.json}` — committed embedded-source fixture (created).

## Decisions Made
- **Sync trait (D-07):** no `async_trait` — keeps the trait object-safe and Send+Sync without an executor; a boot-path byte accessor has no concurrency need.
- **Loader reuses the runtime's own hasher:** `build_bundle_lock` + `update_field` are called, never re-implemented, so the loader byte-reproduces the emitter and integrity never false-positives.
- **workbook_hash KEPT (Codex HIGH #3):** it is the source-workbook hash, distinct from the combined integrity stamp; only the Plan-03 ProvStamp renames its combined-hash field to `combined_hash`.
- **TempBundle over a tempfile dep:** a self-cleaning `std::env::temp_dir`-based test helper avoids adding (and vetting) a new dev-dependency.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Folded Task 4's Cargo.toml edit into the Task 2 commit**
- **Found during:** Task 2 (BundleSource embedded test)
- **Issue:** Task 2's acceptance requires `cargo test --features embedded` to pass and the file to be warning-free, but the `embedded` feature did not exist until Task 4's Cargo.toml edit — so Task 2 could neither compile the embedded `EmbeddedSource`/test nor satisfy CLAUDE.md zero-warning (the `#[cfg(feature="embedded")]` blocks produced `unexpected cfg` warnings).
- **Fix:** Added the Task-1-authorized `include_dir = { version = "0.7.4", optional = true }` dep + `embedded = ["dep:include_dir"]` feature (with the full discipline comment block) as part of the Task 2 commit. Task 4 then reduced to verification-only (build matrix + reader-free cargo-tree + cargo audit), all of which passed.
- **Files modified:** crates/pmcp-workbook-runtime/Cargo.toml
- **Verification:** `cargo test -p pmcp-workbook-runtime bundle_source` (5 pass) and `--features embedded` (6 pass); zero clippy warnings both configs.
- **Committed in:** 963374fc (Task 2 commit)

**2. [Rule 3 - Blocking] Replaced the planned tempfile usage with a no-dependency TempBundle helper**
- **Found during:** Task 2 (LocalDirSource filesystem tests)
- **Issue:** The natural test approach uses `tempfile::tempdir()`, but `tempfile` is not a dependency of this lean reader-free crate, and adding a new crate triggers the package-vetting discipline (Rule 3 install exclusion).
- **Fix:** Wrote a self-cleaning `TempBundle` test helper over `std::env::temp_dir()` + a process-id/atomic-counter unique path + `Drop`-based cleanup — no new dependency.
- **Files modified:** crates/pmcp-workbook-runtime/src/bundle_source.rs (test module only)
- **Verification:** LocalDirSource read/list/not-found tests pass; no new Cargo.toml dep.
- **Committed in:** 963374fc (Task 2 commit)

**3. [Rule 1 - Bug] Scrubbed a residual customer name in the artifact_model module doc-comment**
- **Found during:** Task 3 (S-4 scrub acceptance grep `ufh|quote|coil|towelrad|plot.?3|first_fix`)
- **Issue:** The module-level doc-comment still named `quote-pricing-server` (the lighthouse served binary), which the S-4 scrub grep flags.
- **Fix:** Replaced the customer-specific reference with the neutral "the served binary".
- **Files modified:** crates/pmcp-workbook-runtime/src/artifact_model.rs
- **Verification:** `grep -rniE "ufh|quote|coil|towelrad|plot.?3|first_fix" artifact_model.rs` returns 0.
- **Committed in:** 33633d44 (Task 3 commit)

---

**Total deviations:** 3 auto-fixed (2 blocking, 1 bug)
**Impact on plan:** All three were necessary to satisfy the plan's own acceptance criteria (embedded-feature tests, no-new-dependency discipline, S-4 scrub completeness). The only structural shift is task-commit attribution: Task 4's Cargo.toml edit landed in the Task 2 commit because it is a hard prerequisite for Task 2's tests — no scope was added or dropped, and every Task 4 verification still ran and passed.

## Issues Encountered
- `sheet_ir` and `render` are module directories; `Dag` is re-exported from `crate::dag` (not `sheet_ir`). Resolved the loader's import to `crate::dag::Dag` after a one-line compile fix.
- `cargo fmt --features embedded` re-flowed the feature-gated embedded test (the default-feature fmt pass had not seen it); committed the whitespace fix as part of Task 3.

## Threat Flags

None — all new surface (bundle byte access + the new include_dir dep) is already enumerated in the plan's `<threat_model>` (T-92-01/02/03/04/22/SC) and mitigated by the loader's fail-closed gates + the human-verified, audited dependency.

## Known Stubs

None — the embedded fixture under `tests/fixtures/embedded_bundle` is a real test golden, not a UI stub; LocalDirSource/EmbeddedSource/BundleLoader are fully wired with passing tests.

## User Setup Required
None at runtime. The `include_dir 0.7.4` install was gated behind the Task 1 blocking human-verify and APPROVED (crates.io identity confirmed); `cargo audit` is clean.

## Next Phase Readiness
- The consumer side of the bundle contract is frozen from the leaf crate: Plan 02 (served-tool toolkit module) can build `calculate`/`explain`/`get_manifest`/`diff_version`/`render_workbook` on top of `BundleLoader::load` + `WorkbookBundle`, and Plan 03's ProvStamp can surface `stamp.combined` as `combined_hash` (kept distinct from `workbook_hash`).
- Runtime stays reader-free with the embedded feature; the BundleSource extension seam (S3/registry) is documented but unimplemented by design.

## Self-Check: PASSED
- FOUND: crates/pmcp-workbook-runtime/src/bundle_source.rs
- FOUND: crates/pmcp-workbook-runtime/src/bundle_loader.rs
- FOUND: crates/pmcp-workbook-runtime/src/manifest_model.rs
- FOUND: crates/pmcp-workbook-runtime/src/artifact_model.rs
- FOUND: crates/pmcp-workbook-runtime/Cargo.toml
- FOUND commit: 963374fc (Task 2)
- FOUND commit: 33633d44 (Task 3)

---
*Phase: 92-bundlesource-served-tool-toolkit-module*
*Completed: 2026-06-10*
