---
phase: 94
reviewers: [codex, gemini]
reviewed_at: 2026-06-13T05:41:42Z
plans_reviewed: [94-01-PLAN.md,94-02-PLAN.md,94-03-PLAN.md,94-04-PLAN.md,94-05-PLAN.md]
---

# Cross-AI Plan Review — Phase 94

## Codex Review

**Summary**

The plans are thorough and well aligned with the phase goals at the requirements level, especially around `pmcp.toml`, the `workbook` command group, approval ergonomics, and the ungated `emit` marker. The main risk is that Plans 03 and 04 drift away from “thin CLI shell” into reconstructing compiler internals. There are also several executability gaps: invalid likely test commands, custom exit-code handling not designed end-to-end, temporary stubs that may violate quality policy, and vague seams for version discovery, gated update promotion, and ungated evidence injection.

## 94-01: `pmcp.toml` Parser

**Strengths**

- Strongly scoped to WBCL-04.
- Good optional config behavior: missing `pmcp.toml` is not an error.
- Duplicate `bundle_id` and path traversal are correctly identified as security concerns.
- Property testing is appropriate for TOML/config parsing.

**Concerns**

- **HIGH:** `validate(&self)` cannot correctly enforce “out_dir under project root” without a `project_root` parameter or a clearly defined lexical containment algorithm.
- **HIGH:** `cargo test -p cargo-pmcp --lib workbook::config` may be invalid if `cargo-pmcp` is binary-only. Use the repo’s actual test target shape.
- **MEDIUM:** `grep -v '^#' ... version\|approver` is brittle; it may match comments, tests, error text, or planning names.
- **MEDIUM:** Arbitrary `".*"` TOML fuzz may generate very large/unhelpful cases and slow tests.
- **LOW:** Rejecting all `..` segments is conservative but should be explicitly documented if `foo/../bar` is rejected.

**Suggestions**

- Change validation to `validate(&self, project_root: &Path)` or store a resolved root-aware config type.
- Add explicit absolute-path handling for `path` and `out_dir`.
- Replace grep acceptance with structural tests that prove `WorkbookEntry` has only the expected fields via serde deny/unknown-field behavior if desired.
- Use bounded regex/string strategies for proptest.

**Risk Assessment: MEDIUM**

Good plan, but path containment and test-target assumptions need tightening before execution.

## 94-02: Command Group + Lint

**Strengths**

- Correctly establishes `cargo pmcp workbook ...` as a subcommand group.
- Good reuse intent for `render_lint_report` and `lint_exit_code`.
- Correctly keeps workbook commands out of target-consuming env injection.
- Human/JSON split matches the BA and CI audiences.

**Concerns**

- **HIGH:** Plan creates `compile.rs` and `emit.rs` stubs, but they are not listed in `files_modified`. That breaks ownership/dependency clarity.
- **HIGH:** Stubs that ship a user-visible “not yet implemented” command may violate the repo’s zero-defect policy if committed between waves.
- **MEDIUM:** `// STUB:` comments may be treated as SATD-like debt by local policy/tools.
- **MEDIUM:** `render_lint_report` prints directly, which makes JSON round-trip tests awkward without stdout capture.
- **MEDIUM:** `bail!` after printing gives only the process’s default error exit behavior; fine for lint’s nonzero, but this pattern will not support Plan 03’s distinct gate code.

**Suggestions**

- Either include stub files in `files_modified` or split the enum so unimplemented variants are added only when handlers exist.
- Prefer placeholder modules without debt language, or implement compile/emit arg structs in the later plans and keep Plan 02 focused on lint.
- Split rendering into `format_lint_report(...) -> String` and `print_lint_report(...)`.
- Confirm the exact ingest API before plan execution; “find the ingest fn” is a planning gap.

**Risk Assessment: MEDIUM**

The CLI skeleton is straightforward, but intermediate stubs and testability need cleanup.

## 94-03: Compile Handler

**Strengths**

- Captures the key UX decisions: mandatory `--approver`, `--accept`, bundle-id resolution, compile-all, and gate-block output.
- Correctly calls out gate-before-write as the central safety property.
- Distinct exit-code intent is good for CI.

**Concerns**

- **HIGH:** This is the largest scope-risk plan. It asks the CLI to assemble `PromoteInputs`, derive/replay corpus, detect baselines, compute candidate hashes, and call gate/promote internals. That is no longer a thin shell unless Phase 93 already exposes a single facade.
- **HIGH:** Gate-before-write may be impossible if the only public “compile” API writes as part of `compile_workbook`. The plan needs a precise candidate-build API that does not emit/promote before gating.
- **HIGH:** Distinct exit code `2` is not implemented by returning `anyhow::Result<()>` unless `main.rs` already supports typed exit errors. The plan does not modify the dispatcher.
- **HIGH:** `compile_workbook(..., version, ...)` requires a version, but the plan says “version comes from workbook” without specifying the public API to read it before compile.
- **MEDIUM:** Path-vs-bundle-id detection using “path-that-is-a-file” is ambiguous for nonexistent workbook paths.
- **MEDIUM:** `--out` override must receive the same containment validation as `pmcp.toml`, but no shared API is specified.

**Suggestions**

- Add or require a compiler-level facade such as `prepare_candidate`, `compile_seed`, `gate_candidate`, `accept_candidate`, `promote_candidate`, then keep CLI orchestration shallow.
- Define a typed CLI error/exit-code path in `main.rs`, or use `std::process::ExitCode` consistently.
- Specify how workbook version is obtained from the library.
- Make target resolution explicit: `--workflow` implies path mode, otherwise resolve bundle id; do not rely only on file existence.
- Add a hard acceptance criterion that no cargo-pmcp code duplicates gate fingerprint or promotion logic.

**Risk Assessment: HIGH**

This plan is the most likely to violate the phase boundary and fail during implementation because library seams are underspecified.

## 94-04: Emit Handler

**Strengths**

- Clear WBCL-03 behavior: ungated, loud banner, persisted evidence marker.
- Correctly does not require `--approver`.
- Reuses `pmcp.toml` resolution and lint behavior.

**Concerns**

- **HIGH:** “Compile minus gate” again risks reimplementing compiler pipeline internals in cargo-pmcp.
- **HIGH:** The evidence marker seam is not confirmed. If `EvidenceInputs` cannot express `gated: false`, this plan requires compiler changes not listed.
- **MEDIUM:** “Emit cannot overwrite promoted baseline” is asserted, but direct `emit_bundle` may not use the same atomic promotion semantics as gated promote.
- **MEDIUM:** The banner behavior is inconsistent: gated on quiet, but “prefer emitting even in quiet.” Safety warning policy should be deterministic.
- **LOW:** If lint errors block emit, that should be stated as a decision, not “Claude’s discretion.”

**Suggestions**

- Prefer a compiler facade like `emit_ungated_bundle(..., evidence_marker)` to avoid duplicating compile/reconcile/emit assembly.
- Confirm `emit_bundle` write semantics and non-overwrite behavior before execution.
- Make the ungated banner always stderr, even in quiet, unless `--format json` is used; for JSON, put status on stderr only.
- Define exact marker filename and schema, e.g. `evidence/gate.json` with `{ "gated": false }`.

**Risk Assessment: HIGH**

The intent is right, but the plan depends on unverified low-level artifact seams and may pull compiler logic into the CLI.

## 94-05: Integration, Example, Purity

**Strengths**

- Good closure plan: real CLI integration, purity gate, runnable example.
- Correctly validates that `cargo-pmcp` is offline tooling and served crates remain reader-free.
- Covers all WBCL requirements end-to-end.

**Concerns**

- **HIGH:** Running `make purity-check` inside a cargo test can be slow, recursive, and flaky. It belongs in explicit verification or an ignored test.
- **HIGH:** The plan allows a “TODO note” for gate-block E2E. The repo forbids SATD; do not add TODOs.
- **MEDIUM:** Existing fixture availability is assumed. If no two-version/gate fixture exists, Plan 03’s most important behavior remains only unit-level.
- **MEDIUM:** `cargo run -p cargo-pmcp --example workbook_cli_demo` may be awkward if examples need to invoke the same package binary or depend on fixture paths.
- **LOW:** Integration tests that spawn cargo-pmcp and process real workbooks may be expensive; keep fixture count minimal.

**Suggestions**

- Keep `make purity-check` as a required manual/CI verification step, not a normal integration test. If persisted, mark ignored with clear command docs.
- Replace TODO with a concrete skipped/ignored test only if unavoidable, or omit the note and document residual risk in the summary.
- Add a small reusable test fixture helper.
- Ensure the example uses direct handler calls or a stable fixture path via `CARGO_MANIFEST_DIR`.

**Risk Assessment: MEDIUM**

Good validation strategy, but purity and example mechanics should be made less fragile.

## Overall Risk Assessment: HIGH

The phase goal is achievable, and the plans are unusually complete on requirements, UX, and security intent. The high risk comes from Plans 03 and 04 depending on internal compiler mechanics that may not be exposed as clean library verbs. Before execution, tighten the public compiler seams, define exit-code plumbing, remove intermediate stubs/TODO-style debt, and replace vague “find the API” instructions with exact callable interfaces. That will keep this phase a thin CLI shell instead of a second compiler implementation.

---

## Gemini Review

# Phase 94: CLI Subcommands + `pmcp.toml` - Plan Review

This review evaluates the five implementation plans for adding the `cargo pmcp workbook` subcommand surface and the project-level `pmcp.toml` configuration.

## Summary
The plans provide a comprehensive and idiomatic implementation of the Phase 94 requirements. By surfacing the `pmcp-workbook-compiler` library verbs through a namespaced subcommand group (`cargo pmcp workbook`), the plans ensure a clean and scalable CLI surface. The inclusion of a project-level `pmcp.toml` successfully generalizes the tool beyond the original lighthouse project's hardcoded paths. The testing strategy is exceptionally robust, incorporating unit tests, property-based fuzzing (proptest) for the new parser, and end-to-end integration tests that reuse existing library fixtures.

## Strengths
*   **Persona-Driven Design:** The plans maintain a strong focus on the Business Analyst (BA) lifecycle, ensuring low-friction approval flows (copy-pasteable `--accept` commands) and actionable findings.
*   **Security & Integrity:** Mandatory `--approver` flags, path-traversal rejection in `pmcp.toml`, and the "gate-before-write" invariant are all explicitly addressed and tested.
*   **Robust Parsing:** Using `proptest` for the `pmcp.toml` parser is an excellent application of the "Always Fuzz" mandate for externally supplied input.
*   **Architectural Purity:** The plans explicitly verify that the Excel-reader dependency (`umya`) remains confined to the offline CLI tooling and does not leak into served runtime crates.
*   **Pattern Adherence:** The subcommands are modeled directly after existing `cargo-pmcp` groups (`app`, `configure`), ensuring consistency in argument parsing and output rendering (Quiet/JSON/Human).

## Concerns

*   **Exit Code Propagation (MEDIUM):** Plans 02 and 03 define specific exit codes (0: OK, 1: Error, 2: Gate-Block). However, since the handlers return `anyhow::Result<()>`, `anyhow` typically maps all errors to exit code `1`. To ensure a code of `2` reaches the shell, the handler must either call `std::process::exit(2)` directly (after flushing stdout/stderr) or the `main.rs` dispatcher must be updated to handle a specific error type that carries the exit code.
*   **Compile-All Failure Policy (LOW):** In the `compile-all` scenario, if multiple workbooks are processed, the plans suggest a "worst-status-wins" policy. It should be clarified that an error in the first workbook does not necessarily prevent the compilation of subsequent workbooks (unless they are dependent, which is not currently the case).

## Suggestions
*   **Consolidated Exit Code Logic:** Factor the exit-code mapping into a shared utility within `workbook/mod.rs` so that `lint`, `compile`, and `emit` (if it ever gains a gate) use the same integer constants.
*   **JSON Envelope Consistency:** Ensure the JSON output for `--format json` wraps the library types in a consistent envelope (e.g., `{ "status": "error", "data": ... }`) if the library types themselves don't provide a root object.
*   **Handling `std::process::exit`:** If using `std::process::exit`, ensure all `Drop` types (like loggers or file buffers) are flushed, or prefer returning a custom error variant to `main.rs`.

## Risk Assessment
**Risk Level: LOW**

The risk is low because the phase is a "thin shell" over a library that has already been hardened in Phase 93. No new business or compiler logic is being introduced. The most significant risks (path traversal and dependency leaks) are proactively mitigated through validation logic and the purity-gate check. The dependency on existing fixtures for integration testing ensures that the CLI is verified against real-world data from day one.

---
**Verdict:** APPROVED. Proceed with Wave 1 (Plan 94-01).

---

## Consensus Summary

Two independent reviewers (Codex, Gemini) assessed the five Phase 94 plans. They
**agree the plans are unusually complete** on requirements coverage, BA-lifecycle UX,
security intent, and testing — but **diverge sharply on overall risk**: Gemini rates
the phase **LOW** ("thin shell over a hardened library, no new logic"), while Codex
rates it **HIGH**, worried that Plans 03/04 underspecify the library seams and could
drift into reimplementing compiler internals inside cargo-pmcp.

The divergence is itself the signal: the plans are *requirement-complete* but have a
few *executability seams* that, if the library facade is clean, make this LOW (Gemini's
read) and, if it isn't, make this HIGH (Codex's read). Resolving the seams below
collapses the disagreement.

### Agreed Strengths
- **`workbook` subcommand group + `pmcp.toml`** correctly generalize beyond the
  lighthouse's single-workbook assumption (WBCL-04), modeled on existing `app`/`configure` groups.
- **Security posture is proactive** — mandatory `--approver`, path-traversal rejection,
  gate-before-write, and the `gated: false` evidence marker are all explicitly tested.
- **`proptest` fuzzing of the `pmcp.toml` parser** is the right application of the
  ALWAYS-fuzz mandate for externally-supplied input.
- **Purity-gate confirmation** (umya/xlsx deps stay out of served crates) is a genuine
  architectural check, enforced by tooling not prose.

### Agreed Concerns (highest priority)
1. **[HIGH — top shared issue] Exit-code propagation.** Both reviewers independently flag
   that handlers returning `anyhow::Result<()>` collapse all errors to exit code `1`.
   The distinct **gate-block = exit 2** (D-10) will NOT reach the shell unless `main.rs`
   gains typed-exit handling or handlers use `std::process::ExitCode` (with flushed
   stdout/stderr). **No plan currently modifies the dispatcher for this.** This is the
   single most actionable fix.
2. **[MEDIUM] Exit-code logic should be one shared utility** in `workbook/mod.rs`
   (shared integer constants) rather than per-handler — both reviewers converge here.

### Divergent Views (worth investigating before execution)
- **Overall risk LOW vs HIGH** — driven entirely by whether Phase 93 exposes clean
  library verbs (`compile_workbook`, `gate::gate`, `accept::accept`/`promote`,
  `emit_bundle`) that the CLI can shell over shallowly. Codex assumes they may not and
  rates HIGH; Gemini assumes Phase 93 hardened them and rates LOW. **Action: confirm the
  facade before Wave 3** — this is verifiable by reading `pmcp-workbook-compiler` re-exports.

### Codex-only concerns (single-reviewer, but specific and worth triaging)
- **Plans 03/04 scope-drift risk (HIGH):** ensure cargo-pmcp does NOT duplicate gate
  fingerprint / corpus-replay / promotion logic. Suggest a hard acceptance criterion:
  "no cargo-pmcp code reimplements gate or promotion logic" (94-03 already has a
  `grep -c ... ≥ 3` library-call assertion, which partially covers this).
- **Gate-before-write feasibility (HIGH):** if the only public compile API writes during
  `compile_workbook`, "gate before any write" needs a candidate-build API that doesn't
  emit/promote first. Verify the seam.
- **Version-discovery seam (HIGH):** `compile_workbook(..., version, ...)` needs a version
  "from the workbook" (D-11) — name the public API that reads it before compile.
- **Stubs in 94-02 (HIGH/MEDIUM):** `compile.rs`/`emit.rs` stubs aren't in `files_modified`,
  and `// STUB:` + "not yet implemented" between waves may trip the zero-SATD / zero-defect
  policy. Either list them in `files_modified` or add enum variants only when handlers land.
- **`make purity-check` inside a cargo test (HIGH):** slow/recursive/flaky — Codex
  recommends it stay an explicit verification/CI step or an `#[ignore]` test, not a normal
  integration test.
- **`cargo test -p cargo-pmcp --lib` target validity (MEDIUM):** confirm cargo-pmcp builds
  a lib target (not binary-only) so the unit-test path resolves; otherwise adjust the test command.
- **`validate()` needs `project_root` (HIGH):** lexical `..`-rejection alone can't enforce
  "out_dir under project root" without the root — pass it in.

### Recommended next step
Most concerns are addressable with **targeted plan edits**, not a replan. Run
`/gsd:plan-phase 94 --reviews` to fold in: (1) the exit-code/dispatcher fix [top priority],
(2) `validate(project_root)` signature, (3) library-facade confirmation + a "no
gate/promotion logic in cargo-pmcp" acceptance criterion for 03/04, (4) stub ownership in
`files_modified`, and (5) downgrading `make purity-check`/TODO to an ignored test or
explicit verification step.
