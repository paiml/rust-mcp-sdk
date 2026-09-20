---
phase: 95-shape-a-binary-pmcp-workbook-server
verified: 2026-06-14T00:00:00Z
status: passed
score: 3/3 must-haves verified
overrides_applied: 0
re_verification: false
---

# Phase 95: Shape A pmcp-workbook-server Verification Report

**Phase Goal:** A `pmcp-workbook-server` pure-config binary stands up a live MCP server from a compiled bundle alone, with no user Rust — mirroring `pmcp-sql-server` field-for-field (lib `run`/`serve` + thin `main.rs` shim, `RunError` → non-zero exit), selecting a `BundleSource` from CLI args.

**Verified:** 2026-06-14
**Status:** passed
**Re-verification:** No — initial verification

---

## Requirement Coverage

**WBCL-06** (REQUIREMENTS.md line 65): "A `pmcp-workbook-server` pure-config binary stands up a live MCP server from a compiled bundle alone, no user Rust (Shape A, mirroring `pmcp-sql-server`)"
- **Status: SATISFIED** — all three success criteria verified (see below).

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | Running `pmcp-workbook-server --bundle-dir <dir>` (`--bundle-id` optional, `--http` optional) stands up a live MCP server serving the five workbook tools from the compiled bundle, zero user Rust | VERIFIED | `parity_workbook.rs` invokes the real `run_serving` binary path, drives `tools/list` over live HTTP, and asserts all five tools present. `http_smoke.rs` drives a full MCP `initialize` round-trip. Tests pass exit 0. |
| 2 | Binary selects `BundleSource` from CLI args, runs boot integrity gate, surfaces load/integrity failure as typed `RunError` → non-zero exit | VERIFIED | `assemble.rs`: `LocalDirSource::new(&args.bundle_dir)` → `load_bundle` (id check) → `try_with_workbook_bundle`. `RunError::Bundle(#[from] ToolkitError)` and `RunError::BundleIdMismatch` wired. `nonexistent_bundle_dir_fails_closed` unit test asserts `RunError::Bundle`. `bundle_id_props.rs` proptest asserts every non-matching id → `RunError::BundleIdMismatch` fail-closed. `main.rs` returns `run(args).await?` through `Box<dyn Error>` so tokio Debug-prints the error to stderr and exits non-zero. All tests pass. |
| 3 | The published binary (slot 9a) links only `pmcp-server-toolkit[workbook]` + `pmcp-workbook-runtime` (transitively) — purity gate confirms no reader in the tree | VERIFIED | `Cargo.toml` deps: `pmcp-server-toolkit` with `default-features = false, features = ["workbook", "http"]`; no direct `pmcp-workbook-runtime` dep. `make purity-check` output: "pmcp-workbook-server reader-free (umya/calamine/quick-xml/swc_/pmcp-code-mode absent in the served binary tree)" — exit 0. CLAUDE.md line 233: slot 9a entry present. |

**Score:** 3/3 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/pmcp-workbook-server/src/lib.rs` | `pub async fn run_serving`, `pub async fn run`, `RunError` enum (no `Io` variant) | VERIFIED | All three functions present. `RunError` has `Bundle`, `BundleIdMismatch`, `Addr`, `Serve`, `Serving` variants — no `Io`. `handle.await.map_err(RunError::Serving)?` confirmed at line 239. |
| `crates/pmcp-workbook-server/src/cli.rs` | clap `Args` with `--bundle-dir`, `--bundle-id`, `--http` default `127.0.0.1:8080`, `#[command(version)]`, no `env(...)` | VERIFIED | All fields present. `#[command(version)]` at line 46. `#[arg]` attributes at lines 53, 59, 63 have no `env(...)` (the one `env(` hit is a comment documenting D-03 rationale, not a functional override). |
| `crates/pmcp-workbook-server/src/main.rs` | Thin `#[tokio::main]` shim, no `process::exit`, no `eprintln` | VERIFIED | 19-line shim: `Args::parse()` → `pmcp_workbook_server::run(args).await?`. No `process::exit`. |
| `crates/pmcp-workbook-server/src/assemble.rs` | `pub fn build_server`, `LocalDirSource::new`, `try_with_workbook_bundle`, imports from toolkit only (D-11), double-read rationale comment | VERIFIED | All present. Import: `use pmcp_server_toolkit::workbook::{load_bundle, LocalDirSource, WorkbookBuilderExt}`. No `pmcp_workbook_runtime` import. "Accepted double-read (NOT a TOCTOU)" comment at line 55. |
| `crates/pmcp-workbook-server/Cargo.toml` | `pmcp-server-toolkit` dep with `features = ["workbook", "http"]`, no `pmcp-workbook-runtime` dep, version `0.1.0` | VERIFIED | `default-features = false, features = ["workbook", "http"]`. `pmcp-workbook-runtime` appears only in a comment (D-11 rationale). Version `0.1.0`. Workspace member confirmed at root `Cargo.toml` line 541. |
| `crates/pmcp-workbook-server/examples/workbook_server_min.rs` | Runnable example over synthetic golden bundle, exit 0 | VERIFIED | References `tax-calc@1.1.0` via `CARGO_MANIFEST_DIR`. `cargo run -p pmcp-workbook-server --example workbook_server_min` exits 0 printing "calculate present: true". |
| `crates/pmcp-workbook-server/tests/assemble.rs` | Five-tool surface assertion via `Server::get_tool` | VERIFIED | Asserts all five tools via the stable inspection API. 2 tests pass. |
| `crates/pmcp-workbook-server/tests/http_smoke.rs` | Ephemeral-port HTTP `initialize` round-trip, response-id assertion | VERIFIED | Drives full MCP `initialize` over `StreamableHttpTransport`, asserts response id echoes request id. 1 test passes. |
| `crates/pmcp-workbook-server/tests/parity_workbook.rs` | Real `run_serving` path, live `tools/list` + `resources/list` + tool call | VERIFIED | Invokes `run_serving`, asserts all five tools + `workbook://render/` resource over wire, drives `get_manifest`. 1 test passes. |
| `crates/pmcp-workbook-server/tests/bundle_id_props.rs` | Proptest: every non-matching `bundle_id` → `RunError::BundleIdMismatch` fail-closed | VERIFIED | `non_matching_bundle_id_always_fails_closed` proptest with `prop_assume!(id != GOLDEN_BUNDLE_ID)`. Explicit `Some("tax-calc")` → Ok and `None` → Ok cases. 3 tests pass. |
| `Makefile` purity-check | Phase 95 block asserting `pmcp-workbook-server` served cone reader-free | VERIFIED | Lines 594–621 contain fail-closed `cargo tree -p pmcp-workbook-server` with `|| status=$?`, `BAN='umya|calamine|quick-xml|swc_|pmcp-code-mode'`. Comment documents broad BAN is intentional. `make purity-check` exits 0 with "pmcp-workbook-server reader-free" line. |
| `CLAUDE.md` | Slot 9a publish-order entry | VERIFIED | Line 233: `9a. pmcp-workbook-server` listed in Release & Publish Workflow. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `assemble.rs` | `pmcp_server_toolkit::workbook::try_with_workbook_bundle` | `Server::builder().try_with_workbook_bundle(&source)?` | WIRED | Confirmed at assemble.rs line 85. |
| `assemble.rs` | `pmcp_server_toolkit::workbook::load_bundle` | pre-load id check when `--bundle-id` is `Some` | WIRED | Confirmed at assemble.rs line 72: `load_bundle(&source).map_err(...)` |
| `Cargo.toml` | `pmcp-server-toolkit` with `features = ["workbook", "http"]` | no direct `pmcp-workbook-runtime` dep (D-11) | WIRED | `default-features = false, features = ["workbook", "http"]` — no runtime dep in `[dependencies]`. |
| `tests/parity_workbook.rs` | `pmcp_workbook_server::run_serving` | real binary path, ephemeral port, `handle.abort()` | WIRED | `run_serving(&args).await` at line 69; `handle.abort()` at line 142. |
| `Makefile purity-check` | pmcp-workbook-server served cone | `cargo tree -p pmcp-workbook-server`, fail-closed `|| status=$?` | WIRED | Lines 611–621 in Makefile. `make purity-check` exits 0. |

---

### Data-Flow Trace (Level 4)

The `pmcp-workbook-server` binary is a configuration-driven dispatcher, not a data-rendering component. Data flow is from the bundle files → `try_with_workbook_bundle` boot gate → registered tool handlers → MCP response wire. This is verified end-to-end by the parity test's live `get_manifest` call returning a non-error result, proving real bundle data flows through to the MCP client.

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|-------------------|--------|
| `parity_workbook.rs` | `manifest` (live tool call result) | `run_serving` → toolkit `get_manifest` handler → golden `tax-calc@1.1.0` bundle | Yes — `assert!(!manifest.is_error)` | FLOWING |
| `assemble.rs` `build_server` | `bundle.stamp.bundle_id` | `load_bundle(&source)` reads `BUNDLE.lock` from disk | Yes — the `matching_bundle_id_succeeds` test confirms the real id `"tax-calc"` is returned | FLOWING |

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Example builds server from golden bundle | `cargo run -p pmcp-workbook-server --example workbook_server_min` | exit 0, "calculate present: true" | PASS |
| Lib unit tests (12 inline tests) | `cargo test -p pmcp-workbook-server --lib` | 12 passed, 0 failed | PASS |
| Integration tests (all 4 files) | `cargo test -p pmcp-workbook-server --test assemble --test http_smoke --test parity_workbook --test bundle_id_props -- --test-threads=1` | 9 tests passed (2+1+1+3+proptest runs) | PASS |
| Purity gate | `make purity-check` | exit 0, "pmcp-workbook-server reader-free" | PASS |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|---------|
| WBCL-06 | 95-01-PLAN.md, 95-02-PLAN.md | Shape A pure-config workbook binary | SATISFIED | All three success criteria proven by tests + purity gate + slot-9a wiring |

---

### Anti-Patterns Found

Scanned all files modified in this phase.

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| None | — | — | No TBD/FIXME/XXX markers. No unconstructible variants. No placeholder returns. No hardcoded empty data passed to rendering paths. |

Specific checks confirmed:
- `grep -n "TBD\|FIXME\|XXX"` across all phase files: 0 matches.
- `RunError` has no `Io` variant (no unconstructible variant — Codex MEDIUM #5).
- No `return null`, `return {}`, `return []` in any source file.
- `env(...)` in `cli.rs` appears only in a comment (D-03 rationale), not in any `#[arg]` attribute — confirmed by inspecting all three `#[arg(long)]` and `#[arg(long, default_value = ...)]` attributes.

---

### Human Verification Required

None. All success criteria are verifiable programmatically:
- SC1: proven by running tests (live HTTP initialize + tools/list + tool call).
- SC2: proven by unit tests (BundleIdMismatch, invalid-http → Addr, panic → Serving, nonexistent-dir → Bundle).
- SC3: proven by `make purity-check` (cargo-tree negative assertion, exit 0).

---

### Gaps Summary

No gaps. All three phase success criteria are observably met in the codebase with passing tests and a passing purity gate.

---

_Verified: 2026-06-14_
_Verifier: Claude (gsd-verifier)_
