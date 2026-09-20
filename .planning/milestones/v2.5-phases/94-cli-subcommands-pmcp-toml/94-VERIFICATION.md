---
status: passed
phase: 94-cli-subcommands-pmcp-toml
verified: 2026-06-13
verifier: inline (gsd-verifier subagent died on API socket error; orchestrator verified goal-backward against the live codebase + test runs)
plans: [94-00, 94-01, 94-02, 94-03, 94-04, 94-05]
requirements: [WBCL-01, WBCL-02, WBCL-03, WBCL-04]
score: 4/4 requirements delivered
---

# Phase 94 Verification — CLI Subcommands + `pmcp.toml`

**Goal:** The compiler's verbs become first-class `cargo pmcp` subcommands (thin shells over
the Phase 93 compiler) carrying the gated BA approval flow, and a project-level `pmcp.toml`
maps workbooks → bundle IDs, eliminating the lighthouse's single-workbook assumptions.

**Verdict: PASSED.** All four requirements delivered and ground-truth verified against the
live code (not just SUMMARY claims). The gated approval lane — the headline goal — ships this
phase via the Wave-0 library-seam plan rather than being deferred.

## Requirement Coverage (ground-truth verified)

| Req | What it demands | Evidence in code | Status |
|-----|-----------------|------------------|--------|
| WBCL-01 | compile: ingest→lint→…→gate→write, gate before any write; gated BA approval flow | `compile.rs`: `--approver required=true` (D-06); version via `read_workbook_version` (no `--version` flag, no hardcode — doc-confirmed at lines 25/67); seed lane (`compile_workbook`) + gated lane (`prepare_candidate`→`gate::gate`→block-or-`promote`); gate-block → `WorkbookExit::gate_block` distinct **exit 2**, writes nothing; `main.rs` `downcast_ref::<WorkbookExit>()`→`std::process::exit(code)`. Integration test confirms versionless-fixture refusal (correctness boundary). | ✅ |
| WBCL-02 | lint standalone, non-zero on errors | `lint.rs`: ingest→`dialect::linter::lint`→pure `format_lint_report` (text/json) + `lint_exit_code` (errors fail, warnings pass — D-10). Integration: `lint_over_the_fixture_exits_zero_with_no_findings`, `lint_format_json_emits_parseable_json_on_stdout`. | ✅ |
| WBCL-03 | emit without the gate (dev/reference) | `emit.rs`: **zero** `gate::gate` calls; hash-covered `gated:false` marker via `write_gate_marker` (`evidence/gate.json` + sidecar `gate.sha256`); loud `UNGATED` banner (deterministic stderr); `--approver` not required; CR-02 `@<version>` non-overwrite. | ✅ |
| WBCL-04 | project-level `pmcp.toml` maps workbooks → bundle IDs; kills single-workbook assumption | `config.rs`: `PmcpToml`/`WorkbookEntry` (`path`+`bundle_id`+`out_dir` only — no version/approver per D-02); `validate(&self, project_root)` containment (rejects absolute + `..`-escape); `load()`→`Ok(None)` when absent (optional, D-03); `resolve`/`all_entries`; bare `compile`=compile-all (D-05). Integration: `compile_all_over_two_entry_toml_attempts_both_with_continue_on_error`. | ✅ |

## Locked-decision spot-checks

- **D-04 (group naming):** `Workbook` variant in `main.rs` `enum Commands`; surfaces as `workbook compile|lint|emit`. **NOT** in `is_target_consuming()` (0 matches) — no Phase-77 env injection. ✅
- **D-08 (ungated marker travels with artifact):** `write_gate_marker`/`read_gate_marker` round-trips to `(gated=false, digest_ok=true)`; an edited marker flips `digest_ok` false (tamper-evident). ✅
- **D-11 (version stays in Excel):** compile/emit refuse the versionless fixture rather than defaulting — proven by `compile_seed_lane_over_the_fixture_refuses_a_versionless_workbook` + `emit_over_the_fixture_refuses_a_versionless_workbook`. ✅
- **Served-contract guard:** `crates/pmcp-workbook-runtime/` frozen evidence set untouched this phase (empty diff); integration tests `served_runtime_tree_stays_reader_free` + `served_dialect_tree_stays_reader_free` confirm `umya`/`quick-xml` stay out of the served trees. The new `cargo-pmcp → pmcp-workbook-compiler` offline-cone edge is non-vacuous (`cargo_pmcp_links_the_compiler_non_vacuously`). ✅

## Test evidence (integrated tree, run by orchestrator)

- `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::` → **53 passed, 0 failed** (incl. 5 bounded proptests).
- `cargo test -p cargo-pmcp --test workbook_cli_integration` → **10 passed, 0 failed, 1 ignored** (the `make purity-check` test is intentionally `#[ignore]`d to keep the default run fast).
- `cargo build -p cargo-pmcp -p pmcp-workbook-compiler` → builds clean (15 pre-existing dead-code warnings in unrelated pentest modules, not workbook code).
- `cargo run -p cargo-pmcp --example workbook_cli_demo` → exits 0; `cargo clippy -p cargo-pmcp --examples` → no issues.

## Residual risk (documented, not a delivery gap)

The gate-BLOCK happy path and accepted-version E2E require a genuine versioned (and two-version)
workbook fixture that cannot be constructed from the sole Phase-93 `tax-calc.xlsx`. These paths
are **unit-covered** in 94-00/94-03/94-04; the integration suite covers everything reachable from
the available fixture. Recorded in `94-05-SUMMARY.md` as residual risk.

## Environment caveat (not a phase defect)

`make quality-gate`'s `cargo fuzz` ASAN/sancov step fails on this host
(`failed to run rustc to learn about target-specific information` — a pre-existing nightly
sanitizer-toolchain limitation; the recipe `|| echo`s it and `make` exits 0). `fmt`, `clippy`,
`build`, and `test` all pass. Every executor confirmed this is environmental, unrelated to the
workbook code.

## Test-target note for future phases

`cargo-pmcp`'s `commands::*` modules compile into the **bin target only** (`lib.rs` excludes
them). Use `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::<mod>` — `--lib` reports a false
`0 passed`. Discovered during execution (Genchi Genbutsu); the cross-AI review's `--lib` concern
was directionally right for a subtler reason than it stated.
