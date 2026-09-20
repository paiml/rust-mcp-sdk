---
phase: 92-bundlesource-served-tool-toolkit-module
plan: 05
subsystem: pmcp-server-toolkit (workbook served-tool module)
tags: [workbook, builder-ext, streamable-http, purity-gate, fail-closed, embedded-source]
requires:
  - "pmcp-workbook-runtime: BundleSource/LocalDirSource/EmbeddedSource + load_bundle (92-01)"
  - "pmcp-server-toolkit workbook feature skeleton + tests/support tamper helpers (92-02)"
  - "workbook handlers: Calculate/Explain/GetManifest/DiffVersion (92-03)"
  - "RenderWorkbookHandler + RenderWorkbookResource (92-04)"
provides:
  - "WorkbookBuilderExt::with_workbook_bundle / try_with_workbook_bundle — one-call boot+register"
  - "Crate-root + module re-exports of the full boot surface (D-11)"
  - "ToolkitError::Workbook(#[from] BundleLoadError) (workbook-gated)"
  - "examples/workbook_server_http.rs — streamable-HTTP server over all five tools"
  - "tests/workbook_integration.rs — build-and-assert + tamper-fails-boot + boot smoke-run"
  - "Makefile purity-check: per-feature toolkit workbook[-embedded] reader-absence gate"
affects:
  - "Shape A/B workbook servers (consume only pmcp-server-toolkit)"
  - "CI purity job (make purity-check now covers the toolkit workbook features)"
tech-stack:
  added:
    - "include_dir 0.7.4 — optional toolkit dep gated on workbook-embedded (macro needs a direct, nameable crate)"
  patterns:
    - "panicking-convenience + fallible-companion builder-ext pair (mirrors ServerBuilderExt, review R7)"
    - "fail-closed boot load (load_bundle before any tool registers, WBSV-08)"
    - "distinct per-feature purity assertion (NOT appended to PURITY_CRATES)"
key-files:
  created:
    - "crates/pmcp-server-toolkit/examples/workbook_server_http.rs"
    - "crates/pmcp-server-toolkit/tests/workbook_integration.rs"
  modified:
    - "crates/pmcp-server-toolkit/src/workbook/mod.rs (WorkbookBuilderExt + re-exports)"
    - "crates/pmcp-server-toolkit/src/lib.rs (crate-root re-exports + smoke consts)"
    - "crates/pmcp-server-toolkit/src/error.rs (Workbook variant)"
    - "crates/pmcp-server-toolkit/Cargo.toml ([[example]] + include_dir dep)"
    - "Makefile (purity-check toolkit workbook[-embedded] assertion)"
decisions:
  - "Used the runtime's free fn load_bundle(source) (re-exported) — there is no BundleLoader struct; the plan's `BundleLoader::load` interface text is shorthand for this established runtime API (handler.rs already uses load_bundle)."
  - "include_dir is a DIRECT optional toolkit dep (not a runtime re-export) because the include_dir! macro emits unqualified `include_dir::` paths, so the crate must be nameable at the consumer's root. Gated on workbook-embedded via dep:include_dir, keeping it out of the plain workbook / default builds."
  - "The toolkit purity assertion runs --no-default-features so the toolkit's default code-mode (which legitimately pulls pmcp-code-mode, a BAN-listed crate) does not falsely trip the gate; the assertion proves the WORKBOOK feature itself adds no reader."
metrics:
  duration: ~25m
  completed: 2026-06-10
  tasks: 3
  files-created: 2
  files-modified: 6
  commits: 5
---

# Phase 92 Plan 05: Workbook Toolkit Wiring + End-to-End Proof Summary

One-liner: A single `WorkbookBuilderExt::with_workbook_bundle(source)` call now
loads + integrity-verifies a governed-Excel bundle fail-closed at boot and
registers all five served tools + the `workbook://` resource — the full boot
surface re-exported so Shape A/B consumers never name `pmcp-workbook-runtime`,
proven by a streamable-HTTP example, build/tamper/boot integration tests, and a
purity gate that keeps both the `workbook` and `workbook-embedded` trees
reader-free.

## What shipped

**Task 1 — Builder-ext registration + boot-surface re-exports (commit 39d31c8c)**
- `WorkbookBuilderExt` in `workbook/mod.rs`: `with_workbook_bundle` (panicking,
  `# Panics`) + `try_with_workbook_bundle` (fallible, `# Errors`) for
  `ServerBuilder`, mirroring `builder_ext.rs:260-318`. `try_` calls
  `Arc::new(load_bundle(source)?)` (WBSV-08 fail-closed boot), then registers
  `calculate`/`explain`/`get_manifest`/`diff_version`/`render_workbook` via
  `tool_arc` and the single `workbook://` resource via `resources_arc`
  (`RenderWorkbookResource`, no DispatchingResource wrapper — A3). Emits a
  `tracing::warn!` when `bundle.cell_map.outputs` is empty (operator visibility,
  builder_ext.rs:273-279 idiom).
- Crate-root + module re-exports of the full boot surface (D-11): `BundleSource`,
  `LocalDirSource`, `EmbeddedSource` (cfg `workbook-embedded`), `BundleSourceError`,
  `BundleLoadError`, `load_bundle`, `WorkbookBundle`, `WorkbookBuilderExt`.
- `ToolkitError::Workbook(#[from] pmcp_workbook_runtime::BundleLoadError)`,
  feature-gated on `workbook` so the `?` in `try_with_workbook_bundle` converts a
  load failure into the toolkit error type.
- Extended `_WORKBOOK_REEXPORT_SMOKE` / `_WORKBOOK_EMBEDDED_REEXPORT_SMOKE`
  compile-const witnesses.

**Task 2 — Streamable-HTTP example + integration tests (commit 58ade148)**
- `examples/workbook_server_http.rs`: builds `Server::builder().name("workbook-tax-calc")`,
  chains `.try_with_workbook_bundle(&source)?`, serves over `StreamableHttpServer`
  bound to `127.0.0.1:0`, prints `PMCP_WORKBOOK_SERVER_ADDR=`. Default source is
  the embedded committed `@1.1.0` golden via the EXACT
  `include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/tax-calc@1.1.0")` (no
  examples/fixtures copy — Codex HIGH #5); `--bundle-dir` switches to a
  `LocalDirSource` with a doc-comment explaining the embedded→local production
  transition (Gemini).
- `[[example]] workbook_server_http` with `required-features = ["workbook-embedded", "http"]`.
- `tests/workbook_integration.rs`: (1) build-and-assert — all five tools
  `get_tool(...).is_some()`; (2) tamper-fails-boot — `try_with_workbook_bundle`
  over a `copy_golden_to_temp()` + `flip_byte("manifest.json")` returns `Err`
  (WBSV-08 through the builder); (3) ephemeral-port boot smoke-run (Codex MEDIUM
  #11, cfg `workbook-embedded,http`) — boots the embedded-bundle server within a
  5s timeout, asserts all five tools registered, aborts the serve task for a
  clean shutdown (no hang, no leaked socket).

**Task 3 — Purity gate extension (commit d0883b92)**
- `Makefile purity-check`: a DISTINCT per-feature reader-absence assertion for
  `pmcp-server-toolkit --features workbook` AND `--features workbook-embedded`
  (T-92-19, WBRT-04 forward), using `--no-default-features` + the reused BAN
  regex (`umya|calamine|quick-xml|swc_|pmcp-code-mode`), fail-closed on a
  non-zero cargo status. The toolkit is NOT appended to `PURITY_CRATES` (it is
  not unconditionally reader-free; RESEARCH Pitfall 1). `just purity-check`
  already delegates to `make purity-check`; CI already invokes `make purity-check`
  (no CI edit needed).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] include_dir added as a direct toolkit dependency**
- **Found during:** Task 2 — the example failed to compile (`unresolved import include_dir`).
- **Issue:** The plan's interface implies the `include_dir!` macro is reachable
  through the runtime/toolkit, but the macro emits UNQUALIFIED `include_dir::Dir`
  paths, so the crate must be a direct, nameable dependency at the consumer's
  root. A re-export does NOT satisfy the macro.
- **Fix:** Added `include_dir = { version = "0.7.4", optional = true }` to the
  toolkit and gated it on `workbook-embedded` via `dep:include_dir`. Verified the
  plain `workbook` tree still has zero `include_dir` (purity intact). It is not a
  reader/JS crate, so the purity BAN regex does not flag it.
- **Files modified:** `Cargo.toml`, `examples/workbook_server_http.rs`, `workbook/mod.rs`.
- **Commit:** 58ade148

**2. [Rule 2 - Missing critical functionality] ToolkitError::Workbook variant**
- **Found during:** Task 1 — `try_with_workbook_bundle`'s `?` needs a
  `From<BundleLoadError>` for the toolkit `Result`.
- **Fix:** Added a `workbook`-gated `Workbook(#[from] BundleLoadError)` variant.
- **Commit:** 39d31c8c

### Interface reconciliation (not a code deviation)

The plan's `<interfaces>` names `BundleLoader::load(...)`. The runtime exposes a
free function `load` re-exported as `load_bundle` — there is no `BundleLoader`
struct. I used `load_bundle(source)`, the established pattern already used by
`workbook/handler.rs` and `render_resource.rs`. The key-link intent (fail-closed
boot load via the runtime loader) is satisfied.

### Deferred-items sweep (commit bc78fc47)

The phase `deferred-items.md` flagged pre-existing rustfmt drift in three Plan-02
test-support files to be picked up by the first plan running `make quality-gate`.
Plan 05 ran the gate, so the whitespace-only reformat is committed here as
`style(92-05)`. Marked RESOLVED in deferred-items.md.

## Out-of-scope discovery (logged, not fixed)

A default-on `clippy::redundant_guards` warning in `src/http/auth.rs:538`
(Phase-90 OpenAPI file, surfaces only under `--features http`) is unrelated to
this plan's workbook work. Logged to `deferred-items.md`. `make quality-gate`
PASSES regardless (the toolkit crate is not pedantic-gated; the real CI lints
only root `pmcp`).

## Verification

- `cargo build -p pmcp-server-toolkit` (no features) — PASS
- `cargo build -p pmcp-server-toolkit --features workbook,http` — PASS
- `cargo test -p pmcp-server-toolkit --features workbook-embedded,http --test workbook_integration` — 3/3 PASS
- `cargo test -p pmcp-server-toolkit --features workbook,http --test workbook_integration` — 2/2 PASS (smoke-run correctly gated to embedded)
- `cargo build --example workbook_server_http --features workbook-embedded,http` — PASS
- `cargo tree --features workbook | grep -c include_dir` → 0; `--features workbook-embedded` → present
- `make purity-check` — PASS (toolkit workbook + workbook-embedded reader-free)
- `make quality-gate` — PASS (fmt/clippy/build/test-all/examples/purity)

## Threat-model coverage

- **T-92-19** (reader enters served tree via feature unification): mitigated —
  distinct `--features workbook` AND `--features workbook-embedded` reader-absence
  assertions, merge-blocking, fail-closed (Task 3).
- **T-92-20** (tampered bundle served): mitigated — `try_with_workbook_bundle`
  loads via `load_bundle` (fail-closed); the tamper-fails-boot integration test
  proves `Err` propagates through the builder (Tasks 1-2).
- **T-92-21** (consumer reaching runtime directly): accepted — the toolkit
  re-exports the full boot surface (D-11); direct runtime use stays reader-free.
- **T-92-SC** (package installs): mitigated — `include_dir` is a vetted, pinned
  (0.7.4) crate already present in the workspace/embedded tree (no new untrusted
  install); the purity gate keeps reader/slop crates out of the served tree.

## Self-Check: PASSED

All four key files exist on disk and all five commits (39d31c8c, 58ade148, d0883b92, bc78fc47, 179c2c22) are present in the git log. Working tree clean.
