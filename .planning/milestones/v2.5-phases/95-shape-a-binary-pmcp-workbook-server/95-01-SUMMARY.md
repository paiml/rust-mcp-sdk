---
phase: 95-shape-a-binary-pmcp-workbook-server
plan: 01
subsystem: pmcp-workbook-server
tags: [shape-a, binary, workbook, config-driven, mcp-server]
requires:
  - pmcp-server-toolkit (workbook + http features)
  - pmcp (streamable-http)
  - golden fixture crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0
provides:
  - pmcp-workbook-server crate (lib + bin) — Shape A pure-config workbook MCP server
  - run / serve / run_serving pipeline + RunError
  - build_server assemble seam (LocalDirSource + --bundle-id + try_with_workbook_bundle)
  - runnable example workbook_server_min
affects:
  - root Cargo.toml workspace members
tech-stack:
  added:
    - thiserror 2 (RunError)
  patterns:
    - lib+bin split (testable run() in lib, thin #[tokio::main] shim in main.rs)
    - field-for-field re-skin of pmcp-sql-server, swapping only the bundle seam
    - fail-closed boot integrity gate via try_with_workbook_bundle
key-files:
  created:
    - crates/pmcp-workbook-server/Cargo.toml
    - crates/pmcp-workbook-server/src/main.rs
    - crates/pmcp-workbook-server/src/cli.rs
    - crates/pmcp-workbook-server/src/assemble.rs
    - crates/pmcp-workbook-server/src/lib.rs
    - crates/pmcp-workbook-server/examples/workbook_server_min.rs
  modified:
    - Cargo.toml (root — workspace members)
decisions:
  - "RunError drops sql-server's Io variant (D-03 pure CLI args, no file reads — Codex MEDIUM #5)"
  - "--bundle-id accepted double-read is NOT a TOCTOU; the assembly load is the integrity boundary"
  - "tracing target retargeted to pmcp_workbook_server"
metrics:
  duration: ~35m (Task 3 + summary; Tasks 1-2 by prior executor before crash)
  completed: 2026-06-14
requirements: [WBCL-06]
---

# Phase 95 Plan 01: Shape A pmcp-workbook-server Summary

Created the `pmcp-workbook-server` Shape A pure-config binary — a field-for-field
re-skin of `pmcp-sql-server` that stands up a live MCP server serving the five
workbook tools from a compiled `bundle@version` directory alone, with no user Rust,
gated fail-closed by the toolkit's bundle integrity boot check.

## Execution Note

Plan 95-01 was executed across two executor agents. The first agent completed and
committed **Task 1** (crate scaffold) and **Task 2** (assemble.rs seam + a minimal
`RunError`/`build_server` stub in lib.rs) before dying mid-Task-3 on an API socket
error. A continuation agent (this one) completed **Task 3** only — expanding the
lib.rs stub to the full `run`/`serve`/`run_serving` pipeline and adding the runnable
example. No Task 1/2 work was redone. This summary covers all three tasks.

## What Was Built

### Task 1 — Crate scaffold (commit 62c39ff6, prior agent)
- `crates/pmcp-workbook-server/Cargo.toml` (v0.1.0) mirroring sql-server metadata,
  `exclude`, `[package.metadata.docs.rs]`, `[lib]`/`[[bin]]`. Deps: `pmcp`
  (streamable-http), `pmcp-server-toolkit` with `features = ["workbook", "http"]`
  (D-02, NOT workbook-embedded), clap/tokio/tracing/thiserror. **No** direct
  `pmcp-workbook-runtime` dep (D-11) — boot surface reached only via the toolkit.
- `src/main.rs` — thin `#[tokio::main]` shim returning `run()`'s `Result` (non-zero
  exit + legible stderr come from `#[tokio::main]` Debug-printing `RunError`; no
  `eprintln`/`process::exit`).
- `src/cli.rs` — clap `Args` (`--bundle-dir` required, `--bundle-id` optional,
  `--http` default `127.0.0.1:8080`), `#[command(version)]` self-reports v0.1.0,
  **no** `env(...)` overrides (D-03). Inline parse tests incl. `--bundle-id` → None.
- Registered the crate in the root workspace `members`.

### Task 2 — assemble.rs seam (commit d7a11021, prior agent)
- `pub fn build_server(args: &Args) -> Result<Server, RunError>`: `LocalDirSource`
  from `--bundle-dir`, optional `--bundle-id` pre-load assertion via the
  toolkit-re-exported `load_bundle`, then `try_with_workbook_bundle(&source)`
  registering all five tools + the `workbook://` resource.
- Imports come **only** from `pmcp_server_toolkit::workbook` (D-11).
- Documents the accepted double-read / not-a-TOCTOU rationale at the pre-load call
  (the second assembly load independently re-verifies BUNDLE.lock hashes).
- Inline tests: five-tool registration via `Server::get_tool`, matching `--bundle-id`
  succeeds, mismatched `--bundle-id` → `BundleIdMismatch`, missing/tampered bundle →
  `RunError::Bundle` (fail-closed). Task 2 also left a minimal `RunError`+`build_server`
  stub in lib.rs so its tests compiled.

### Task 3 — lib.rs pipeline + runnable example (commit 02292388, this agent)
- Expanded the lib.rs stub to the full pipeline mirroring sql-server:
  - `serve()` — transport-only Tower/axum `StreamableHttpServer` adapter (D-04),
    returns `(bound_addr, JoinHandle)`.
  - `run_serving()` — `build_server(args)?` → parse `--http` to `SocketAddr`
    (failure → `RunError::Addr`) → `serve`.
  - `run()` — best-effort tracing init, awaits the handle with
    `handle.await.map_err(RunError::Serving)?` (crash-surfacing T-85-10-02), tracing
    target retargeted to `"pmcp_workbook_server"`.
- `RunError`: `Bundle(#[from] ToolkitError)`, `BundleIdMismatch { expected, actual }`,
  `Addr { addr, source }`, `Serve(#[source] pmcp::Error)`,
  `Serving(#[source] JoinError)`. **No `Io` variant** — the binary reads no
  config/schema files (D-03), so an `Io` variant would be unconstructible (Codex
  MEDIUM #5). sql-server's `Io` is deliberately dropped.
- Inline tests: `invalid_http_maps_to_run_error_addr` (new, Codex MEDIUM #3 — uses
  the VALID golden bundle so the failure isolates the address-parse, asserting the
  `Addr` variant is reached and serving never is), plus the verbatim
  `serving_task_panic_maps_to_run_error_serving` and
  `run_error_serving_display_is_descriptive` crash-surfacing tests.
- `examples/workbook_server_min.rs` — the ALWAYS runnable example: builds a server
  from the committed synthetic golden `tax-calc@1.1.0` bundle (resolved from
  `CARGO_MANIFEST_DIR`), prints the server identity, uses **only** synthetic golden
  material (zero customer/TowelRads data, D-05).

## Verification

| Check | Result |
|-------|--------|
| `cargo test -p pmcp-workbook-server --lib` | 12 passed, 0 failed |
| `cargo test -p pmcp-workbook-server --doc` | 4 passed |
| `cargo run -p pmcp-workbook-server --example workbook_server_min` | exit 0 ("calculate present: true") |
| `cargo clippy -p pmcp-workbook-server --all-targets` | zero warnings on this crate |
| `cargo fmt --check -p pmcp-workbook-server` | clean |
| grep: `pub async fn run_serving` / `pub async fn run` | present |
| grep: no `Io {` variant in RunError | confirmed (0) |
| grep: `handle.await.map_err(RunError::Serving)?` | present (T-85-10-02) |
| grep: tracing target `pmcp_workbook_server` | present |
| grep: example references `tax-calc@1.1.0`, no customer material | confirmed |
| grep: D-11 (no direct `pmcp-workbook-runtime` dep) | confirmed (comment only) |
| grep: D-02 (`workbook` not `workbook-embedded`) | confirmed |

## Deviations from Plan

None — Task 3 executed exactly as written. Tasks 1-2 were completed by the prior
executor per plan; this agent made no changes to them.

## Out-of-Scope Observations (NOT fixed — SCOPE BOUNDARY)

Two pre-existing clippy/warn issues surface in `crates/pmcp-server-toolkit` during
this crate's build and are unrelated to Task 3 changes (not fixed here):
- `code_mode.rs:557` — unused import `pmcp_code_mode::CodeExecutor`.
- `http/auth.rs:538` — redundant guard.

These belong to `pmcp-server-toolkit`, not `pmcp-workbook-server`, and do not affect
this crate's tests, example, or clippy cleanliness.

## Known Stubs

None — `build_server`, `run_serving`, and `run` are fully wired against the live
toolkit boot surface and the golden bundle; no placeholder/empty data paths remain.

## Self-Check: PASSED

- FOUND: crates/pmcp-workbook-server/src/lib.rs
- FOUND: crates/pmcp-workbook-server/examples/workbook_server_min.rs
- FOUND commit 02292388 (Task 3), d7a11021 (Task 2), 62c39ff6 (Task 1)
