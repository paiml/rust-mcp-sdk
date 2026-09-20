# Deferred Items — Phase 86

Out-of-scope discoveries logged during plan execution (SCOPE BOUNDARY rule).
These are NOT caused by this phase's changes and are NOT fixed here.

## Plan 86-01

- **Pre-existing clippy `field_reassign_with_default` (rust-1.95.0) in `crates/pmcp-server-toolkit/src/code_mode.rs:520-521`.**
  - Surfaced by `cargo clippy -p pmcp-server-toolkit --features sqlite,code-mode,http --all-targets`.
  - The file was last modified in commit `d962051e` (Plan 85-10) — NOT touched by Plan 86-01 (this plan modifies only `sql/sqlite.rs`, `lib.rs`, `Cargo.toml`).
  - Consistent with prior STATE.md decisions noting pre-existing rust-1.95.0 pedantic lints in the toolkit surface (Phase 84 Plan 08, Phase 85 Plan 03).
  - Out of scope per SCOPE BOUNDARY; left for a dedicated lint-sweep.

- **Pre-existing workspace `cargo fmt --all --check` failures in Phase 84 connector crates (committed code, not this plan).**
  - `make quality-gate` (= `cargo fmt --all -- --check`) reports reflow diffs in committed code of three crates NOT touched by Plan 86-01:
    - `crates/pmcp-toolkit-athena/src/{dev_mock.rs,lib.rs}`, `crates/pmcp-toolkit-athena/tests/integration.rs`
    - `crates/pmcp-toolkit-mysql/src/{dev_mock.rs,lib.rs}`
    - `crates/pmcp-toolkit-postgres/src/{dev_mock.rs,lib.rs}`
  - These files are committed-clean (no uncommitted edits) — the diffs are a newer-rustfmt-version reflow of Phase 84 code, surfaced workspace-wide. Plan 86-01 touches only `pmcp-server-toolkit/{src/sql/sqlite.rs,src/lib.rs,Cargo.toml}`, all of which ARE fmt-clean.
  - Out of scope per SCOPE BOUNDARY; a workspace-wide `cargo fmt --all` reflow should be done as a dedicated chore commit (or in the relevant connector phase), not silently folded into 86-01.

## Plan 86-04

- **Pre-existing clippy `-D warnings` failures in `cargo-pmcp` bin/lib `src/` (NOT touched by this plan).**
  - Surfaced because `cargo clippy --test scaffold_sql_server` compiles the whole crate (bin + lib) to lint the integration test.
  - Locations (all `src/`, none in the test files this plan adds):
    - `src/lib.rs:21-24` — `clippy::doc_lazy_continuation` (doc list item without indentation)
    - `src/loadtest/summary.rs:58` — `clippy::vec_init_then_push`
    - `src/deployment/config.rs:509` — `clippy::collapsible_match`
    - `src/pentest/attacks/prompt_injection.rs:660,694` — `clippy::type_complexity`
    - `src/pentest/attacks/protocol_abuse.rs:563` — `clippy::unnecessary_cast` (`u32` → `u32`)
  - This plan modifies ZERO `cargo-pmcp/src/` files (`git status --short cargo-pmcp/src/` is clean) — these are newer-clippy reflow/lint findings on committed code, consistent with the pre-existing pentest dead-code warnings noted in 86-03.
  - The files THIS plan adds (`cargo-pmcp/tests/scaffold_sql_server.rs`, `cargo-pmcp/tests/support/scaffold_patch.rs`) are clippy-clean (`--> ... scaffold_*` produced NO errors) and fmt-clean. The test binary compiles via `--no-run`.
  - Out of scope per SCOPE BOUNDARY; left for the phase-end lint sweep.
