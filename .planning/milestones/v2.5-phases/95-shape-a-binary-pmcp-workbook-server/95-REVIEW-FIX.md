---
phase: 95-shape-a-binary-pmcp-workbook-server
fixed_at: 2026-06-14T00:00:00Z
review_path: .planning/phases/95-shape-a-binary-pmcp-workbook-server/95-REVIEW.md
iteration: 2
findings_in_scope: 3
fixed: 3
skipped: 0
status: all_fixed
---

# Phase 95: Code Review Fix Report

**Fixed at:** 2026-06-14
**Source review:** .planning/phases/95-shape-a-binary-pmcp-workbook-server/95-REVIEW.md
**Iteration:** 2 (cumulative across two `--fix` passes)

**Summary:**
- Findings in scope: 3 (all severities — `--fix` pass 1 covered Critical+Warning, pass 2 added Info via `--all`)
- Fixed: 3 (1 Warning + 2 Info)
- Skipped: 0

All three review findings are now resolved. WR-01 was fixed in the first
`--fix` pass (`fix_scope=critical_warning`); IN-01 and IN-02 were fixed in the
second `--fix --all` pass (`fix_scope=all`).

## Fixed Issues

### WR-01 (Warning): `RunError::Serve` is overloaded — a `Server::build()` failure is reported as a transport-start error

**Files modified:** `crates/pmcp-workbook-server/src/lib.rs`, `crates/pmcp-workbook-server/src/assemble.rs`
**Commit:** `1e349cc8`
**Applied fix:**
- Added a distinct `Build(#[source] pmcp::Error)` variant to `RunError` in
  `lib.rs` with `#[error("workbook server build failed: {0}")]` and rustdoc
  contrasting it with `Serve` (build = in-process assembly before any listener
  is bound; serve = transport startup). The transport `Serve` variant was left
  untouched.
- Re-mapped the final `.build()` failure in `assemble.rs` from `RunError::Serve`
  to `RunError::Build`, so an in-process assembly fault no longer surfaces as
  "streamable-HTTP server failed to start".
- Reconciled the contradicting rustdoc on `build_server`'s `# Errors` section
  and the `Serve` variant so each error names its real phase.
- Added a regression test `run_error_build_display_names_the_build_phase`.

### IN-01 (Info): Example uses `#[tokio::main]` but never awaits

**File modified:** `crates/pmcp-workbook-server/examples/workbook_server_min.rs`
**Commit:** `1fe4b64e`
**Applied fix:**
- Verified the example body contains no `.await` (it only calls the synchronous
  `build_server(&args)?` and prints), then dropped the `#[tokio::main]`
  attribute and converted `async fn main` to plain
  `fn main() -> Result<(), Box<dyn std::error::Error>>` (body unchanged).
- This removes the dead multi-thread Tokio runtime ceremony for a non-async
  demonstration. `cargo run --example workbook_server_min` still exits 0
  ("calculate present: true").

### IN-02 (Info): CLAUDE.md slot-9a entry references `pmcp-workbook-runtime`, which is not a numbered publish item

**File modified:** `CLAUDE.md`
**Commit:** `4250ce2c`
**Applied fix:**
- Lighter-touch reconciliation (no renumbering): appended a clarifying note to
  the slot-9a publish-order entry explaining that `pmcp-workbook-runtime` is not
  a numbered item because the binary depends on it only transitively through
  `pmcp-server-toolkit`'s `workbook` feature, and it is published out-of-band by
  its own Phase 91/92 release ahead of `pmcp-server-toolkit` (item 5). Verified
  against `cargo tree -p pmcp-workbook-server` (runtime present transitively; no
  `-dialect` crate in the served tree). Exactly one publish-order line changed;
  no other CLAUDE.md section disturbed.

## Skipped Issues

None.

## Verification (scoped to `-p pmcp-workbook-server`)

- `cargo build -p pmcp-workbook-server` — exit 0
- `cargo run -p pmcp-workbook-server --example workbook_server_min` — exit 0
- `cargo test -p pmcp-workbook-server` — all green (13 lib incl. the WR-01
  regression test, 4 integration suites, 4 doctests)
- `cargo clippy -p pmcp-workbook-server --all-targets` — clean for this crate
  (the residual `redundant guard` warning is in the `pmcp-server-toolkit`
  dependency — pre-existing and out of scope, as REVIEW.md documents)
- `cargo fmt -p pmcp-workbook-server` — applied

Note: the workspace-wide `pmcp-toolkit-mysql` build failure (a pre-existing sqlx
SQL-injection lint) is orthogonal — that crate is not in this binary's
dependency cone — so all verification was scoped to `-p pmcp-workbook-server`.

_Report authored by the orchestrator: the gsd-code-fixer applied and committed
all three fixes (1e349cc8, 1fe4b64e, 4250ce2c), but its pass-2 REVIEW-FIX.md
write did not persist to the main checkout; this report reconstructs the fix
record from the committed work and re-verified results._
