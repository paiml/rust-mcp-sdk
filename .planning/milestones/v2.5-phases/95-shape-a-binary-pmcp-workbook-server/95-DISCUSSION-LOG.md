# Phase 95: Shape A Binary `pmcp-workbook-server` - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-14
**Phase:** 95-shape-a-binary-pmcp-workbook-server
**Areas discussed:** Bundle source & CLI shape, pmcp.toml resolution, Transport surface, Mandatory example + fixture

---

## Bundle source & CLI shape — `--bundle-dir` / `--bundle-id` semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Dir = exact bundle, id asserts | `--bundle-dir` is the exact `bundle@version` dir; `--bundle-id` asserted against `BUNDLE.lock`, fails closed on mismatch. Matches D-08 + `LocalDirSource::new(dir)`. | ✓ |
| Dir = parent, id+version selects | `--bundle-dir` is a parent dir; `--bundle-id` (+ `--bundle-version`) selects the `<id>@<version>` subdir. | |
| You decide | Let planning pick based on layout. | |

**User's choice:** Dir = exact bundle, id asserts
**Notes:** Version is implicit in the path; `--bundle-id` is a fail-closed guard, not a resolution input. No `--bundle-version` flag. → CONTEXT D-01.

---

## Bundle source & CLI shape — embedded vs LocalDirSource only

| Option | Description | Selected |
|--------|-------------|----------|
| LocalDirSource only | Crate uses `workbook` feature (no `include_dir`). Pure point-at-a-dir, mirrors `pmcp-sql-server`, satisfies success criterion 3. | ✓ |
| Also support embedded | Enable `workbook-embedded` for a self-contained baked-in bundle. | |
| You decide | Let planning choose feature posture. | |

**User's choice:** LocalDirSource only
**Notes:** `EmbeddedSource` stays the Shape B scaffold's job (Phase 96). → CONTEXT D-02.

---

## pmcp.toml resolution

| Option | Description | Selected |
|--------|-------------|----------|
| Pure CLI args, no toml | Binary takes only `--bundle-dir` (+ `--bundle-id` assertion); `pmcp.toml` stays a build-time artifact. Mirrors sql-server's single-`--config`. | ✓ |
| Read pmcp.toml at runtime | Resolve `--bundle-id` against `pmcp.toml` (id→out_dir). | |
| You decide | Let planning decide. | |

**User's choice:** Pure CLI args, no toml
**Notes:** Keeps the served binary uncoupled from the build-time project layout. → CONTEXT D-03.

---

## Transport surface

| Option | Description | Selected |
|--------|-------------|----------|
| HTTP-only, loopback default | Streamable-HTTP only, `--http 127.0.0.1:8080`, SDK Tower/axum adapter. No stdio. Matches Phase 92 D-12. | ✓ |
| HTTP + stdio | Also offer stdio for local Claude Desktop testing. | |
| You decide | Let planning pick. | |

**User's choice:** HTTP-only, loopback default
**Notes:** Mirrors `pmcp-sql-server` exactly; remote business-user story. → CONTEXT D-04.

---

## Mandatory example + fixture

| Option | Description | Selected |
|--------|-------------|----------|
| --bundle-dir at committed golden | Example drives the binary path with `--bundle-dir` at the synthetic tax-calc golden, serving over HTTP. End-to-end pure-config deploy story. | ✓ |
| Reuse Phase 92's HTTP example | Lean on the existing `EmbeddedSource`-based `workbook_server_http` toolkit example. | |
| You decide | Let planning choose. | |

**User's choice:** --bundle-dir at committed golden
**Notes:** Consistent with LocalDirSource-only (D-02); zero customer data (Phase 92 D-01). → CONTEXT D-05.

---

## Claude's Discretion

- Crate/module file layout (mirror `pmcp-sql-server`'s lib/cli/main/assemble split).
- Exact `RunError` variant set (model on sql-server's; add an id-mismatch variant for D-01).
- Crate metadata: version (`0.1.0`), description, keywords, `exclude`, docs.rs config.
- Where the synthetic golden fixture the example/tests point at lives; reuse existing committed golden.
- Publish-slot ("9a") wiring detail in CLAUDE.md's release order.

## Deferred Ideas

- Embedded (`include_dir`/`EmbeddedSource`) bundle support → Phase 96 (Shape B scaffold).
- Runtime `pmcp.toml` resolution → rejected (D-03).
- stdio transport → rejected (D-04).
- `--bundle-dir` parent-dir + `--bundle-version` selection → rejected (D-01).
- Dialect-version in the provenance stamp → Phase 96 (WBDL-02).
