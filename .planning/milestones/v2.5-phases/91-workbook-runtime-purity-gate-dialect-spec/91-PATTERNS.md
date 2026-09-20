# Phase 91: Workbook Runtime + Purity Gate + Dialect Spec - Pattern Map

**Mapped:** 2026-06-09
**Files analyzed:** 12 (2 new crates × {Cargo.toml + src} + spec doc + 3 build/CI integration points)
**Analogs found:** 12 / 12 (every file has BOTH a lighthouse lift-source AND an in-repo convention analog)

> **This is a TWO-SIDED LIFT.** Every new file has two reference points:
> 1. **Lighthouse lift-source** — what the code is copied FROM (lives at
>    `/Users/guy/Development/AI-on-Cloud/projects/towelrads/mcp-servers/quote-pricing/`).
> 2. **In-repo convention analog** — what SDK conventions the lifted code must
>    CONFORM TO (this repo). Concrete excerpts from the in-repo analogs are below so
>    the planner matches SDK conventions, not just copies lighthouse code blindly.
>
> The deliberate deltas from a verbatim lift (per CONTEXT.md): (1) split a
> `pmcp-workbook-dialect` crate out of the compiler `dialect/` (D-01), (2) flat
> 13-fn whitelist (D-05), (3) add `Deserialize` to the finding types (D-08),
> (4) rename crate/lib identifiers, (5) `version.workspace`→literal `0.1.0`,
> (6) `thiserror` 1→2, (7) `*.workspace = true` deps→literal version strings.

## File Classification

| New/Modified File | Role | Data Flow | Lighthouse Lift-Source | In-Repo Convention Analog | Match Quality |
|-------------------|------|-----------|------------------------|---------------------------|---------------|
| `crates/pmcp-workbook-runtime/Cargo.toml` | config (manifest) | n/a | `crates/workbook-runtime/Cargo.toml` | `crates/pmcp-server-toolkit/Cargo.toml` | exact (both sides) |
| `crates/pmcp-workbook-runtime/src/lib.rs` | model/library root | transform | `crates/workbook-runtime/src/lib.rs` | (lift verbatim; rename only) | exact lift |
| `crates/pmcp-workbook-runtime/src/finding.rs` | model | request-response (serde round-trip) | `crates/workbook-runtime/src/finding.rs` | `pmcp` serde-derive convention (root `Cargo.toml`) | exact lift + D-08 delta |
| `crates/pmcp-workbook-runtime/src/{manifest_model,artifact_model,changelog,formula,dag,resolve,scalar_eval,excel_error,range_ref}.rs` | model/utility | transform/CRUD | `crates/workbook-runtime/src/<same>.rs` | (lift verbatim) | exact lift |
| `crates/pmcp-workbook-runtime/src/sheet_ir/` | service (executor) | event-driven (topo) | `crates/workbook-runtime/src/sheet_ir/` | (lift verbatim) | exact lift |
| `crates/pmcp-workbook-runtime/src/render/` | service (writer) | file-I/O (writer-only) | `crates/workbook-runtime/src/render/` | (lift verbatim) | exact lift |
| `crates/pmcp-workbook-dialect/Cargo.toml` | config (manifest) | n/a | (NEW — split from compiler) | `crates/pmcp-server-toolkit/Cargo.toml` + sql-server path-dep shape | role-match |
| `crates/pmcp-workbook-dialect/src/lib.rs` | model (contract const) + test | request-response | `crates/workbook-compiler/src/dialect/rules.rs` + `dialect/mod.rs` binding test | `pmcp-workbook-runtime/src/lib.rs` re-export surface | exact lift + D-01/D-05 deltas |
| `docs/workbook-dialect-spec.md` | doc (published contract) | n/a | `docs/workbook-dialect-spec.md` (lighthouse) | repo `docs/` (root-level docs convention) | exact lift + D-05 flatten |
| `Cargo.toml` (root `[workspace] members`) | config (workspace) | n/a | (lighthouse workspace members) | root `Cargo.toml:541` members array | exact (in-repo) |
| `Makefile` (`purity-check` target) | config (build) | batch | `justfile` recipe `purity-check` (lighthouse:54-92) | `Makefile:460-480` `quality-gate` target | role-match |
| `.github/workflows/ci.yml` (`purity-check` job + `gate` wire) | config (CI) | batch | (lighthouse ran it via `just`, no CI job) | `ci.yml:160` `quality-gate` job + `:281` `gate` job | role-match |

---

## Pattern Assignments

### `crates/pmcp-workbook-runtime/Cargo.toml` (config, manifest)

**Lighthouse lift-source:** `crates/workbook-runtime/Cargo.toml` (the dep list + the `rust_xlsxwriter` provenance comment).

**In-repo convention analog:** `crates/pmcp-server-toolkit/Cargo.toml` (lines 1-29). This is the SDK shape the lift must conform to. The lighthouse uses `version.workspace = true` and `serde = { workspace = true }`; the SDK does **NOT** — it uses literal versions everywhere.

**SDK package-header convention** (from `pmcp-server-toolkit/Cargo.toml:1-16` and `pmcp-sql-server/Cargo.toml:1-17`):
```toml
[package]
name = "pmcp-workbook-runtime"
version = "0.1.0"                       # literal — NOT version.workspace (Pitfall 2)
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/paiml/rust-mcp-sdk"
description = "Reader-free workbook IR + deterministic executor + writer-only .xlsx renderer"
# Why: keep the published artifact lean and reader-free; planning/pmat/fuzz never ship.
exclude = [".planning/", ".pmat/", "fuzz/", "tests/"]

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[lib]
name = "pmcp_workbook_runtime"          # renamed from lighthouse `workbook_runtime`
path = "src/lib.rs"
```

**SDK dependency convention** — literal version strings, NOT `{ workspace = true }`. The lighthouse `[dependencies]` block (`serde = { workspace = true }`, `thiserror = { workspace = true }`) must be rewritten to the SDK literal form. Match the existing pins seen across `pmcp-server-toolkit/Cargo.toml:25-29` (`serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `thiserror = "2"`) and root `Cargo.toml:52` (`schemars = { version = "1.0", optional = true }`):
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "1.0"
thiserror = "2"                         # bumped from lighthouse 1.0.69 (Pitfall 3)
sha2 = "0.11"
hex = "0.4"
rust_xlsxwriter = { version = "0.95", default-features = false }  # WRITER-ONLY
```

**Critical deltas (lighthouse → SDK):**
- `version.workspace = true` → `version = "0.1.0"` (Pitfall 2 — the SDK has no `[workspace.package] version`; `version.workspace` errors or wrongly pins to `pmcp` 2.9.0).
- `serde = { workspace = true }` etc. → literal `version = "..."` strings (the SDK has no `[workspace.dependencies]` table for these).
- `thiserror = { workspace = true }` (resolves to `1.0.69`) → `thiserror = "2"` (Pitfall 3; matches `pmcp-server-toolkit:29` and root `:55`).
- Lib name `workbook_runtime` → `pmcp_workbook_runtime`.
- Do **NOT** add a `pmcp` dependency (research A1 / Anti-Pattern: D-09 permits but runtime has zero functional need).
- **Preserve verbatim** the `rust_xlsxwriter` provenance comment block (lighthouse Cargo.toml:34-42) — it documents the writer-only / human-verify gate (T-12-SC). Research recommends a `checkpoint:human-verify` re-confirming author `jmcnamara` + `cargo audit` clean.

---

### `crates/pmcp-workbook-runtime/src/lib.rs` (model/library root, transform)

**Lighthouse lift-source:** `crates/workbook-runtime/src/lib.rs` (re-export surface + crate-level lints). **Lift VERBATIM**, changing only the crate-name in doc-comments.

**Crate-level panic-freedom lints (lift exactly — D-10):**
```rust
// Source: lighthouse crates/workbook-runtime/src/lib.rs:18-19
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
```

**Module re-export surface** (lighthouse `lib.rs:21-58`): `pub mod excel_error; pub mod range_ref; pub mod formula; pub mod finding; pub mod dag; pub mod resolve; pub mod manifest_model; pub mod artifact_model; pub mod scalar_eval; pub mod sheet_ir; pub mod render; pub mod changelog;` — lift the whole module tree.

**In-repo convention analog:** none needed beyond the rename — the SDK has no competing convention for module layout; the lift is verbatim. The lib-name reference (`workbook_runtime` → `pmcp_workbook_runtime`) is the only mechanical change.

---

### `crates/pmcp-workbook-runtime/src/finding.rs` (model, request-response) — THE ONE CODE DELTA (D-08)

**Lighthouse lift-source:** `crates/workbook-runtime/src/finding.rs` (full file, 187 LOC). Lift verbatim, then add `Deserialize`.

**In-repo convention analog:** the `pmcp` serde-derive house style — `#[serde(rename_all = "...")]` on enums + `Deserialize, Serialize` round-trip (root `Cargo.toml:50` pins `serde` with `derive`; this is the universal SDK pattern). The lighthouse already matches it for `Serialize`; D-08 adds the missing `Deserialize`.

**The exact derive change** (lighthouse `finding.rs:21,25,41,83` — add `Deserialize`):
```rust
// lighthouse line 21:  use serde::Serialize;
// CHANGE TO:
use serde::{Deserialize, Serialize};

// lighthouse line 25 (Severity) — ADD Deserialize:
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity { Error, Warning, Info }

// lighthouse line 41 (LintFinding) — ADD Deserialize:
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LintFinding {
    pub severity: Severity,
    pub rule: String,          // KEEP String — extensible (D-08), do NOT make it an enum
    pub sheet: String,
    pub cell: Option<String>,
    pub message: String,
    pub repair: String,
}

// lighthouse line 83 (LintReport) — ADD Deserialize:
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LintReport { pub findings: Vec<LintFinding> }
```

**Preserve verbatim:** the three existing tests (`has_errors_gates_on_error_severity_only`, `has_errors_false_when_only_warnings_and_info`, `lint_finding_serializes_with_repair_field`, lighthouse `finding.rs:113-186`), the `LintFinding::new` constructor, `LintReport::{new,push,extend,has_errors}`, and the module doc-comment (update the "These three derive `serde::Serialize`..." line at `finding.rs:17-19` to mention `Deserialize`).

**ADD (Wave-0 gap):** a round-trip test alongside the existing serialize test:
```rust
#[test]
fn lint_report_round_trips_through_json() {
    let mut report = LintReport::new();
    report.push(LintFinding::new(Severity::Error, "whitelist/unsupported-fn",
        "1_Inputs", Some("A2".into()), "msg", "repair"));
    let back: LintReport = serde_json::from_value(serde_json::to_value(&report).unwrap()).unwrap();
    assert_eq!(back.findings.len(), 1);
    assert!(back.has_errors());
}
```

---

### `crates/pmcp-workbook-runtime/src/{manifest_model,artifact_model,changelog,formula,dag,resolve,scalar_eval,excel_error,range_ref}.rs` (model/utility, transform)

**Lighthouse lift-source:** the identically-named files under `crates/workbook-runtime/src/`. **Lift VERBATIM** — no SDK-convention reshaping needed (research: "serde `rename_all`/`schemars::JsonSchema` discipline matches lighthouse models — no reshaping needed beyond D-08's `Deserialize` add").

**In-repo convention analog:** none required — these are self-contained owned types. Do NOT introduce `petgraph` for `dag.rs` (Anti-Pattern; lift the 214-LOC owned `Dag` + Kahn `toposort`). Do NOT hand-roll hashing in `artifact_model.rs` — lift `build_bundle_lock`/`sha256_hex`/`update_field` verbatim (they use `sha2 0.11` + `hex 0.4`, matching the `pmcp-code-mode` pin).

**Only mechanical change:** if any file `use`s the crate by its old name (`workbook_runtime::`), rewrite to `pmcp_workbook_runtime::` (most use `crate::`, so likely zero edits).

---

### `crates/pmcp-workbook-runtime/src/sheet_ir/` (service/executor, event-driven)

**Lighthouse lift-source:** `crates/workbook-runtime/src/sheet_ir/{mod,executor,semantics,eval_bridge,eval_value,value,rounding}.rs`. Lift verbatim.

**Core pattern (WBRT-02)** — the topo executor returns a finding on a cycle (this is WHY `finding.rs` lives in the runtime, D-03):
```rust
// Source: lighthouse crates/workbook-runtime/src/sheet_ir/{mod,executor}.rs
pub use executor::{build_dag, run, EvalTrace, RunResult};
// run() walks Cells in Kahn topo order; on a dependency cycle it returns a
// Box<LintFinding> (so finding.rs must sit on the umya-free runtime side — D-03).
```
`semantics.rs` implements all 13 whitelisted functions; `rounding.rs` carries `excel_round`/`excel_ceiling`/`excel_floor` (round-half-away-from-zero, NOT banker's rounding — do not replace with `f64::round`). Lift the per-module unit tests intact.

---

### `crates/pmcp-workbook-runtime/src/render/` (service/writer, file-I/O writer-only)

**Lighthouse lift-source:** `crates/workbook-runtime/src/render/{mod,layout.rs}`. Lift verbatim.

**Core pattern (WBRT-03)** — deterministic, in-memory, writer-only:
```rust
// Source: lighthouse crates/workbook-runtime/src/render/mod.rs:38
use rust_xlsxwriter::{Color, DocProperties, ExcelDateTime, Format, Formula, Workbook};
// render_xlsx replays a LayoutDescriptor + injects computed values → DETERMINISTIC
// .xlsx bytes IN MEMORY (no filesystem — Lambda-safe). Pins doc properties to a
// FIXED creation datetime + empty author so two renders are byte-identical.
fn writer_err(e: rust_xlsxwriter::XlsxError) -> RenderError { RenderError::Writer(e.to_string()) }
```
`RenderError` uses `#[derive(thiserror::Error)]` — verify it compiles clean under `thiserror = "2"` (Pitfall 3; the simple `#[error("...")]` form is source-compatible across the major). Lift the byte-equality determinism test.

---

### `crates/pmcp-workbook-dialect/Cargo.toml` (config, manifest) — NEW (D-01 split)

**Lighthouse lift-source:** none direct — this crate does **not** exist in the lighthouse (the contract lives inside `workbook-compiler/src/dialect/`). D-01 promotes the contract half to its own crate.

**In-repo convention analog (path-dep shape):** `crates/pmcp-sql-server/Cargo.toml:32-36` — how an SDK crate declares a `version + path` workspace-internal dependency:
```toml
# Source: crates/pmcp-sql-server/Cargo.toml:33 (path-dep convention)
pmcp-server-toolkit = { version = "0.1.0", path = "../pmcp-server-toolkit", features = ["code-mode", "sqlite"] }
```

**Resulting Cargo.toml** (header convention from `pmcp-server-toolkit:1-5`; dep convention from `pmcp-sql-server:33`):
```toml
[package]
name = "pmcp-workbook-dialect"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/paiml/rust-mcp-sdk"
description = "Versioned workbook dialect contract — function whitelist, refuse-set, colour ontology + spec-doc binding test"

[lib]
name = "pmcp_workbook_dialect"
path = "src/lib.rs"

[dependencies]
pmcp-workbook-runtime = { version = "0.1.0", path = "../pmcp-workbook-runtime" }
# NOTE: lighthouse rules.rs has NO serde derives — add `serde` ONLY if DialectRules
# ever needs serialization (research §Supporting: currently it does not).
```

---

### `crates/pmcp-workbook-dialect/src/lib.rs` (model contract + binding test, request-response)

**Lighthouse lift-source (TWO files merge into one):**
1. `crates/workbook-compiler/src/dialect/rules.rs` (full file, 198 LOC) — `WHITELIST` const + `DialectRules` + `CandidateRole` + the colour-ontology ARGB constants. Lift verbatim **except** the D-05 flatten (below).
2. `crates/workbook-compiler/src/dialect/mod.rs:21-117` — the `#[cfg(test)] mod dialect_spec` binding test (`doc_whitelist_table_matches_const` + `parse_doc_whitelist` + `first_backtick_token`). Lift the test mechanism as-is.

**In-repo convention analog (re-export surface):** `crates/pmcp-workbook-runtime/src/lib.rs` (this phase's own runtime). The dialect crate's `lib.rs` should re-export the finding types it layers on top (D-03), mirroring how the lighthouse `dialect/mod.rs:11` did `pub use finding::{LintFinding, LintReport, Severity};` — but now sourcing them from the runtime crate:
```rust
// re-export the runtime's finding types (D-03) so dialect consumers get them here
pub use pmcp_workbook_runtime::finding::{LintFinding, LintReport, Severity};
pub use rules::{CandidateRole, DialectRules, WHITELIST};
```
Do NOT re-export `linter`/`lint` — that stays in Phase 93 (D-02; lighthouse `dialect/mod.rs:16-18` is OUT of scope here).

**The WHITELIST const (D-05 flatten — lift `rules.rs:26-31`, drop the two-tier comment framing):**
```rust
// Source: lighthouse crates/workbook-compiler/src/dialect/rules.rs:26-31
// D-05: drop the "8 core + 5 D-09 widened" comment split; 13 flat first-class names.
pub const WHITELIST: &[&str] = &[
    "IF", "VLOOKUP", "INDEX", "MATCH", "SUMIF", "SUM", "ROUNDUP", "CEILING",
    "IFERROR", "ISNUMBER", "SEARCH", "ROUND", "TEXT",
];
```

**The binding test (lift `dialect/mod.rs:29-117`)** — the SPEC_PATH relative depth is IDENTICAL (lighthouse `crates/workbook-compiler` → `../../docs`; SDK `crates/pmcp-workbook-dialect` → `../../docs`, both two segments up):
```rust
// Source: lighthouse crates/workbook-compiler/src/dialect/mod.rs:34
const SPEC_PATH: &str = "../../docs/workbook-dialect-spec.md"; // relative to CARGO_MANIFEST_DIR — depth UNCHANGED
```

**Two required D-05 adaptations to the binding test (Pitfall 4):**
1. The parser keys on `cols[1].trim() == "core" || category.contains("D-09 widened")` (lighthouse `mod.rs:57`). Under the flat-13 doc table, EITHER relabel all 13 rows to a single category (e.g. `whitelist`) and change the predicate to `category == "whitelist"`, OR key on a table-header marker. **Keep the `!doc_set.is_empty()` guard** (lighthouse `mod.rs:87`) — it catches a silent empty-parse if the table format drifts.
2. The "belt-and-braces" 5-name loop (lighthouse `mod.rs:101-106`) asserts only the D-09 names. Under D-05, either assert all 13 or drop the loop.

**Also flatten** the `rules.rs` test `whitelist_is_exactly_the_thirteen_names` (lighthouse `rules.rs:147-161`) — keep the 13-name assertion; the D-09 sub-loop is now redundant (all 13 are first-class).

---

### `docs/workbook-dialect-spec.md` (doc, published contract)

**Lighthouse lift-source:** `docs/workbook-dialect-spec.md` (155 LOC) — port verbatim, then flatten the whitelist table for D-05.

**In-repo convention analog:** repo root `docs/` (the SDK keeps published docs at repo-root `docs/`, and root `Cargo.toml:15` excludes `docs/` from the `pmcp` crate tarball — consistent with a BA/auditor-facing moat doc that ships in-repo but not in the crate).

**D-05 flatten (lighthouse spec table at lines 49-70):** the table column-2 currently carries `core` / `**D-09 widened**` category labels (lines 56-68) and a summary line "Total: **13 names** (8 core + 5 widened)" (line 70). Under D-05 these become a single flat category. The exact relabel is a planner call (Pitfall 4 / research A3) — but it MUST stay parseable by the binding test (keep a category column the parser keys on, OR adjust the parser predicate in lockstep). The doc note at lighthouse `:72` pointing at `dialect::dialect_spec::doc_whitelist_table_matches_const` should be updated to the new SDK test path (`pmcp_workbook_dialect`).

---

### `Cargo.toml` (root `[workspace] members` — MODIFIED)

**In-repo convention analog:** root `Cargo.toml:541` — the existing members array. Append the two new crate paths (D-04: runtime is slot 2a, dialect is slot 2b):
```toml
# Source: root Cargo.toml:541 (append the two new crates)
members = ["pmcp-macros", ..., "crates/pmcp-sql-server", "crates/pmcp-openapi-server",
           "crates/pmcp-workbook-runtime", "crates/pmcp-workbook-dialect",   # NEW
           "examples/25-oauth-basic", ...]
```
Both new crates are workspace members (not in the `exclude` list at `:543`). They will be picked up by `cargo test --workspace` and the existing `quality-gate`.

---

### `Makefile` (`purity-check` target — NEW)

**Lighthouse lift-source:** the `justfile` recipe `purity-check` (lighthouse:54-92) — the proven cargo-tree reader-vs-writer gate. Lift the cargo-tree arm; DROP the engine-value-path grep arm (that is D-10's clippy job, not the dep boundary) and DROP `pmcp` from the ban token set (D-09 — the SDK runtime may depend on `pmcp`).

**In-repo convention analog:** `Makefile:460-480` (the `quality-gate` target) — the SDK uses `.PHONY:` + `@$(MAKE) sub-target` composition. **Critical (Pitfall 5):** CLAUDE.md mandates `make quality-gate` before commits; the SDK's analogue to the lighthouse `just purity-check` is a NEW `make purity-check` target (NOT a justfile recipe — though a thin justfile passthrough is optional). Per research, keep heavy per-feature runs in CI; provide an on-demand `make purity-check`:
```makefile
# Source pattern: Makefile:460-462 (.PHONY + target shape)
.PHONY: purity-check
purity-check:
	@echo "purity-check: asserting reader/JS stack ∉ workbook crate trees…"
	@for crate in pmcp-workbook-runtime pmcp-workbook-dialect; do \
	  for feat in "" "--no-default-features" "--all-features"; do \
	    if cargo tree -p $$crate $$feat 2>/dev/null | grep -Ei 'umya|calamine|quick-xml|swc_|pmcp-code-mode'; then \
	      echo "purity-check FAILED: reader/JS dep in $$crate ($$feat)"; exit 1; \
	    fi; \
	  done; \
	done
	@if ! cargo tree -p pmcp-workbook-runtime 2>/dev/null | grep -qi 'rust_xlsxwriter'; then \
	  echo "purity-check FAILED: rust_xlsxwriter ABSENT from runtime tree"; exit 1; fi
	@echo "purity-check: reader-free + writer-present (zip permitted via rust_xlsxwriter)"
```

**Deltas from the lighthouse recipe:**
- Ban set is exactly `umya|calamine|quick-xml|swc_|pmcp-code-mode` — `pmcp` is REMOVED (D-09). Do NOT grep `zip` (it enters legitimately via the writer — Anti-Pattern).
- Keep the POSITIVE `rust_xlsxwriter`-present assertion (guards against a vacuous negative pass if the renderer is deleted).
- Drop the lighthouse engine-value-path grep arm (lines 55-73) — that is the crate-level clippy deny's job (D-10), not the purity gate's.

---

### `.github/workflows/ci.yml` (`purity-check` job + `gate` wiring — MODIFIED)

**Lighthouse lift-source:** none — the lighthouse ran purity via `just` locally, no dedicated CI job. The CI wiring is SDK-native.

**In-repo convention analog (TWO references):**
1. **`ci.yml:160-219`** — the `quality-gate` job shape (checkout → `dtolnay/rust-toolchain@stable` → cargo cache → run). The new `purity-check` job should mirror this structure (it needs only the stable toolchain + `cargo tree`; `cargo-deny 0.18.3` is already available per research if Layer 2 is wired):
```yaml
# Source pattern: ci.yml:160-195 (job + toolchain + cache scaffold)
  purity-check:
    name: Purity Gate
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v6
    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable
    - name: Cache cargo
      uses: actions/cache@v5
      with:
        key: ${{ runner.os }}-cargo-purity-${{ hashFiles('**/Cargo.lock') }}
        path: |
          ~/.cargo/registry/index/
          ~/.cargo/registry/cache/
    - name: Run purity gate (per-crate, per-feature)
      run: make purity-check
```
   (Recommended: a `strategy.matrix` over `["", "--no-default-features", "--all-features"] × ["pmcp-workbook-runtime", "pmcp-workbook-dialect"]` per research §CI matrix — the runtime has no `[features]` today so all three trees are identical, but wiring it now future-proofs against a later feature leak.)

2. **`ci.yml:281-296`** — the merge-blocking `gate` aggregation job. To make `purity-check` block merge, add it to `needs:` AND to the result-evaluation script:
```yaml
# Source: ci.yml:281-296 — CURRENT gate job
  gate:
    runs-on: ubuntu-latest
    needs: [test, quality-gate]            # ← APPEND: purity-check
    if: always()
    steps:
      - name: Evaluate required checks
        env:
          TEST_RESULT: ${{ needs.test.result }}
          QG_RESULT: ${{ needs.quality-gate.result }}   # ← ADD: PURITY_RESULT
        run: |
          if [[ "$TEST_RESULT" != "success" ]] || [[ "$QG_RESULT" != "success" ]]; then
            echo "Required checks failed..."; exit 1
          fi
```
**Required edits:** (a) `needs: [test, quality-gate, purity-check]`, (b) add `PURITY_RESULT: ${{ needs.purity-check.result }}` to `env:`, (c) add `|| [[ "$PURITY_RESULT" != "success" ]]` to the `if` condition. This mirrors exactly how CLAUDE.md describes the PMAT `quality-gate` job propagating to the org-required `gate` status check.

---

## Shared Patterns

### SDK crate-manifest convention (literal versions, NOT workspace inheritance)
**Source:** `crates/pmcp-server-toolkit/Cargo.toml:1-29` + `crates/pmcp-sql-server/Cargo.toml:1-44`
**Apply to:** BOTH new `Cargo.toml` files.
Every SDK workspace crate uses literal `version = "0.1.0"`, literal dependency version strings (`serde = { version = "1", features = ["derive"] }`, `thiserror = "2"`), `edition = "2021"`, `license = "MIT OR Apache-2.0"`, `repository = "https://github.com/paiml/rust-mcp-sdk"`, an `exclude = [".planning/", ".pmat/", "fuzz/", "tests/"]`, and a `[package.metadata.docs.rs]` block. The lighthouse `*.workspace = true` idiom is NOT used here (Pitfall 2). `thiserror` is pinned to `2` across the SDK (`pmcp-server-toolkit:29`, `pmcp-sql-server:44`, root `:55`) — the lighthouse `1` must bump (Pitfall 3).

### Path-dependency convention (workspace-internal deps)
**Source:** `crates/pmcp-sql-server/Cargo.toml:33`
**Apply to:** `pmcp-workbook-dialect`'s dep on `pmcp-workbook-runtime`.
```toml
pmcp-workbook-runtime = { version = "0.1.0", path = "../pmcp-workbook-runtime" }
```
Always `version + path` together (so the published crate carries a real version constraint while local builds use the path).

### Crate-level panic-freedom (D-10, NOT the purity gate's job)
**Source:** lighthouse `crates/workbook-runtime/src/lib.rs:18-19`
**Apply to:** `pmcp-workbook-runtime/src/lib.rs` (and, if the dialect crate has a value path, its `lib.rs`).
```rust
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
```
Lift verbatim. This — not the purity gate — enforces panic-freedom (D-10).

### serde/schemars derive discipline
**Source:** lighthouse `finding.rs` + root `Cargo.toml:50,52` (`serde` with `derive`, `schemars = "1.0"`)
**Apply to:** all model types in the runtime.
Enums carry `#[serde(rename_all = "...")]`; public model structs derive `Serialize, Deserialize, schemars::JsonSchema`. The lighthouse already matches this for everything EXCEPT the finding types (which lack `Deserialize` — the single D-08 delta).

### CI gate-propagation (merge-blocking)
**Source:** `.github/workflows/ci.yml:281-296` (the `gate` job) + CLAUDE.md "CI Quality Gates" section
**Apply to:** the new `purity-check` CI job.
A new required check becomes merge-blocking ONLY by being added to the `gate` job's `needs:` array AND its result-evaluation script. Adding a job without wiring `gate` makes it advisory, not blocking.

---

## No Analog Found

None. Every new file maps to BOTH a lighthouse lift-source and an in-repo convention analog. The two files with the weakest direct lighthouse source are:

| File | Role | Note |
|------|------|------|
| `crates/pmcp-workbook-dialect/Cargo.toml` | config | NEW crate (D-01 split) — no lighthouse crate exists; assembled from the SDK manifest convention (`pmcp-server-toolkit`) + the path-dep shape (`pmcp-sql-server:33`). |
| `.github/workflows/ci.yml` `purity-check` job | config/CI | Lighthouse ran purity via `just` locally with no CI job — the CI job is SDK-native, modeled on the existing `quality-gate` job (`ci.yml:160`) + `gate` wiring (`ci.yml:281`). |

---

## Open Items the Planner Must Carry (from RESEARCH, not pattern-mapping)

1. **cargo-deny Layer 2 is partially blocked** (research A5 / Open Question 1): `deny.toml` is infra-managed ("do not edit manually") AND cargo-deny ban scoping is workspace-global (would break Phase 93's legitimate `umya`/`quick-xml` use). **The cargo-tree per-crate arm (Layer 1) + the crate split (Layer 3) fully satisfy WBRT-04.** Treat cargo-deny (Layer 2) as a documented, deferred backstop — do NOT claim a clean three-layer gate.
2. **WBDL-03 traceability update** (research Open Question 2, mechanical/blocking): `.planning/REQUIREMENTS.md` line 103 maps `WBDL-03 → Phase 91`; D-02 re-maps it to Phase 93. Update it AND add `pmcp-workbook-dialect` to ROADMAP Phase 91 scope.
3. **`rust_xlsxwriter` provenance checkpoint** (research A4): add a `checkpoint:human-verify` before the install confirming author `jmcnamara` + repo `github.com/jmcnamara/rust_xlsxwriter` + `cargo audit` clean (slopcheck was unavailable).
4. **Do NOT add a `pmcp` dependency** to the runtime (research A1 / Anti-Pattern): D-09 permits it but the runtime is functionally `pmcp`-free; adding it bloats the tree for nothing.

## Metadata

**Analog search scope (in-repo):** `crates/pmcp-server-toolkit/Cargo.toml`, `crates/pmcp-sql-server/Cargo.toml`, root `Cargo.toml` (`[dependencies]` + `[workspace] members`), `Makefile` (`quality-gate`), `justfile` (recipe convention), `.github/workflows/ci.yml` (`quality-gate` + `gate` jobs).
**Analog search scope (lighthouse):** `crates/workbook-runtime/{Cargo.toml,src/lib.rs,src/finding.rs}`, `crates/workbook-compiler/src/dialect/{rules.rs,mod.rs}`, `docs/workbook-dialect-spec.md`, `justfile` (`purity-check` recipe).
**Files scanned:** 14
**Pattern extraction date:** 2026-06-09
