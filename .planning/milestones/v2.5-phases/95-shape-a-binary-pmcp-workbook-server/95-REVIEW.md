---
phase: 95-shape-a-binary-pmcp-workbook-server
reviewed: 2026-06-14T00:00:00Z
depth: standard
files_reviewed: 13
files_reviewed_list:
  - CLAUDE.md
  - Cargo.toml
  - Makefile
  - crates/pmcp-workbook-server/Cargo.toml
  - crates/pmcp-workbook-server/examples/workbook_server_min.rs
  - crates/pmcp-workbook-server/src/assemble.rs
  - crates/pmcp-workbook-server/src/cli.rs
  - crates/pmcp-workbook-server/src/lib.rs
  - crates/pmcp-workbook-server/src/main.rs
  - crates/pmcp-workbook-server/tests/assemble.rs
  - crates/pmcp-workbook-server/tests/bundle_id_props.rs
  - crates/pmcp-workbook-server/tests/http_smoke.rs
  - crates/pmcp-workbook-server/tests/parity_workbook.rs
findings:
  critical: 0
  warning: 1
  info: 2
  total: 3
status: issues_found
---

# Phase 95: Code Review Report

**Reviewed:** 2026-06-14
**Depth:** standard
**Files Reviewed:** 13
**Status:** issues_found

## Summary

`pmcp-workbook-server` is a faithful field-for-field re-skin of `pmcp-sql-server`
into a Shape A pure-config workbook MCP server binary. I adversarially verified
every load-bearing claim in the source against the real toolkit/runtime APIs
rather than trusting the (extensive) comments:

- The `pmcp-server-toolkit` `workbook` + `http` features both exist and gate the
  used surface (`crates/pmcp-server-toolkit/Cargo.toml:117,146`,
  `src/lib.rs:52,156`).
- `LocalDirSource`, `load_bundle`, and `WorkbookBuilderExt` are genuinely
  re-exported through `pmcp_server_toolkit::workbook`
  (`src/workbook/mod.rs:49-73`); `load_bundle(&source)` correctly coerces
  `LocalDirSource` to `&dyn BundleSource` (`bundle_loader.rs:268`).
- `bundle.stamp.bundle_id` is a valid field path (`bundle_loader.rs:97`,
  `artifact_model.rs:69`); `ToolkitError::Workbook(#[from] BundleLoadError)`
  exists and is `workbook`-gated, so `e.into()` in `assemble.rs:72` compiles
  (`error.rs:86-88`).
- All five tool NAME constants (`calculate`/`explain`/`get_manifest`/
  `diff_version`/`render_workbook`) and the `workbook://render/` resource URI the
  tests assert against match the toolkit exactly (`handler.rs:151..540`,
  `render_uri.rs:49`).
- The `StreamableHttpServerConfig::default()` security posture the docs claim
  (`allowed_origins: None` → runtime `AllowedOrigins::localhost()`) is real and
  fail-closed (`src/server/streamable_http_server.rs:230,305`). Binding
  `0.0.0.0` does NOT loosen CORS — good.
- The Phase 95 `make purity-check` block passes: `cargo tree -p
  pmcp-workbook-server` (verified empirically, including dev-deps) contains none
  of `umya|calamine|quick-xml|swc_|pmcp-code-mode`. The `default-features =
  false` on the toolkit dep is doing real work — the served cone stays
  code-mode-free.
- `cargo clippy -p pmcp-workbook-server --all-targets` is clean for this crate
  (the one `redundant guard` warning emitted is in `pmcp-server-toolkit`, a
  dependency, and is out of scope + pre-existing). Doctests (4) and all test
  targets compile.

One genuine quality defect: the `RunError` re-skin collapsed the reference's
distinct `Build` error variant into the transport-start `Serve` variant, so a
server-build failure is reported with a transport error message. Two minor INFO
items round it out. No security or correctness blockers.

## Warnings

### WR-01: `RunError::Serve` is overloaded — a `Server::build()` failure is reported as a transport-start error

**File:** `crates/pmcp-workbook-server/src/assemble.rs:87` (and the variant at `crates/pmcp-workbook-server/src/lib.rs:83-85`)

**Issue:** `build_server` maps the final builder failure with
`.build().map_err(RunError::Serve)`. But `RunError::Serve` is declared as the
*transport* error:

```rust
/// Binding / starting the streamable-HTTP listener failed.
#[error("streamable-HTTP server failed to start: {0}")]
Serve(#[source] pmcp::Error),
```

So a `Server::builder().build()` failure (an in-process assembly error that
happens BEFORE any listener is touched) is surfaced to the operator as
`"streamable-HTTP server failed to start: ..."` — a factually wrong diagnostic
that will send an operator chasing port/bind problems for what is actually a
server-construction fault.

This is also an internal documentation contradiction the re-skin introduced: the
`build_server` rustdoc at `assemble.rs:49` says *"`RunError::Serve` when the
final `pmcp::Server` build fails"*, while the variant's own doc at `lib.rs:84`
says the opposite (*"streamable-HTTP listener failed"*). The same variant cannot
honestly mean both.

The reference crate does NOT do this — `pmcp-sql-server` carries a *distinct*
variant for builder failures (`crates/pmcp-sql-server/src/assemble.rs:79-82`):

```rust
/// The final `pmcp::Server::builder().build()` failed (e.g. an internal ...)
Build(#[from] pmcp::Error),
```

So this is a fidelity regression in the "field-for-field re-skin," not an
inherited trait.

**Fix:** Restore the distinct build-failure variant so each error names its real
phase:

```rust
// lib.rs — add to RunError:
/// The final `pmcp::Server::builder().build()` failed (in-process assembly,
/// before any listener is bound).
#[error("workbook server build failed: {0}")]
Build(#[source] pmcp::Error),

// assemble.rs:87 — map the builder failure to the build variant:
        .build()
        .map_err(RunError::Build)?;
```

(Leave `serve()`'s `http.start().await.map_err(RunError::Serve)` as-is — that one
genuinely is the transport-start path.)

## Info

### IN-01: Example uses `#[tokio::main]` but never awaits — no async runtime is needed

**File:** `crates/pmcp-workbook-server/examples/workbook_server_min.rs:21-44`

**Issue:** `main` is `#[tokio::main] async fn` but the body only calls the
*synchronous* `build_server(&args)?` and prints — there is no `.await` anywhere
(the trailing comment even notes a real binary "would `run_serving(&args).await`",
which this example deliberately does not). Spinning up a multi-thread Tokio
runtime to run zero async work is dead ceremony and slightly misleads a reader
into thinking the build path is async.

**Fix:** Drop the attribute and make it a plain `fn main`:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ... unchanged body (build_server is synchronous) ...
}
```

This also removes the example's implicit dependency on the `tokio` macros
feature for a non-async demonstration.

### IN-02: CLAUDE.md publish-order entry depends on `pmcp-workbook-runtime`, which is not a numbered item in the list

**File:** `CLAUDE.md:233` (the new `9a.` entry)

**Issue:** The added `9a.` entry states `pmcp-workbook-server` "Must publish
AFTER `pmcp-server-toolkit` (item 5) and its `pmcp-workbook-runtime` dep." But
`pmcp-workbook-runtime` (a genuine transitive runtime dependency of the served
binary — confirmed via `cargo tree`) appears nowhere in the numbered publish
order (items 1-12). A reader following the list to drive a release has no
ordering slot for the runtime crate, so the stated "AFTER ... its
`pmcp-workbook-runtime` dep" precondition is unverifiable from the list itself.

This is a release-documentation gap, not a code defect — and the release
workflow skips already-published crates gracefully — but it should be reconciled
so the publish-order list is self-consistent (either add the runtime as a
numbered item or note explicitly that it is published out-of-band by Phase
91/92's own release).

**Fix:** Add `pmcp-workbook-runtime` (and any other workbook-runtime-tree crates)
as numbered publish-order items ahead of `pmcp-server-toolkit`, or append a note
to `9a.` clarifying where `pmcp-workbook-runtime` is published in the ordering.

---

_Reviewed: 2026-06-14_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
