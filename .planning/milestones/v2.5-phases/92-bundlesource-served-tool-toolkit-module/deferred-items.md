# Deferred Items — Phase 92

## Out-of-scope warnings (not caused by this plan's changes)

- **`crates/pmcp-server-toolkit/src/code_mode.rs:557`** — `unused import: pmcp_code_mode::CodeExecutor`.
  Pre-existing at HEAD (`git show HEAD:crates/pmcp-server-toolkit/src/code_mode.rs` line 557 confirms it).
  Surfaces under the default `code-mode` feature; my Plan 02 changes never touch this file. The real
  CI clippy gate lints only root `pmcp` with an allow-list (the toolkit crate is not clippy-gated — see
  user MEMORY `project_rust195_clippy_gate_debt.md`), so this does not block CI. Left untouched per the
  executor scope boundary (only auto-fix issues directly caused by the current task's changes).
## [92-04] Pre-existing rustfmt drift in Plan-02 test-support files

- **Files:** `crates/pmcp-server-toolkit/tests/support/fixture_gen.rs`, `tests/support/tamper.rs`, `tests/fixture_byte_stability.rs`
- **Discovered during:** Plan 92-04 Task 1 (`cargo fmt -p pmcp-server-toolkit` reformatted them).
- **Issue:** These Plan-02 fixture/tamper helpers were committed with a slightly older rustfmt formatting (multi-line fn args / `assert!` wrapping). A current-toolchain `cargo fmt --all --check` flags them. They are NOT touched by Plan 04 and the change is whitespace-only.
- **Disposition:** Out of scope (executor scope boundary — not caused by Plan 04's changes). Reverted to keep Plan 04 commits scoped. Should be picked up by a workspace-wide `cargo fmt --all` in the next plan that runs `make quality-gate` (Plan 05 wires the builder-ext + purity gate + example, which runs the full gate).
- **Resolution:** RESOLVED in Plan 05 (`style(92-05): rustfmt the plan-02 test-support files`). `make quality-gate` is now clean.

## [92-05] Pre-existing clippy `redundant_guards` warning in http/auth.rs (not workbook)

- **File:** `crates/pmcp-server-toolkit/src/http/auth.rs:538` (`Some(name) if name.is_empty() => ...`).
- **Discovered during:** Plan 92-05 (running default `cargo clippy --features http` over the toolkit to double-check the workbook combos).
- **Issue:** A default-on `clippy::redundant_guards` (rust-1.95) warning surfaces only under `--features http`; the match-guard could be rewritten as `Some("")`. It is in a Phase-90 OpenAPI file, NOT touched by Plan 05's workbook work.
- **Disposition:** Out of scope (executor scope boundary — not caused by Plan 05's changes). `make quality-gate` PASSES (its `make lint` does not flag this — the toolkit crate is not pedantic-gated per user MEMORY `project_rust195_clippy_gate_debt.md`; the real CI lints only root `pmcp` with an allow-list). Left untouched; a future Phase-90 maintenance pass should rewrite the guard.
