# Phase 95: Shape A Binary `pmcp-workbook-server` - Context

**Gathered:** 2026-06-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Deliver a new `pmcp-workbook-server` **pure-config binary crate** that stands up a
live MCP server from a compiled workbook bundle alone — **zero user Rust** — serving
the five workbook tools (`calculate` / `explain` / `get_manifest` / `diff_version` /
`render_workbook`) entirely from the bundle. It **mirrors `pmcp-sql-server`
field-for-field**: a testable library (`run` / `serve` / `run_serving`) plus a thin
`#[tokio::main]` `main.rs` shim, a typed `RunError` enum that maps every failure to a
**non-zero process exit**, and a clap `Args` surface that selects a `BundleSource`
from CLI arguments. The binary runs the existing fail-closed boot integrity gate
(`load_bundle` → `BUNDLE.lock` hash recomputation) and surfaces any load/integrity
failure as `RunError`.

**Requirements:** WBCL-06.

**Explicitly NOT in this phase:**
- The compiler / `umya` ingest / linter / CLI subcommands (Phases 93–94, already built).
- `cargo pmcp new --kind workbook-server` **scaffold** (Shape B), workbook-declared
  **dialect-version declaration**, and the **second-workbook generalization gate** —
  all Phase 96 (WBCL-05, WBDL-02, WBEX-01/02).
- Embedded (`include_dir` / `EmbeddedSource`) bundle support in this binary (D-02).
- Runtime `pmcp.toml` resolution (D-03).
- Does not touch `pmcp-code-mode`.

</domain>

<decisions>
## Implementation Decisions

### A. Bundle source & CLI shape (the novel seam vs sql's `dispatch`)
- **D-01 (`--bundle-dir` = exact bundle@version dir; `--bundle-id` asserts):**
  `--bundle-dir <dir>` points at the **exact `bundle@version` directory** and is
  handed straight to `LocalDirSource::new(dir)` (Phase 92 D-08: one source instance =
  one `bundle@version`; the **version is implicit in the path**). `--bundle-id <id>`
  is **asserted against the loaded `BUNDLE.lock` `bundle_id`** and **fails closed on
  mismatch** (a typed `RunError`, non-zero exit) — it is a guard that the operator is
  serving the bundle they think they are, not a resolution input. No separate
  `--bundle-version` flag.
- **D-02 (LocalDirSource only — `workbook` feature, NOT `workbook-embedded`):** The
  crate links `pmcp-server-toolkit` with the **`workbook`** feature (LocalDir-only, no
  `include_dir`). This is the pure "point it at a compiled bundle dir" story, mirrors
  `pmcp-sql-server`'s point-at-config posture, and satisfies **success criterion 3**
  ("the published binary links only `pmcp-server-toolkit[workbook]` +
  `pmcp-workbook-runtime`; the purity gate confirms no reader in its tree").
  `EmbeddedSource` / baked-in bundles stay the **Shape B scaffold's** concern
  (Phase 96).

### B. Config resolution
- **D-03 (pure CLI args — no runtime `pmcp.toml` read):** The binary takes only
  `--bundle-dir` (+ the `--bundle-id` assertion). The Phase-94 `pmcp.toml` stays a
  **build-time artifact** (consumed by `cargo pmcp workbook compile`) and is **never
  read by the served binary** — keeping the runtime crate uncoupled from the
  compile-time project layout, mirroring `pmcp-sql-server`'s single-`--config`-file
  model.

### C. Transport surface
- **D-04 (streamable-HTTP only; loopback default):** Mirror `pmcp-sql-server`
  exactly — **streamable-HTTP only**, `--http` defaulting to **`127.0.0.1:8080`**
  (loopback so the out-of-the-box binary exposes no public listener), served through
  the SDK's Phase 56 **Tower/axum adapter** (`StreamableHttpServer`) so DNS-rebinding,
  CORS, and security-header layers are applied by the SDK and never hand-rolled. **No
  stdio transport.** Matches Phase 92 D-12's remote business-user framing.

### D. Mandatory runnable example
- **D-05 (`--bundle-dir` at the committed synthetic golden):** The ALWAYS-required
  example invokes the binary path (`run` / `run_serving`) with `--bundle-dir` pointing
  at the **committed synthetic tax-calc golden fixture** (Phase 92/93 — **zero
  customer data / TowelRads material**, hard constraint) and serves over HTTP. It
  demonstrates the real end-to-end "point it at a compiled bundle and serve" deploy
  story, consistent with the LocalDirSource-only posture (D-02). Do NOT lean on
  Phase 92's `EmbeddedSource`-based `workbook_server_http` toolkit example as the
  Shape A demonstration.

### Claude's Discretion
- Crate/module file layout inside `crates/pmcp-workbook-server/src/` — but **mirror
  `pmcp-sql-server`'s split**: `lib.rs` (`run`/`serve`/`run_serving` + `RunError`),
  `cli.rs` (`Args`), `main.rs` (thin shim), and an `assemble`-equivalent seam that
  turns (`LocalDirSource`, `--bundle-id` assertion) → a built `pmcp::Server` via the
  toolkit's `WorkbookBuilderExt::try_with_workbook_bundle`.
- Exact `RunError` variant set (model it on `pmcp-sql-server`'s: `Io`, a
  bundle-load/integrity variant wrapping `BundleLoadError`/`BundleSourceError`, an
  id-mismatch variant for D-01's assertion, `Addr`, `Serve`, `Serving`).
- Crate metadata: `version` (new crate — start at `0.1.0`), `description`, keywords,
  `exclude` of test fixtures from the published tarball (mirror `pmcp-sql-server`'s
  `exclude = [".planning/", ".pmat/", "fuzz/", "tests/"]`), `docs.rs` config.
- Where the synthetic golden fixture the example/tests point at lives relative to the
  new crate (reuse the existing committed golden rather than regenerating a new one).
- Publish-slot wiring detail ("slot 9a") in CLAUDE.md's release order — confirm the
  new crate's position after `pmcp-server-toolkit` + `pmcp-workbook-runtime`.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase contract
- `.planning/ROADMAP.md` — Phase 95 entry (goal + 3 success criteria + WBCL-06);
  Phase 96 entry (the Shape B / dialect-version / generalization-gate scope boundary
  this phase must NOT cross)
- `.planning/REQUIREMENTS.md` — WBCL-06 verbatim + the "Workbook CLL & Developer
  Experience" traceability block

### Prior-phase decisions carried forward
- `.planning/phases/92-bundlesource-served-tool-toolkit-module/92-CONTEXT.md` —
  **the most load-bearing prior context.** D-06/D-07 (`BundleSource` is sync, lives in
  `pmcp-workbook-runtime`), D-08 (one source = one `bundle@version`), D-09
  (`with_workbook_bundle` extension-method registration), D-10/D-11 (`workbook` vs
  `workbook-embedded` feature split; toolkit re-exports the boot surface so consumers
  never name the runtime crate), D-12 (streamable-HTTP, remote business-user story),
  D-15 (provenance stamp = bundle_id + version + `BUNDLE.lock` hash), D-17
  (`bundle_id` field name)
- `.planning/phases/94-cli-subcommands-pmcp-toml/94-CONTEXT.md` — the `pmcp.toml`
  shape this binary deliberately does NOT read at runtime (D-03); confirms `pmcp.toml`
  is a build-time/compile concern

### Shape A field-for-field mirror (in-repo — the precedent to clone)
- `crates/pmcp-sql-server/src/lib.rs` — `run` / `serve` / `run_serving` /
  `load_config_and_schema` split + the `RunError` enum (Io/Config/Dispatch/Assemble/
  Addr/Serve/Serving) + the `handle.await.map_err(RunError::Serving)` crash-surfacing
  pattern (threat T-85-10-02). **The structural template for `pmcp-workbook-server`.**
- `crates/pmcp-sql-server/src/cli.rs` — the clap `Args` surface (required path args +
  `--http` loopback default) to mirror for `--bundle-dir` / `--bundle-id` / `--http`
- `crates/pmcp-sql-server/src/main.rs` — the thin `#[tokio::main]` shim that returns
  `run()`'s `Result` (non-zero exit on `RunError`)
- `crates/pmcp-sql-server/src/assemble.rs` — config → built `pmcp::Server` seam
  (the workbook analog assembles via `WorkbookBuilderExt::try_with_workbook_bundle`)
- `crates/pmcp-sql-server/Cargo.toml` — crate metadata, `exclude`, feature posture,
  dev-dep `mcp-tester` parity-test pattern to mirror
- `crates/pmcp-sql-server/tests/` — the dispatch/assemble/HTTP-smoke/parity test
  shapes to mirror (drive `run_serving` to an ephemeral port, `abort()` the handle)

### Toolkit + runtime surface this binary wires over (in-repo)
- `crates/pmcp-server-toolkit/src/workbook/mod.rs` — the served module's public API:
  `WorkbookBuilderExt` (incl. `try_with_workbook_bundle(builder, &dyn BundleSource)`),
  `ProvStamp`, and the re-exports of `load_bundle`, `BundleSource`, `LocalDirSource`,
  `BundleLoadError`, `BundleSourceError` (the boot surface — depend only on the
  toolkit, never name `pmcp-workbook-runtime` in the binary's Cargo.toml per D-11)
- `crates/pmcp-server-toolkit/src/lib.rs` — top-level re-exports + the
  `try_with_workbook_bundle` signature reference (`lib.rs:300`); the `workbook` vs
  `workbook-embedded` feature definitions
- `crates/pmcp-server-toolkit/Cargo.toml` — `workbook` / `workbook-embedded` feature
  matrix (lines ~146-147) and the existing `workbook_server_http` example (D-05's
  contrast — do not reuse as the Shape A demo)
- `crates/pmcp-workbook-runtime/src/bundle_source.rs` — `BundleSource` trait,
  `LocalDirSource::new` (line ~112), `EmbeddedSource` (out of scope here)
- `crates/pmcp-workbook-runtime/src/bundle_loader.rs` — `load`/`load_bundle`
  fail-closed boot gate (`BUNDLE.lock` recomputation) the binary relies on

### Published contracts / purity gate
- `docs/workbook-dialect-spec.md` + the `workbook://` URI contract doc — the served
  contract the binary honors
- `Makefile`/`justfile` `purity-check` + `.github/workflows` purity job — must confirm
  the new `pmcp-workbook-server` served tree (binary → toolkit[workbook] → runtime →
  pmcp) stays reader-free (no `umya`/`quick-xml`/`zip`) per success criterion 3
- `CLAUDE.md` ## Release & Publish Workflow — the crate-publish-order list the new
  binary's slot ("9a") attaches to (after items 5–8 / runtime + toolkit)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`pmcp-sql-server` (Phase 85)** — the complete Shape A skeleton: lib/bin split,
  `RunError`→exit discipline, clap `Args`, Tower/axum HTTP serve, ephemeral-port
  integration tests, `mcp-tester` parity harness. `pmcp-workbook-server` is a
  near-mechanical re-skin of it with the connector-dispatch seam replaced by a
  `LocalDirSource` + `--bundle-id` assertion seam.
- **`pmcp-server-toolkit` `workbook` module (Phase 92)** — already registers all five
  tools + the `workbook://` resource via `WorkbookBuilderExt::try_with_workbook_bundle`
  and runs the fail-closed boot gate. The binary supplies a `LocalDirSource` and calls
  this; it adds **no served logic**.
- **Toolkit `http` feature** — already forwards `pmcp/streamable-http`; the
  `StreamableHttpServer` adapter (as used by `pmcp-sql-server::serve`) needs no new
  transport plumbing.

### Established Patterns
- **lib (`run`/`serve`/`run_serving`) + thin `main.rs` shim** — the testable-seam
  split locked by `pmcp-sql-server`.
- **`RunError` (thiserror, `#[non_exhaustive]`) → non-zero exit**, with a `Serving`
  variant propagating serve-task `JoinError` so a crashed listener exits non-zero.
- **Fail-closed boot integrity** — `load_bundle` recomputes `BUNDLE.lock`; the binary
  maps failure to `RunError`, never serves a tampered/incomplete bundle.
- **Purity gate per feature-combination** — the new served cone must be added to /
  confirmed by the existing purity job.

### Integration Points
- New crate `crates/pmcp-workbook-server/` → depends on `pmcp` (streamable-http) +
  `pmcp-server-toolkit` (feature `workbook` + `http`). Served cone:
  binary → toolkit[workbook] → `pmcp-workbook-runtime` → pmcp (reader-free).
- Publish order: after `pmcp-server-toolkit` (5) and `pmcp-workbook-runtime` — the
  "slot 9a" the roadmap names; add to CLAUDE.md's publish-order list.
- The binary names ONLY `pmcp-server-toolkit` for the bundle surface (D-11) — it must
  NOT add `pmcp-workbook-runtime` directly to its `[dependencies]` for the
  `BundleSource`/`LocalDirSource` types (they're re-exported through the toolkit).

</code_context>

<specifics>
## Specific Ideas

- **Field-for-field with `pmcp-sql-server`** is the user's explicit framing — the
  goal sentence names it. Deviations from that precedent should be justified by a
  workbook-specific need (the bundle-source seam, the five fixed tools), not by
  preference.
- **Remote business-user deploy story** (Phase 92 D-12, carried): the binary + example
  read like "a compiled workbook bundle, served over HTTP for remote callers", not a
  developer-local toy.
- **Zero customer material** (Phase 92 D-01, hard constraint): the example/tests use
  ONLY the synthetic tax-calc golden — no TowelRads data, logic, or identifiers
  anywhere in code, comments, fixtures, or docs.

</specifics>

<deferred>
## Deferred Ideas

- **Embedded (`include_dir` / `EmbeddedSource`) bundle support** — deferred to the
  Shape B scaffold (Phase 96), which the goal says uses `EmbeddedSource` in the
  scaffolded `main.rs`. The `workbook-embedded` feature exists but this binary does
  not enable it (D-02).
- **Runtime `pmcp.toml` resolution** (`--bundle-id` → dir/version) — rejected for this
  phase (D-03); revisit only if operators find naming bundles instead of paths
  compelling and the runtime-coupling cost is acceptable.
- **stdio transport** — rejected (D-04, HTTP-only); revisit if a local Claude Desktop
  testing story is demanded.
- **`--bundle-dir` as a parent dir + `--bundle-version` selection** — rejected (D-01,
  exact-dir model); revisit with Phase 96 multi-workbook/version-evolution work if
  needed.
- **Dialect-version in the provenance stamp** — Phase 96 (WBDL-02), per Phase 92 D-15.

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 95-shape-a-binary-pmcp-workbook-server*
*Context gathered: 2026-06-14*
