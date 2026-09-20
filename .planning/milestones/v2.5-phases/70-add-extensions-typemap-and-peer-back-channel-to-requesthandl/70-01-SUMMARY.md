---
phase: 70
plan: 01
subsystem: server/cancellation + shared/cancellation + workflow/prompt_handler + proptests
tags: [parity-handler-01, extensions-typemap, non-exhaustive, semver, proptests]
dependency_graph:
  requires:
    - http = "1.1" (already direct dep in Cargo.toml:58 — no new deps)
  provides:
    - "RequestHandlerExtra.extensions: http::Extensions typemap"
    - "RequestHandlerExtra::{extensions, extensions_mut}() accessors (both canonical and shared struct)"
    - "#[non_exhaustive] marker on both RequestHandlerExtra structs"
  affects:
    - "Downstream crates using positional struct literals of RequestHandlerExtra (breaking change)"
    - "Phase 70 Plan 02 (server-to-client dispatcher) — builds on #[non_exhaustive]"
    - "Phase 70 Plan 03 (peer wiring) — appends .with_peer(...) without breaking positional literals"
tech-stack:
  added: []
  patterns:
    - "Typed-key middleware → handler state transfer via http::Extensions"
    - "#[non_exhaustive] + ::new() / .with_*(...) builder chain enforcement"
key-files:
  created:
    - tests/handler_extensions_properties.rs
  modified:
    - src/server/cancellation.rs
    - src/shared/cancellation.rs
    - src/server/workflow/prompt_handler.rs
decisions:
  - "Task 1 (struct changes) and Task 2 (prompt_handler struct-literal refactor) committed atomically: Task 1 alone causes E0063 'missing field extensions' in 12 test sites, breaking `make quality-gate`. Single atomic commit is the only path that satisfies Toyota Way zero-tolerance gate at each commit boundary."
  - "Followed the Option A path from 70-RESEARCH.md §4: switch to ::new() + builder chain rather than ..Default::default() spread. Cleaner — the #[non_exhaustive] marker + ::new() mandate flow downstream to external crates without leaking new fields as positional literals."
  - "Per-site non-default-field audit at all 12 prompt_handler.rs sites confirmed 0/12 non-default fields (all sites set only defaults — no .with_*(...) chain needed). Audit documented below."
metrics:
  duration: ~20m (wall clock, inclusive of quality-gate verification)
  completed: 2026-04-16
---

# Phase 70 Plan 01: Add Extensions typemap to RequestHandlerExtra — Summary

**One-liner:** Added `http::Extensions` typemap field to both `RequestHandlerExtra` structs with `#[non_exhaustive]` marker, refactored 12 positional struct-literal test sites in `prompt_handler.rs` to the `::new()` builder form, and shipped 5 proptests (≥100 cases each) covering insert/get roundtrip, key-collision old-value return, clone preservation, `remove<T>()` semantics, and mixed-type coexistence.

## Outcome

All three tasks in `70-01-PLAN.md` executed:

- **Task 1** — `#[non_exhaustive]` + `pub extensions: http::Extensions` field + `extensions()` / `extensions_mut()` accessors landed on BOTH `src/server/cancellation.rs` (canonical) and `src/shared/cancellation.rs` (wasm-safe / parity per Codex review MEDIUM). Default + ::new() + Debug impls all updated. Struct rustdoc carries explicit `# Semver note` section documenting that `#[non_exhaustive]` IS a breaking change for downstream crates that used positional struct literals — NOT breaking for `::new(...)` / `::default()` / `.with_*(...)` users.
- **Task 2** — All 12 positional struct-literal sites in `src/server/workflow/prompt_handler.rs` (lines 1055, 1238, 1347, 1441, 1573, 1704, 1772, 1851, 1948, 2081, 2223, 2338 in the pre-edit file) converted to `RequestHandlerExtra::new(...)` form. Post-edit `grep -c 'RequestHandlerExtra {'` returns `0`. Per-site audit confirmed 0/12 non-default fields dropped.
- **Task 3** — New test file `tests/handler_extensions_properties.rs` with 5 proptests @ 100 cases each: `prop_extensions_insert_get_roundtrip`, `prop_extensions_key_collision_returns_old_value`, `prop_extra_clone_preserves_extensions`, `prop_extensions_remove_returns_value`, `prop_extensions_two_types_coexist`. All pass.

Zero new Cargo.toml `[dependencies]` entries. Zero new feature flags.

## Verification Results

| Check | Status | Notes |
|-------|--------|-------|
| `cargo build --features "full"` | ✅ pass | 0 compile errors |
| `cargo check --target wasm32-unknown-unknown --features schema-generation` | ✅ pass | Extensions field is wasm-safe |
| `cargo test --lib server::cancellation::tests::test_extensions_*` | ✅ 3 pass | default_empty, insert_overwrite_returns_old, debug_prints_type_names_only |
| `cargo test --lib workflow::prompt_handler` | ✅ 21 pass | All existing prompt_handler tests still green after refactor |
| `cargo test --test handler_extensions_properties` | ✅ 5 pass | All 5 proptests pass @ 100 cases each |
| `grep -c 'RequestHandlerExtra {' src/server/workflow/prompt_handler.rs` | ✅ `0` | All 12 sites converted |
| `grep -c 'RequestHandlerExtra::new(' src/server/workflow/prompt_handler.rs` | ✅ `12` | All sites now use ::new() |
| `make quality-gate` | ✅ pass | fmt + clippy pedantic+nursery + build + test + audit all green |

## Per-Site Non-Default-Field Audit (Codex review LOW elevated to acceptance)

All 12 sites in `src/server/workflow/prompt_handler.rs` (pre-edit):

| Site | Line (pre) | Type path | request_id literal | cancellation_token | Non-default fields? |
|------|------------|-----------|--------------------|--------------------|---------------------|
| 1    | 1055       | `RequestHandlerExtra` | `"test-1"` | `Default::default()` | **none** |
| 2    | 1238       | `RequestHandlerExtra` | `"test-integration"` | `Default::default()` | **none** |
| 3    | 1347       | `crate::server::cancellation::RequestHandlerExtra` | `"test"` | `Default::default()` | **none** |
| 4    | 1441       | `crate::server::cancellation::RequestHandlerExtra` | `"test"` | `Default::default()` | **none** |
| 5    | 1573       | `crate::server::cancellation::RequestHandlerExtra` | `"test"` | `Default::default()` | **none** |
| 6    | 1704       | `crate::server::cancellation::RequestHandlerExtra` | `"test"` | `Default::default()` | **none** |
| 7    | 1772       | `crate::server::cancellation::RequestHandlerExtra` | `"test"` | `Default::default()` | **none** |
| 8    | 1851       | `crate::server::cancellation::RequestHandlerExtra` | `"test"` | `Default::default()` | **none** |
| 9    | 1948       | `crate::server::cancellation::RequestHandlerExtra` | `"test"` | `Default::default()` | **none** |
| 10   | 2081       | `crate::server::cancellation::RequestHandlerExtra` | `"test"` | `Default::default()` | **none** |
| 11   | 2223       | `crate::server::cancellation::RequestHandlerExtra` | `"test"` | `Default::default()` | **none** |
| 12   | 2338       | `crate::server::cancellation::RequestHandlerExtra` | `"test"` | `Default::default()` | **none** |

**Audit verdict:** 0/12 non-default fields — no `.with_*(...)` chaining needed. All sites safe to convert to `::new()` alone.

## Deviations from Plan

### Rule 3 — Blocking issue (auto-fixed)

**1. [Rule 3 - Blocking] Task 1 + Task 2 committed atomically instead of as two separate commits**
- **Found during:** Task 1 verify step (`cargo test --lib server::cancellation::tests::...`)
- **Issue:** The plan's Task 1 verify command compiles the library-under-test (which includes `src/server/workflow/prompt_handler.rs`). Because Task 1 adds a required `extensions: http::Extensions` field to a struct still marked `#[non_exhaustive]` (but the 12 positional struct-literal test sites at `prompt_handler.rs:1055..` have not yet been refactored — that's Task 2), `cargo test --lib` fails with 12 × `E0063: missing field 'extensions' in initializer` errors. Per `CLAUDE.md`, `make quality-gate` (and therefore per-commit pre-commit hooks) must be green at every commit boundary.
- **Fix:** Combined Task 1 and Task 2 edits into a single atomic commit (`9d1a2257`). Task 3 (proptest file) is purely additive and does not depend on the struct changes at compile time, but was included in the same atomic commit for coherence of the plan's deliverable story.
- **Files modified:** `src/server/cancellation.rs`, `src/shared/cancellation.rs`, `src/server/workflow/prompt_handler.rs`, `tests/handler_extensions_properties.rs`
- **Commit:** 9d1a2257

**2. [Rule 2 - Missing critical functionality observation, not fix] `src/shared/cancellation.rs` is not in the build graph**
- **Found during:** Task 1 verify (`cargo test shared::cancellation::tests::test_shared_extensions_parity` returned 0 tests)
- **Issue:** The file `src/shared/cancellation.rs` exists on disk and is referenced by `use crate::shared::cancellation::RequestHandlerExtra;` in `src/server/traits.rs` and `src/wasi.rs`, BUT: (a) there is NO `pub mod cancellation;` declaration anywhere in `src/shared/mod.rs` or elsewhere in the src/shared tree, and (b) `src/server/traits.rs` itself is also an orphan file (not declared as a module). Verification: `mv src/shared/cancellation.rs .bak && cargo check --features full` compiles cleanly, proving the file is not part of the active build.
- **Consequences for this plan:**
  - `grep`-based acceptance criteria on `src/shared/cancellation.rs` ARE satisfied (the file carries `#[non_exhaustive]`, the `extensions` field, and the `extensions()` / `extensions_mut()` accessors exactly as the plan specifies).
  - The named `test_shared_extensions_parity` runtime test cannot execute because the module is not compiled. This is a pre-existing repo condition, not a defect introduced by this plan.
- **Fix:** Did NOT re-wire the orphan module — that is out of scope for this plan (would be a Rule 4 architectural decision). The shared/cancellation.rs file is now structurally parity-complete for the day the module is wired back into the build. Documented in this SUMMARY under Known Limitations; flagged for potential inclusion in a future cleanup phase.
- **Files observed:** `src/shared/cancellation.rs`, `src/shared/mod.rs`, `src/server/traits.rs`, `src/wasi.rs`
- **Commit:** 9d1a2257 (no extra commit — observation only)

## Known Limitations

- `src/shared/cancellation.rs` and `src/server/traits.rs` are orphan files (not declared in any `mod ...;` statement). They compile only when somebody imports them with `mod path = "..."`, which does not appear to happen in the active build graph. Changes made in this plan to `src/shared/cancellation.rs` are structurally correct and grep-visible, but cannot be runtime-verified via `cargo test` until a future phase re-wires the module. The canonical `src/server/cancellation.rs` is the sole build-active struct — all 3 unit tests there pass.

## Threat Flags

None. All threat-register dispositions from the plan's `<threat_model>` block were honored:

- **T-70-01 (Info Disclosure)** mitigated: Debug impl line `.field("extensions", &self.extensions)` prints only type names (http::Extensions own Debug contract); verified by `test_debug_extensions_prints_type_names_only` unit test asserting inserted `"SECRET_VALUE_DO_NOT_LEAK"` is NOT in Debug output.
- **T-70-03 (Elevation of Privilege)** mitigated: `http::Extensions::insert<T>` requires `T: Clone + Send + Sync + 'static` — non-'static smuggling is compile-blocked.
- **T-70-06 (Spoofing — struct-literal test sites)** accepted and disposed: all 12 test sites converted to `::new()` form, no longer expose full struct surface.
- **T-70-11 (Tampering — downstream semver)** accepted and documented: explicit `# Semver note` rustdoc section on the struct.

## Self-Check: PASSED

- ✅ File created: `tests/handler_extensions_properties.rs`
- ✅ File modified: `src/server/cancellation.rs` (verified `#[non_exhaustive]` + `pub extensions` + `extensions_mut()` grep-visible)
- ✅ File modified: `src/shared/cancellation.rs` (verified `#[non_exhaustive]` + `pub extensions` + parity accessors grep-visible)
- ✅ File modified: `src/server/workflow/prompt_handler.rs` (`grep -c 'RequestHandlerExtra {'` = 0, `grep -c 'RequestHandlerExtra::new('` = 12)
- ✅ Commit exists: `9d1a2257 feat(70-01): add http::Extensions typemap to RequestHandlerExtra`
- ✅ `make quality-gate` green (fmt + clippy pedantic+nursery + build + test + audit)
- ✅ 5 proptests pass @ 100 cases each
- ✅ 3 new unit tests in server::cancellation::tests pass
- ✅ 21 existing prompt_handler tests still pass after refactor
- ✅ WASM check compiles (`cargo check --target wasm32-unknown-unknown --features schema-generation`)
