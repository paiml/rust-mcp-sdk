//! `pmcp-workbook-compiler` — the OFFLINE Excel→MCP compiler.
//!
//! This crate runs the build-time pipeline that turns a governed Excel workbook
//! into a tested, versioned, deterministic served bundle:
//! ingest → lint → manifest synth → formula parse → DAG compile →
//! penny-reconcile → artifact emit → promote-time gate.
//!
//! # The purity boundary (the milestone's #1 trap)
//!
//! This is the ONE crate in the workspace where the Excel reader
//! (`umya-spreadsheet`, plus its transitive `quick-xml`/`zip`) is allowed. The
//! reader is confined to the [`ingest`] and [`provenance`] modules; no umya type
//! leaks across the crate boundary, and the served-tree crates
//! (`pmcp-workbook-runtime`, `pmcp-workbook-dialect`, `pmcp-server-toolkit`)
//! NEVER link it. The Makefile `purity-check` gate POSITIVELY asserts umya IS
//! here and is ABSENT everywhere served.
//!
//! # Re-export, don't re-declare (the keystone)
//!
//! Every shared model/IR/hash/changelog/finding/rounding type is re-exported
//! from [`pmcp_workbook_runtime`] (and the dialect contract from
//! [`pmcp_workbook_dialect`]) — NEVER re-declared. A second copy of
//! `Manifest`/`ChangeClass`/`WHITELIST` would make the served loader and the
//! `diff_version` tool read a DIFFERENT definition than the compiler emits.

// Compiler/clippy-enforced panic-freedom on the library value path (copied
// verbatim from pmcp-workbook-runtime). Test code constructs fixtures freely.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

// ---- Pipeline modules (each a compilable Wave 1 stub; downstream plans fill bodies) ----

/// The compiler's typed error surface ([`CompileError`]).
pub mod error;

/// Ingest stage — the umya-isolated `.xlsx` reader (Plan 02).
pub mod ingest;

/// Dialect linter stage — runs against the SDK dialect contract (Plan 03).
pub mod dialect;

/// Manifest synthesis stage — colour/Guide/header heuristics → roles (Plan 04).
pub mod manifest;

/// Formula parse stage — tokenize + parse to the runtime's owned `Expr` (Plan 05).
pub mod formula;

/// DAG compile stage — build the dependency graph + toposort (Plan 05).
pub mod dag;

/// Sheet-IR eval bridge — drives the runtime's SERVE-time executor (Plan 05).
pub mod sheet_ir;

/// Reconcile stage — grade computed outputs against the cached oracle (Plan 06).
pub mod reconcile;

/// Provenance stage — quarantined raw-parts identity reader (Plan 02).
pub mod provenance;

/// Artifact emission stage — write the served bundle (Plan 07).
pub mod artifact;

/// Change-classification stage — diff a candidate vs the prior baseline (Plan 07).
pub mod change_class;

/// Promote-time governance gate — the build-time approval boundary (Plan 07).
pub mod gate;

/// Stage-1 composed pass — collect-all lint + synth + freshness + drift (Plan 06).
pub mod stage1;

/// Workbook-declared version accessor — the read-only `read_workbook_version`
/// surface the thin-shell CLI reads instead of a `--version` flag (D-02/D-11).
pub mod version;

/// Workbook-declared DIALECT-version accessor (WBDL-02) — a SIBLING of [`version`]
/// reading `pmcp_dialect_version` with absent→baseline (D-05) + a fail-closed
/// semver-compat decision (D-04). PUBLIC so the fuzz target + examples reach the
/// parser. Declared HERE (Plan 96-01 Task 2); the call-site wiring is Task 3.
pub mod dialect_version;

// Producer/consumer golden proof (WBCO-05). In-crate `#[cfg(test)]` so it can
// reach the `#[cfg(test)]`-only `compile_workbook_with_fixture_override`
// (CR-01: the override is unreachable from any publishable feature; an external
// integration test could not see it). Runs via plain `cargo test`.
#[cfg(test)]
mod reemit_golden;

// The WBV2-04 ACCEPTANCE proof (Plan 100-07): a REAL override-compile of the
// Table-authored `template.xlsx` emits exactly TWO tools (calculate_tax +
// estimate_refund) with disjoint populated input_keys + non-empty served schemas.
// In-crate `#[cfg(test)]` for the SAME CR-01 reachability reason as `reemit_golden`:
// it must reach the `#[cfg(test)]`-only `compile_workbook_with_fixture_override`,
// which is invisible to an external `tests/` integration crate. (The frontmatter
// names an external `tests/template_compile_e2e.rs`; that placement is superseded
// by this in-`src` module for reachability — recorded in the SUMMARY.)
#[cfg(test)]
mod template_compile_e2e;

// The reusable `#[cfg(test)]` rust_xlsxwriter fixture author (Plan 96-03 Task 1).
// Lives in `src/` so its self-tests reach the `#[cfg(test)]`-only
// `compile_workbook_with_fixture_override` (same CR-01 reachability reason as
// `reemit_golden`); the WBEX gates (Plans 04/05) author their fixtures through it.
#[cfg(test)]
mod fixture_author;

// WBEX-01 generalization gate (Plan 96-04): a SECOND, non-lighthouse loan/mortgage
// workbook compiles through the generic driver and serves ITS OWN
// get_manifest/tools/list schema behind the SAME five generic tool names. In-crate
// `#[cfg(test)]` for the same CR-01 reachability reason as `reemit_golden`: it must
// reach the `#[cfg(test)]`-only `compile_workbook_with_fixture_override`.
#[cfg(test)]
mod reemit_loan;

// WBEX-02 Excel-quirk reconcile corpus (Plan 96-05): the D-08 layer-2
// (penny-reconcile) partner of the runtime crate's scalar_eval quirk unit tests.
// Each numerically-expressible quirk is a tiny fixture compiled through the
// trusted-fixture override, then graded by retrieving the executor's recomputed
// value + the cached oracle and comparing via the real `reconcile::within_tol`
// penny path. In-crate `#[cfg(test)]` for the same CR-01 reachability reason as
// `reemit_golden`: it must reach the `#[cfg(test)]`-only override.
#[cfg(test)]
mod quirks_reconcile;

// In-crate `#[cfg(test)]` tests for the `prepare_candidate` facade (Task 94-00-02).
// Lives in `src/` (not `tests/`) so it can reach the `#[cfg(test)]`-only
// `prepare_candidate_with_fixture_override` (same CR-01 reachability reason as
// `reemit_golden`): an external integration test cannot see `#[cfg(test)]` items.
#[cfg(test)]
mod prepare_candidate_tests;

pub use error::CompileError;

// ---- Curated re-export surface (re-export the runtime/dialect shared types) ----
//
// These mirror the names `pmcp-workbook-runtime` exports so downstream consumers
// (and Plan 07's driver wiring) get one definition of every shared type. NEVER
// re-declare any of these as a fresh struct/enum here.

// Formula AST + Excel error value set + DAG container + executor value type.
pub use pmcp_workbook_runtime::{toposort, BinOp, CellValue, Dag, ExcelError, Expr, UnOp};

// The version-changelog module (reach `changelog::Severity` via its module path
// to preserve the runtime's `changelog::Severity` vs `finding::Severity`
// collision rule — the bare `Severity` re-exported below stays the lint tier).
pub use pmcp_workbook_runtime::changelog;
pub use pmcp_workbook_runtime::{ChangeClass, OutputDelta, OutputMeta, VersionChangelog};

// The logical manifest projection model the compiler EMITS (lives in the
// runtime; re-export, never re-declare).
pub use pmcp_workbook_runtime::{AnnotationDecl, CellRole, Dtype, InputTier, Manifest, Role};

// The collect-all located lint findings the linter emits (bare `Severity` here
// is the lint-finding tier — the changelog tier stays module-path-only above).
pub use pmcp_workbook_runtime::{LintFinding, LintReport, Severity};

// The bundle artifact model + hashing helpers (NEVER hand-roll the combined
// hash — the served loader recomputes with these).
pub use pmcp_workbook_runtime::{build_bundle_lock, fold_evidence_hash, sha256_hex, BundleLock};

// The fail-closed bundle loader (re-exported so a consumer loads a prior baseline
// through the SAME integrity-verifying path the server uses — NEVER hand-read the
// members + rebuild the DAG, which would feed an UNVERIFIED golden into the
// governance gate). `load_bundle` returns a `WorkbookBundle` only after the frozen
// member set + integrity lock + stamp binding all verify (fail-closed).
pub use pmcp_workbook_runtime::{
    load_bundle, BundleLoadError, BundleSource, BundleSourceError, LocalDirSource, WorkbookBundle,
};

// The single shared MCP tool-name sanitizer (the SAME definition the served
// registration + the compiler's collision lint call) — re-exported so the offline
// `workbook explain` preview sanitizes output-Table names identically (never a
// second copy that could drift from registration).
pub use pmcp_workbook_runtime::sanitize_tool_name;

// The served-key projector (the F3 in_/out_ strip) — re-exported so the offline
// `workbook explain` preview resolves each tool's served input/output key through the
// SAME function the served schema builder uses (it cannot resolve a different key).
pub use pmcp_workbook_runtime::json_key_for_role;

// The served `Tool` artifact shape — re-exported at the compiler root so the offline
// preview maps the production tool list (names + DAG-derived `input_keys` + `outputs`)
// without naming `pmcp-workbook-runtime` directly.
pub use pmcp_workbook_runtime::Tool;

// The reserved meta-tool name set (H3) — the SINGLE source (runtime leaf) the
// reserved-name output-Table gate checks against, so the compiler reads the SAME set
// the served binary registers WITHOUT a compiler→toolkit dep (purity-safe).
pub use pmcp_workbook_runtime::RESERVED_TOOL_NAMES;

// The Excel rounding helpers the reconcile classifier anchors on.
pub use pmcp_workbook_runtime::sheet_ir::rounding::{excel_ceiling, excel_round, excel_roundup};

// The dialect contract the linter runs against (re-export, never re-declare — a
// second WHITELIST copy would defeat the dialect crate's spec-binding test).
pub use pmcp_workbook_dialect::{CandidateRole, DialectRules, WHITELIST};

// The SERVE-time executor surface the compiler-side reconcile drives (the runtime's
// pure-Rust executor — NO SWC/JS oracle; re-export, never re-declare). The O-1
// parity proof depends on the compiler and the server reconciling through ONE path.
pub use pmcp_workbook_runtime::{run_executor, CellEnv, EvalTrace, RunResult};

// The manifest→CellSource wiring seam (93-02 ⋈ 93-03): the real WorkbookMap drives
// the linter/parser/DAG through this adapter.
pub use manifest::WorkbookCellSource;

// The penny-reconcile surface (WBCO-04): the operand-anchored classifier, the D-03
// named-output/helper severity split, and the collect-all driver.
pub use reconcile::classifier::{MismatchClass, MismatchEvidence, BOUNDARY_EPSILON};
pub use reconcile::drift::{is_named_output, mismatch_severity};
pub use reconcile::{
    reconcile as reconcile_oracle, ComparisonMap, GradedMismatch, ReconcileReport,
};

// The bundle-emit surface (WBCO-05/WBGV-07): the seven-member emitter (deterministic
// serialization + bundle_id BUNDLE.lock via the runtime hash helpers + the WR-01
// enum-tier skip) and its evidence/parser-equivalence record. The runtime hash
// helpers (build_bundle_lock/fold_evidence_hash/sha256_hex/BundleLock) are already
// re-exported above from the runtime — NOT re-routed through `artifact` here, so
// there is exactly ONE definition of each.
pub use artifact::{
    build_cell_map, build_layout_descriptor, emit_bundle, parser_equivalence_json, CellEntry,
    CellMap, EmitError, EvidenceInputs, LayoutDescriptor, ParserEquivalence,
};

// The per-Table multi-tool fan-out surface (WBV2-04/05): the OutputTable membership
// type + the build_tools/collision-lint/per-tool-reconcile primitives the
// production driver now wires in (replacing the dead-on-production single-tool path
// — CR-01). Re-exported so the gated-update CLI lane reads the SAME types.
pub use artifact::{
    build_tools, reconcile_tools, tool_name_collision_findings, OutputTable, ToolReconcileReport,
};

// The hash-covered ungated/gated EMIT MARKER channel (WBCL-03 / D-08): the emit
// status travels WITH the artifact. A SELF-CONTAINED additive channel — it does
// NOT enter the served loader's FROZEN evidence fold / allow-set (T-94-00-FROZEN).
pub use artifact::{
    read_gate_marker, write_gate_marker, GateMarker, EVIDENCE_GATE_DIGEST, EVIDENCE_GATE_MARKER,
};

// The change-class surface (WBGV-01/02/03): the symmetric demotion-aware classifier,
// the strictest-policy reducer, the per-class routing policy + block message, the
// canonical IR sub-DAG identity hash, and the output redefinition diff. `ChangeClass`
// / `OutputDelta` / `VersionChangelog` are re-exported above from the runtime (the
// served `diff_version` tool reads the SAME enum) — NEVER re-declared here.
pub use change_class::{
    block_message, classify, diff_outputs, effective_policy, ir_subdag_hash, policy, GatePolicy,
};

// The composed stage-1 pass + its freshness policy (the production driver passes
// `Enforce`; a TEST may pass `TrustedFixture` to admit a committed neutral fixture).
pub use stage1::{run_stage1, FreshnessPolicy, Stage1Output};

// The workbook-declared-version accessor (WBCL-01 / D-02 / D-11): the version the
// bundle is stamped with comes FROM the workbook, never a CLI flag or `pmcp.toml`.
// The thin-shell CLI reads it through this re-export.
pub use version::read_workbook_version;

// The workbook-declared DIALECT-version accessor (WBDL-02 / D-03/D-04/D-05): a
// SIBLING of `read_workbook_version`. `resolve_dialect_version` resolves+validates
// the `pmcp_dialect_version` declaration (absent → baseline, no error; present →
// fail-closed semver-compat). The `pub mod dialect_version` declaration (which
// exposes the public parser to the fuzz target + examples) lives above; this is
// just the convenience re-export of the resolve entry point.
pub use dialect_version::resolve_dialect_version;

use pmcp_workbook_runtime::sheet_ir::{Cell, CellExpr};

/// Compile a governed Excel workbook into a served bundle.
///
/// This is the GENERIC driver that replaces the lighthouse's hardcoded
/// reference-manifest builder (the one surviving §5 gap — WBCO-02): the manifest
/// comes SOLELY from [`manifest::synthesize`] → [`manifest::ratify`], never from a
/// hand-built customer-specific literal, and there is NO hardcoded
/// reference-workbook-path / workflow-name const — `workflow` is a parameter.
///
/// The wired pipeline: read ORIGINAL bytes (provenance anchor) → [`ingest::ingest`]
/// (umya) → [`stage1::run_stage1`] (lint + synth + freshness, collect-all) →
/// [`manifest::ratify`] → parse formulas + [`dag::build_dag`] → run the SHARED
/// runtime executor → [`reconcile::reconcile`] (named-out = ERROR, helper =
/// WARNING) → emit the seven-member bundle into `{workflow}@{version}/`.
///
/// # Arguments
/// * `workbook_path` — the source `.xlsx` to compile.
/// * `out_root` — the bundle output root (one `{workflow}@{version}/` dir).
/// * `workflow` — the workflow/bundle name (NEVER a hardcoded literal — WBCO-02).
/// * `version` — the workbook-declared version (`BUNDLE.lock` version == changelog
///   `to_version`).
/// * `approver` — the human approver recorded in the manifest sign-off.
///
/// # Errors
/// Returns the per-stage [`CompileError`] variant on failure:
/// [`Ingest`](CompileError::Ingest), [`Lint`](CompileError::Lint),
/// [`Reconcile`](CompileError::Reconcile), [`Emit`](CompileError::Emit), or
/// [`Gate`](CompileError::Gate).
pub fn compile_workbook(
    workbook_path: &Path,
    out_root: &Path,
    workflow: &str,
    version: &str,
    approver: &str,
) -> Result<BundleLock, CompileError> {
    compile_workbook_inner(
        workbook_path,
        out_root,
        workflow,
        version,
        approver,
        FreshnessPolicy::Enforce,
    )
}

/// The shared driver body. `freshness` is [`FreshnessPolicy::Enforce`] on the
/// production path ([`compile_workbook`]); only the TEST-ONLY
/// [`compile_workbook_with_fixture_override`] passes
/// [`FreshnessPolicy::TrustedFixture`].
fn compile_workbook_inner(
    workbook_path: &Path,
    out_root: &Path,
    workflow: &str,
    version: &str,
    approver: &str,
    freshness: FreshnessPolicy,
) -> Result<BundleLock, CompileError> {
    // (1) ORIGINAL on-disk bytes (the provenance anchor — NEVER a umya round-trip).
    let bytes = std::fs::read(workbook_path)?;

    // (2) umya ingest → owned WorkbookMap + collect-all ingest findings.
    let (map, ingest_findings) =
        ingest::ingest(workbook_path).map_err(|e| CompileError::Ingest(e.to_string()))?;

    // (2a) Resolve + validate the workbook-declared DIALECT version (WBDL-02).
    // Fail-closed (D-04): a different major OR a newer-than-supported minor is a
    // typed `CompileError::Lint` reported in the same refuse pass as the lint/
    // freshness gate. An ABSENT declaration resolves to the baseline (D-05) with
    // NO error — every existing fixture (which declares no `pmcp_dialect_version`)
    // keeps compiling with zero edits. Run over the ingested `map` (mirroring how
    // `promote_named_outputs` consumes `&map`), BEFORE stage-1, so an
    // incompatible dialect is refused before any synth/reconcile work. This is the
    // SHARED step `prepare_candidate_inner` (the gated-update lane) ALSO runs — both
    // lanes call the one `validate_dialect_version_step` so the D-04 gate cannot
    // drift between them (HI-01).
    dialect_version::validate_dialect_version_step(&map)?;

    // (3) Composed stage-1 pass (lint + synth + freshness), collect-all refuse.
    let stage1 = stage1::run_stage1(&bytes, &map, &ingest_findings, workflow, freshness)?;

    // (3a) Promote the workbook's `out_*` named-range targets to `Role::Output`.
    // Synthesis classifies Input/Constant/Formula from COLOUR alone (it never
    // emits `Role::Output`); the OUTPUT convention is a named-range act the
    // workbook authors (`out_taxable_income → 3_Outputs!B2`). The driver applies
    // it so the manifest carries the declared outputs a served `calculate`
    // requires — without this every emit would fail loud in `build_cell_map`
    // (zero-output manifest). This stays out of `synth` (colour-only) and is the
    // driver's naming-convention responsibility.
    let mut manifest = stage1.synth_manifest;
    promote_named_outputs(&mut manifest, &map);
    // (3b) Name `in_*` named-range INPUT cells so the served input schema carries
    // a stable semantic key (`loan_amount`) rather than the cell's numeric value.
    // The INPUT analogue of the `out_*` convention — naming only, never re-roling.
    name_named_inputs(&mut manifest, &map);
    // (3b-tables) ADDITIVELY promote a TABLE-AUTHORED workbook's harvested Excel
    // Tables (template.xlsx): name input rows + re-role output-Table formula cells to
    // Role::Output. A named-range workbook harvests zero Tables → no-op. This is what
    // lets a Table-authored workbook flow through the SAME refuse/reconcile/emit gates
    // and reach the WBV2-04 per-Table fan-out (CR-01).
    promote_harvested_tables(&mut manifest, &map);

    // Excel commonly persists formatting on the first empty row below a Table.
    // A blue-font, value-less cell there is an OOXML style artifact, not an input.
    // Drop it only after both naming paths had a chance to declare it: a named
    // blank input remains real and is preserved.
    remove_unnamed_blank_inputs(&mut manifest);

    // (3c) Refuse loudly if any input is left without a callable semantic key
    // (no `in_*` named range), or two inputs collide on one served key, or an
    // input's served key is empty/value-shaped. The reconcile/gate stages grade
    // OUTPUTS only, so without this an uncallable value-keyed input would ship
    // silently (F1/H2).
    refuse_uncallable_inputs(&manifest)?;
    // (3d) M4: the OUTPUT mirror — refuse two outputs colliding on one served key,
    // an empty output key, or a value-shaped output key (the served outputSchema +
    // payload would otherwise last-writer-wins silently).
    refuse_uncallable_outputs(&manifest)?;

    // (4) RATIFY the candidate manifest (a recorded sign-off). The sidecar lives
    // beside the output root so the audit trail is co-located with the bundle.
    let workbook_hash = sha256_hex(&bytes);
    let sidecar = out_root.join(format!("{workflow}.ratifications.jsonl"));
    manifest::ratify(&mut manifest, &workbook_hash, approver, &sidecar)
        .map_err(|e| CompileError::Emit(e.to_string()))?;

    // (5) Build the IR + DAG from the parsed formulas (whitelist-at-parse, D-06).
    // The literal cells emitted are the GOVERNED CONSTANTS only (the bracket
    // table) — inputs are seeded at run-time via the cell-map, and decorative
    // text labels never enter the served IR.
    let (ir, dag) = build_ir_and_dag(&map, &manifest)?;

    // (6) Run the SHARED runtime executor over the IR with the input cells seeded
    // from their cached values — the SAME pure-Rust path the served binary uses.
    let seed = seed_from_inputs(&map);
    let run = sheet_ir::eval(&ir, &dag, &seed)
        .map_err(|finding| CompileError::Reconcile(finding.message.clone()))?;

    // (7) Reconcile the computed outputs against the cached oracle (named-output =
    // ERROR, helper = WARNING). A named-output mismatch blocks the emit.
    let comparison = comparison_from_outputs(&map, &manifest);
    let report = reconcile::reconcile(&run.computed, &run.traces, &ir, &comparison, &manifest);
    if report.has_errors() || report.is_hard_fail() {
        return Err(CompileError::Reconcile(format!(
            "{} named-output mismatch(es) against the cached oracle",
            report
                .mismatches
                .iter()
                .filter(|m| m.severity == Severity::Error)
                .count()
        )));
    }

    // (7a) Derive the per-Table OutputTable membership from the harvested Tables +
    // the role-promoted manifest (WBV2-04). A named-range workbook harvests zero
    // Tables → an EMPTY set → the single-tool `build_cell_map` fallback (corpus).
    let output_tables = output_tables_from_harvest(&map, &manifest);
    // (7b) Fold tool-name-collision Errors into the stage-1 gate (T-100-17): two
    // output Tables sanitizing to one MCP name is a cell-precise compile failure
    // BEFORE any bundle is written, not a silent last-writer-wins at served boot.
    refuse_colliding_output_tables(&output_tables)?;
    // (7b-reserved) H3: reject an output Table whose name sanitizes to a RESERVED
    // meta-tool name (explain/get_manifest/diff_version/render_workbook) — it would
    // silently overwrite the built-in meta tool at registration. Same stage-1 gate.
    refuse_reserved_output_table_names(&output_tables)?;
    // (7c) On the multi-tool path, reconcile each derived tool against ITS OWN oracle
    // (WBV2-05/M6). The per-tool oracle is the authored cached-`<v>` map, so the
    // per-tool reconcile genuinely grades the table path (a perturbed cached value
    // blocks). Any per-tool mismatch blocks the emit. The named-range fallback (empty
    // set) keeps the shared comparison_from_outputs reconcile above.
    let output_oracles = output_oracle_map(&map, &manifest);
    reconcile_output_tables(&output_tables, &dag, &manifest, &run, &output_oracles)?;

    // (8) Emit the seven-member bundle through the SEED lane (first version: no
    // prior baseline). The manifest came SOLELY from synth→ratify (no hand-built
    // per-workbook reference manifest on this path — the WBCO-02 generalization).
    let layout = artifact::build_layout_descriptor(&map, &workbook_hash);
    let changelog = VersionChangelog {
        from_version: String::new(),
        to_version: version.to_string(),
        deltas: vec![],
        summary: format!("seed {version}"),
    };
    let inputs = gate::accept::PromoteInputs {
        bundle_id: workflow,
        version,
        ir: &ir,
        manifest: &manifest,
        layout: &layout,
        changelog: &changelog,
        parser_equivalence: &stage1.parser_equivalence,
        workbook_hash,
        // The harvest-derived per-Table membership (WBV2-04): NON-EMPTY → the
        // build_tools fan-out; EMPTY (named-range corpus) → single-tool fallback.
        output_tables: &output_tables,
        dag: &dag,
    };
    let (lock, _dir) = gate::accept::promote(&gate::accept::EmitLane::Seed, out_root, &inputs)
        .map_err(|e| CompileError::Emit(e.to_string()))?;
    Ok(lock)
}

/// TEST-ONLY: compile a committed neutral fixture, honouring its trusted-fixture
/// provenance override so the in-crate producer/consumer golden proof can run
/// against a workbook authored WITHOUT a genuine Excel save (its `fullCalcOnLoad`
/// staleness signal is demoted to a Warning). Reachable ONLY under `cfg(test)`
/// (CR-01: there is NO publishable feature that arms this — a default or
/// `--all-features` build of the crate as a dependency neither compiles nor
/// exposes this symbol). The production [`compile_workbook`] always passes
/// [`FreshnessPolicy::Enforce`], so the same bytes are REFUSED on the production
/// path (the override cannot weaken production refusal).
#[cfg(test)]
fn compile_workbook_with_fixture_override(
    workbook_path: &Path,
    out_root: &Path,
    workflow: &str,
    version: &str,
    approver: &str,
) -> Result<BundleLock, CompileError> {
    compile_workbook_inner(
        workbook_path,
        out_root,
        workflow,
        version,
        approver,
        FreshnessPolicy::TrustedFixture,
    )
}

/// The READ-ONLY served tool-surface projection of a workbook (H1): the production
/// [`Manifest`] (with the SAME harvest/named-range role promotions the compile path
/// applies) plus the production [`Tool`] list [`build_tools`] derives. This is what
/// the offline `workbook explain` preview drives, so the preview CANNOT diverge from
/// the served surface by construction (it runs the SAME pre-emit pipeline functions).
#[derive(Debug, Clone)]
pub struct ToolSurfaceProjection {
    /// The role-promoted manifest (the input/output `CellRole`s the served schema
    /// resolves dtype/unit/enum from, by `json_key`/`seed_coord`).
    pub manifest: Manifest,
    /// The production per-Table tools — each with its DAG-derived `input_keys`
    /// (`upstream_input_leaves`, ranges + cross-sheet refs native) and `outputs`
    /// (`CellEntry` carrying the stripped `json_key`).
    pub tools: Vec<Tool>,
}

/// Project a workbook's SERVED tool surface WITHOUT writing any bundle or running
/// any compile gate (H1) — the production projection the `workbook explain` preview
/// drives so it cannot lie about what the served binary registers.
///
/// Runs the EXACT pre-emit pipeline [`compile_workbook_inner`] uses, then STOPS
/// before ratify/reconcile/emit: [`ingest::ingest`] → [`stage1::run_stage1`] (synth)
/// → [`promote_named_outputs`] → [`name_named_inputs`] → [`promote_harvested_tables`]
/// → [`build_ir_and_dag`] → [`output_tables_from_harvest`] → [`build_tools`]. The
/// per-tool inputs come from `build_tools`' DAG-derived `input_keys`
/// ([`upstream_input_leaves`](pmcp_workbook_runtime::upstream_input_leaves)); the
/// per-tool input/output keys are the stripped served keys
/// ([`json_key_for_role`](pmcp_workbook_runtime::json_key_for_role)); the tool names
/// are the raw output-Table names (the caller sanitizes via
/// [`sanitize_tool_name`](pmcp_workbook_runtime::sanitize_tool_name)).
///
/// Uses [`FreshnessPolicy::Preview`] so a workbook whose cached oracle values are
/// flagged stale (`fullCalcOnLoad`) is still previewable: the preview is read-only —
/// it writes nothing, grades NO oracle value, and runs NO emit/promote gate — so the
/// "cached values not trusted" staleness signal is irrelevant to the structural tool
/// surface. `Preview` is production-safe (only this read-only projection constructs
/// it; the compile/emit path always enforces the oracle-trust refusal).
///
/// # Errors
/// Returns the per-stage [`CompileError`] on a malformed workbook (ingest/synth/parse
/// failure), or [`CompileError::Emit`] if a Table-authored workbook with output
/// Tables fails [`build_tools`] (an unmappable derived membership). A NAMED-RANGE
/// (zero-Table) workbook yields a single fallback tool wrapping all outputs.
pub fn project_tool_surface_from_workbook(
    workbook_path: &Path,
) -> Result<ToolSurfaceProjection, CompileError> {
    // (1) ORIGINAL bytes + (2) umya ingest — the SAME read the compile path runs.
    let bytes = std::fs::read(workbook_path)?;
    let (map, ingest_findings) =
        ingest::ingest(workbook_path).map_err(|e| CompileError::Ingest(e.to_string()))?;

    // (2a) Dialect-version gate + (3) stage-1 synth — the SAME pre-emit steps. The
    // preview is read-only, so it honours TrustedFixture freshness (a dry-run preview
    // of an un-saved workbook must not be blocked by the staleness signal).
    dialect_version::validate_dialect_version_step(&map)?;
    let stage1 = stage1::run_stage1(
        &bytes,
        &map,
        &ingest_findings,
        "explain-preview",
        FreshnessPolicy::Preview,
    )?;

    // (3a-3b-tables) The SAME role promotions the compile path applies.
    let mut manifest = stage1.synth_manifest;
    promote_named_outputs(&mut manifest, &map);
    name_named_inputs(&mut manifest, &map);
    promote_harvested_tables(&mut manifest, &map);

    // (5) Build the DAG (the per-tool `upstream_input_leaves` source) and (7a) the
    // per-Table membership, then (7) build_tools — the production served surface.
    let (_ir, dag) = build_ir_and_dag(&map, &manifest)?;
    let output_tables = output_tables_from_harvest(&map, &manifest);
    let tools = tools_for_surface(&manifest, &dag, &output_tables)?;

    Ok(ToolSurfaceProjection { manifest, tools })
}

/// Build the production [`Tool`] list for the preview projection: the multi-tool
/// [`build_tools`] fan-out when the workbook harvested ≥1 output Table, else the
/// single-tool [`build_cell_map`] fallback wrapping all outputs (the named-range
/// corpus path — the SAME fallback `emit_bundle` routes an empty set to). Kept
/// separate so [`project_tool_surface_from_workbook`] stays a thin pipeline (cog ≤25).
fn tools_for_surface(
    manifest: &Manifest,
    dag: &Dag,
    output_tables: &[OutputTable],
) -> Result<Vec<Tool>, CompileError> {
    if output_tables.is_empty() {
        // Named-range corpus: no harvested Table → the single-tool fallback.
        return Ok(build_cell_map(manifest).map_err(CompileError::Emit)?.tools);
    }
    // The read-only preview grades NO oracle, so pass an empty oracle map (M6: the
    // cached-`<v>` oracle is a compile-time reconcile artifact, irrelevant to the
    // structural surface the preview projects).
    let empty_oracles = BTreeMap::new();
    let (tools, _lints) =
        build_tools(manifest, dag, output_tables, &empty_oracles).map_err(CompileError::Emit)?;
    Ok(tools)
}

/// Build the executable IR (`{cell_key -> Cell}`) and its dependency [`Dag`] from
/// the ingested [`ingest::WorkbookMap`].
///
/// Every formula cell parses to a [`CellExpr::Formula`] whose `Expr::Ref` nodes are
/// rebased to fully-qualified `cell_key`s (so the executor resolves cross-sheet
/// references). For NON-formula cells, ONLY the manifest's `Role::Constant`
/// (governed) cells enter the IR as a [`CellExpr::Literal`]: inputs are seeded at
/// run-time via the cell-map and decorative text labels never enter the served IR
/// (matching the frozen golden's IR shape — formula cells + governed constants).
/// The DAG is reconstructed SOLELY from the parsed references (never
/// `calcChain.xml`).
fn build_ir_and_dag(
    map: &ingest::WorkbookMap,
    manifest: &Manifest,
) -> Result<(HashMap<String, Cell>, Dag), CompileError> {
    use pmcp_workbook_runtime::range_ref::cell_key;
    let mut ir: HashMap<String, Cell> = HashMap::new();
    let mut parsed: Vec<dag::ParsedCell> = Vec::new();

    // The governed-constant cells that may enter the IR as literals.
    let governed: std::collections::HashSet<&str> = manifest
        .cells
        .iter()
        .filter(|c| matches!(c.role, Role::Constant))
        .map(|c| c.cell.as_str())
        .collect();

    for sheet in &map.sheets {
        for cell in &sheet.cells {
            let key = cell_key(&sheet.name, &cell.addr);
            if let Some(formula) = &cell.formula {
                let expanded = formula::structured_ref::expand_structured_references(formula, map)
                    .map_err(|e| CompileError::Lint(format!("structured reference {key}: {e}")))?;
                let expr = formula::parse(&expanded, &sheet.name, &cell.addr)
                    .map_err(|e| CompileError::Lint(format!("parse {key}: {e}")))?;
                parsed.push(dag::ParsedCell {
                    sheet: sheet.name.clone(),
                    addr: cell.addr.clone(),
                    expr: expr.clone(),
                });
                let rebased = rebase_refs(&expr, &sheet.name);
                ir.insert(
                    key.clone(),
                    Cell {
                        key,
                        expr: CellExpr::Formula(rebased),
                    },
                );
            } else if governed.contains(key.as_str()) {
                if let Some(value) = &cell.value {
                    let lit = parse_cell_value(value);
                    ir.insert(
                        key.clone(),
                        Cell {
                            key,
                            expr: CellExpr::Literal(lit),
                        },
                    );
                }
            }
        }
    }

    // No synthetic defined names on the generic path (D-07: names resolve only
    // when the workbook authors them; the neutral fixture uses cell refs).
    let names: Vec<dialect::DefinedName> = Vec::new();
    let (dag, _order) =
        dag::build_dag(&parsed, &names).map_err(|e| CompileError::Lint(e.to_string()))?;
    Ok((ir, dag))
}

/// Rebase every [`Expr::Ref`] in `expr` to a fully-qualified `cell_key` against
/// `current_sheet` so the executor resolves a bare `B2` as `{sheet}!B2` and keeps
/// a cross-sheet `1_Inputs!B2` unchanged (the IR refs match the DAG node keys).
fn rebase_refs(expr: &Expr, current_sheet: &str) -> Expr {
    use pmcp_workbook_runtime::resolve::split_ref;
    match expr {
        Expr::Ref(reference) => {
            let (sheet, addr) = split_ref(reference, current_sheet);
            Expr::Ref(pmcp_workbook_runtime::range_ref::cell_key(&sheet, &addr))
        },
        Expr::BinaryOp { left, op, right } => Expr::BinaryOp {
            left: Box::new(rebase_refs(left, current_sheet)),
            op: *op,
            right: Box::new(rebase_refs(right, current_sheet)),
        },
        Expr::UnaryOp { op, operand } => Expr::UnaryOp {
            op: *op,
            operand: Box::new(rebase_refs(operand, current_sheet)),
        },
        Expr::Call { name, args } => Expr::Call {
            name: name.clone(),
            args: args.iter().map(|a| rebase_refs(a, current_sheet)).collect(),
        },
        // Ranges/names/literals pass through unchanged (the neutral fixture uses
        // only scalar cell refs; range rebasing is the DAG's concern).
        other => other.clone(),
    }
}

/// Parse a cached cell-value string into a [`CellValue`]: a parseable number is
/// [`CellValue::Number`], `TRUE`/`FALSE` is [`CellValue::Bool`], anything else is
/// [`CellValue::Text`]. An empty string is [`CellValue::Empty`].
fn parse_cell_value(raw: &str) -> CellValue {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return CellValue::Empty;
    }
    if let Ok(n) = trimmed.parse::<f64>() {
        return CellValue::Number(n);
    }
    match trimmed.to_ascii_uppercase().as_str() {
        "TRUE" => CellValue::Bool(true),
        "FALSE" => CellValue::Bool(false),
        _ => CellValue::Text(trimmed.to_string()),
    }
}

/// Seed the executor [`CellEnv`] from every non-formula literal cell in the
/// workbook map — the leaves (inputs, constants, governed bracket-table
/// literals) the executor needs pre-loaded before it walks the formula DAG.
fn seed_from_inputs(map: &ingest::WorkbookMap) -> CellEnv {
    let mut value_by_key: HashMap<String, CellValue> = HashMap::new();
    for sheet in &map.sheets {
        for cell in &sheet.cells {
            if cell.is_formula {
                continue;
            }
            if let Some(value) = &cell.value {
                let key = pmcp_workbook_runtime::range_ref::cell_key(&sheet.name, &cell.addr);
                value_by_key.insert(key, parse_cell_value(value));
            }
        }
    }

    // Seed every non-formula literal cell the executor depends on: the
    // manifest's `Input`/`Constant` cells AND any governed literal the manifest
    // did not role (e.g. a bracket-table constant) so the DAG's leaf cells
    // resolve. `value_by_key` is already the superset of both, so one pass covers
    // it.
    let mut env = CellEnv::new();
    for (key, value) in &value_by_key {
        env = env.seed_cell(key, value);
    }
    env
}

/// Build the reconcile [`reconcile::ComparisonMap`] from the manifest's
/// `Role::Output` cells, taking each output's cached value (the oracle the
/// reconcile stage grades the computed output against).
fn comparison_from_outputs(
    map: &ingest::WorkbookMap,
    manifest: &Manifest,
) -> reconcile::ComparisonMap {
    let mut value_by_key: HashMap<String, CellValue> = HashMap::new();
    for sheet in &map.sheets {
        for cell in &sheet.cells {
            if let Some(value) = &cell.value {
                let key = pmcp_workbook_runtime::range_ref::cell_key(&sheet.name, &cell.addr);
                value_by_key.insert(key, parse_cell_value(value));
            }
        }
    }

    let mut comparison = reconcile::ComparisonMap::new();
    for role in &manifest.cells {
        if matches!(role.role, Role::Output) {
            if let Some(value) = value_by_key.get(&role.cell) {
                comparison = comparison.with_value(&role.cell, value.clone());
            }
        }
    }
    comparison
}

/// The authored cached-`<v>` ORACLE map for the per-tool reconcile (M6): each
/// `Role::Output` cell's cached value, keyed by its fully-qualified CELL key — the
/// SAME cached values [`comparison_from_outputs`] grades the shared named-output
/// reconcile against, projected into the `{cell_key -> CellValue}` shape
/// [`build_tools`] takes as `output_oracles`. Threading this in makes each tool's
/// oracle NON-empty so the per-tool reconcile genuinely grades the table path (a
/// perturbed cached value blocks the emit), no longer a vacuous net.
fn output_oracle_map(
    map: &ingest::WorkbookMap,
    manifest: &Manifest,
) -> BTreeMap<String, CellValue> {
    let mut value_by_key: HashMap<String, CellValue> = HashMap::new();
    for sheet in &map.sheets {
        for cell in &sheet.cells {
            if let Some(value) = &cell.value {
                let key = pmcp_workbook_runtime::range_ref::cell_key(&sheet.name, &cell.addr);
                value_by_key.insert(key, parse_cell_value(value));
            }
        }
    }

    let mut oracles = BTreeMap::new();
    for role in &manifest.cells {
        if matches!(role.role, Role::Output) {
            if let Some(value) = value_by_key.get(&role.cell) {
                oracles.insert(role.cell.clone(), value.clone());
            }
        }
    }
    oracles
}

/// Promote every `out_*` named-range target cell in `manifest` to [`Role::Output`].
///
/// Synthesis classifies cells from COLOUR alone and never emits [`Role::Output`];
/// the OUTPUT convention is a named-range the WORKBOOK authors (`out_<name>`
/// targeting a single result cell). For each workbook defined name whose name
/// starts with `out_` and whose target is a single cell, this re-roles the matching
/// manifest cell to [`Role::Output`] and records the named-range `name` (the
/// cell-map `json_key` source). A `Role::Output` cell with no synthesized
/// counterpart (e.g. an output formula cell synthesis classified as
/// [`Role::Formula`]) is re-roled in place; an unmatched name is ignored (a
/// defined name pointing nowhere is a linter concern, not a hard failure here).
fn promote_named_outputs(manifest: &mut Manifest, map: &ingest::WorkbookMap) {
    use pmcp_workbook_runtime::range_ref::cell_key;
    for dn in &map.defined_names {
        if !dn.name.starts_with("out_") {
            continue;
        }
        // Single-cell target only: start == end (a range output is not a scalar
        // named output and is left for the linter to flag).
        if dn.target.start != dn.target.end {
            continue;
        }
        let key = cell_key(&dn.target.sheet, &dn.target.start);
        if let Some(role) = manifest.cells.iter_mut().find(|c| c.cell == key) {
            role.role = Role::Output;
            role.name = Some(dn.name.clone());
        }
    }
}

/// Name every `in_*` named-range target cell that synthesis already classified
/// [`Role::Input`] (the blue-font convention) with the named-range identifier.
///
/// Synthesis classifies a cell's ROLE from colour alone but never assigns a
/// semantic `name`; for an INPUT cell (a bare numeric leaf) the cell-map
/// [`json_key_for_role`](pmcp_workbook_runtime::json_key_for_role) precedence
/// (`name → meaning → cell key`) would otherwise fall through to the cell's own
/// numeric VALUE string — a meaningless served input key (`"240000"`). The `in_<name>`
/// named range is the INPUT analogue of the proven `out_<name>` output convention
/// (see [`promote_named_outputs`]): the WORKBOOK author declares it so the served
/// `calculate`/`explain` input schema carries a stable semantic key (`loan_amount`),
/// exactly as the output side does. This NEVER changes a cell's role (an `in_*`
/// name targeting a non-Input cell is ignored — naming is not re-roling); it only
/// records the `name` the cell-map `json_key` reads. An unmatched name is ignored
/// (a defined name pointing nowhere is a linter concern, not a hard failure).
fn name_named_inputs(manifest: &mut Manifest, map: &ingest::WorkbookMap) {
    use pmcp_workbook_runtime::range_ref::cell_key;
    for dn in &map.defined_names {
        if !dn.name.starts_with("in_") {
            continue;
        }
        // Single-cell target only (a range input is not a scalar named input).
        if dn.target.start != dn.target.end {
            continue;
        }
        let key = cell_key(&dn.target.sheet, &dn.target.start);
        if let Some(role) = manifest
            .cells
            .iter_mut()
            .find(|c| c.cell == key && c.role == Role::Input)
        {
            role.name = Some(dn.name.clone());
        }
    }
}

/// Promote a TABLE-AUTHORED workbook's harvested Excel Tables into manifest roles
/// (WBV2-04 — the ADDITIVE sibling of [`promote_named_outputs`]/[`name_named_inputs`]).
///
/// A Table-authored workbook (the §7 `template.xlsx`) declares NO `out_*`/`in_*`
/// named ranges; its inputs/outputs live as named Excel Tables
/// (`name | value | description [| tier]`). Synthesis classifies the `value` cells
/// from colour alone — inputs as [`Role::Input`] (no `name`), output formulas as
/// [`Role::Formula`] — so WITHOUT this step a Table-authored workbook never carries
/// a callable input key or a single [`Role::Output`], and emits ONE empty-input tool.
///
/// For each harvested [`ingest::TableRecord`] this:
/// - **Inputs Table** (one whose `value` cells are already `Role::Input`/`Constant`):
///   sets each input cell's `name` from the row's `name` column (so the served key
///   is `income`, not the cell value) — the Table analogue of `name_named_inputs`.
/// - **Output Table** (one whose `value` cells are `Role::Formula`): re-roles each
///   `value` cell to [`Role::Output`] and names it from the `name` column — the
///   Table analogue of `promote_named_outputs`.
///
/// A named-range workbook harvests ZERO `table_records`, so this is a no-op there
/// (the corpus keeps flowing through the named-range promotions unchanged).
fn promote_harvested_tables(manifest: &mut Manifest, map: &ingest::WorkbookMap) {
    for sheet in &map.sheets {
        for table in &sheet.table_records {
            promote_one_harvested_table(manifest, sheet, table);
        }
    }
}

/// Promote ONE harvested Table's `value`-column body cells into manifest roles
/// (kept separate so [`promote_harvested_tables`] stays a thin loop, cog ≤25). The
/// `value` column is the SECOND column of the Table area (col `area.start.col + 1`);
/// body rows run from the row below the header (`area.start.row + 1`) to
/// `area.end.row`. The `name` column is the FIRST (`area.start.col`).
fn promote_one_harvested_table(
    manifest: &mut Manifest,
    sheet: &ingest::SheetRecord,
    table: &ingest::TableRecord,
) {
    let Some((name_col, header_row)) = split_a1_col_row(&table.area.start) else {
        return;
    };
    let Some((_, end_row)) = split_a1_col_row(&table.area.end) else {
        return;
    };
    let value_col = next_col(&name_col);

    for body_row in (header_row + 1)..=end_row {
        let value_key = pmcp_workbook_runtime::range_ref::cell_key(
            &sheet.name,
            &format!("{value_col}{body_row}"),
        );
        let name_addr = format!("{name_col}{body_row}");
        let row_name = cell_value_text(sheet, &name_addr);

        let Some(role) = manifest.cells.iter_mut().find(|c| c.cell == value_key) else {
            continue;
        };
        match role.role {
            // An output formula cell → re-role to Output + name from the row.
            Role::Formula => {
                role.role = Role::Output;
                if role.name.is_none() {
                    role.name = row_name;
                }
            },
            // An already-classified input/constant → just attach the semantic name
            // (naming is never re-roling — the input analogue of name_named_inputs).
            Role::Input | Role::Constant => {
                if role.name.is_none() {
                    role.name = row_name;
                }
            },
            Role::Output => {},
        }
    }
}

/// The trimmed text of a sheet cell by A1 address (`None` when absent/blank).
fn cell_value_text(sheet: &ingest::SheetRecord, addr: &str) -> Option<String> {
    sheet
        .cells
        .iter()
        .find(|c| c.addr == addr)
        .and_then(|c| c.value.as_deref())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Remove cells that carry only the input colour style and no callable/data
/// evidence. Excel retains such cells below resized Tables (for example a blank
/// blue-font `B19` immediately after an `A3:D18` Inputs Table). Treating that
/// formatting residue as a real input creates a phantom
/// `manifest/input-no-semantic-key` failure.
///
/// This runs after named-range and Table promotion. Consequently a deliberately
/// blank input with a semantic name is retained; only `Role::Input` cells with no
/// name and no non-whitespace value are removed.
fn remove_unnamed_blank_inputs(manifest: &mut Manifest) {
    manifest.cells.retain(|role| {
        let blank = role
            .meaning
            .as_deref()
            .map(str::trim)
            .map(str::is_empty)
            .unwrap_or(true);
        !(role.role == Role::Input && role.name.is_none() && blank)
    });
}

/// Split an A1 address into `(column-letters, 1-based row)` — e.g. `"B11"` →
/// `("B", 11)`. Mirrors `template_harvest_e2e::split_a1`.
fn split_a1_col_row(addr: &str) -> Option<(String, u32)> {
    let split = addr.find(|c: char| c.is_ascii_digit())?;
    if split == 0 {
        return None;
    }
    let (col, row) = addr.split_at(split);
    Some((col.to_string(), row.parse().ok()?))
}

/// The next column letter after `col` (`"A"` → `"B"`, `"Z"` → `"AA"`) — bijective
/// base-26 (`A`=1). The Table `value` column is one right of the `name` column.
fn next_col(col: &str) -> String {
    let mut n: u32 = 0;
    for ch in col.bytes() {
        n = n * 26 + u32::from(ch.to_ascii_uppercase().wrapping_sub(b'A') + 1);
    }
    n += 1; // the NEXT column
    index_to_col(n)
}

/// Convert a 1-based column index into its A1 letter run (`1` → `"A"`, `27` → `"AA"`).
fn index_to_col(mut index: u32) -> String {
    let mut letters = Vec::new();
    while index > 0 {
        let rem = ((index - 1) % 26) as u8;
        letters.push(b'A' + rem);
        index = (index - 1) / 26;
    }
    letters.reverse();
    String::from_utf8(letters).unwrap_or_default()
}

/// Derive the per-Table [`OutputTable`] membership (WBV2-04) from the harvested
/// `table_records` + the (already role-promoted) manifest: one [`OutputTable`] per
/// Table that contributes ≥1 [`Role::Output`] cell, named by the Table's ListObject
/// `name`, described by the caption one row above the Table area, with `output_cells`
/// = the Table's `value`-column cells present in the manifest as [`Role::Output`].
///
/// A Table whose body cells are all inputs (the `Inputs` Table) contributes ZERO
/// output cells and is SKIPPED. A named-range workbook (zero `table_records`) yields
/// an EMPTY `Vec` → the single-tool `build_cell_map` fallback (the corpus path).
fn output_tables_from_harvest(map: &ingest::WorkbookMap, manifest: &Manifest) -> Vec<OutputTable> {
    let output_keys: std::collections::HashSet<&str> = manifest
        .cells
        .iter()
        .filter(|c| matches!(c.role, Role::Output))
        .map(|c| c.cell.as_str())
        .collect();

    let mut tables = Vec::new();
    for sheet in &map.sheets {
        for table in &sheet.table_records {
            let output_cells = output_cells_in_table(sheet, table, &output_keys);
            if output_cells.is_empty() {
                continue; // an all-input Table (Inputs) exposes no tool.
            }
            tables.push(OutputTable {
                name: table.name.clone(),
                description: caption_above_table(sheet, table),
                output_cells,
            });
        }
    }
    tables
}

/// The fully-qualified `value`-column cell keys inside `table`'s area that the
/// manifest carries as [`Role::Output`] (the Table's served output cells). Kept
/// separate so [`output_tables_from_harvest`] stays a thin loop (cog ≤25).
fn output_cells_in_table(
    sheet: &ingest::SheetRecord,
    table: &ingest::TableRecord,
    output_keys: &std::collections::HashSet<&str>,
) -> Vec<String> {
    let Some((name_col, header_row)) = split_a1_col_row(&table.area.start) else {
        return Vec::new();
    };
    let Some((_, end_row)) = split_a1_col_row(&table.area.end) else {
        return Vec::new();
    };
    let value_col = next_col(&name_col);
    let mut cells = Vec::new();
    for body_row in (header_row + 1)..=end_row {
        let key = pmcp_workbook_runtime::range_ref::cell_key(
            &sheet.name,
            &format!("{value_col}{body_row}"),
        );
        if output_keys.contains(key.as_str()) {
            cells.push(key);
        }
    }
    cells
}

/// The caption cell directly above a Table area (§4: caption = tool description) —
/// the cell in the Table's first column, one row above its header row. Mirrors
/// `template_harvest_e2e::caption_above`.
fn caption_above_table(sheet: &ingest::SheetRecord, table: &ingest::TableRecord) -> Option<String> {
    let (col, header_row) = split_a1_col_row(&table.area.start)?;
    if header_row <= 1 {
        return None;
    }
    cell_value_text(sheet, &format!("{col}{}", header_row - 1))
}

/// Fold tool-name-collision Errors into the stage-1 refuse gate (T-100-17): if any
/// derived [`OutputTable`] sanitizes to a colliding or unmappable MCP tool name,
/// surface it as a cell-precise [`CompileError::Lint`] BEFORE any bundle write
/// (the SAME aggregate render `refuse_uncallable_inputs` uses). An empty set (the
/// named-range fallback) never collides.
///
/// # Errors
/// Returns [`CompileError::Lint`] if [`tool_name_collision_findings`] reports any
/// `Severity::Error` finding.
fn refuse_colliding_output_tables(output_tables: &[OutputTable]) -> Result<(), CompileError> {
    let findings = tool_name_collision_findings(output_tables);
    let errors: Vec<&LintFinding> = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .collect();
    if errors.is_empty() {
        return Ok(());
    }
    Err(CompileError::Lint(stage1::render_aggregate(&errors)))
}

/// Reconcile each derived per-Table tool against ITS OWN output-cell oracle
/// (WBV2-05) on the multi-tool path. Builds the tools via [`build_tools`], converts
/// the run's computed values to the `&BTreeMap` [`reconcile_tools`] takes, and
/// blocks the emit ([`CompileError::Reconcile`]) on any per-tool mismatch. An EMPTY
/// `output_tables` set (the named-range fallback) is a no-op — that path keeps the
/// shared `comparison_from_outputs` reconcile.
///
/// # Errors
/// Returns [`CompileError::Reconcile`] on any per-tool oracle mismatch, or
/// [`CompileError::Emit`] if [`build_tools`]/[`reconcile_tools`] fail (a malformed
/// derived membership — e.g. an unmappable tool name slipping past the collision
/// gate).
fn reconcile_output_tables(
    output_tables: &[OutputTable],
    dag: &Dag,
    manifest: &Manifest,
    run: &RunResult,
    output_oracles: &BTreeMap<String, CellValue>,
) -> Result<(), CompileError> {
    if output_tables.is_empty() {
        return Ok(());
    }
    // M6: pass the authored cached-`<v>` oracle map so each tool's oracle is NON-empty
    // — the per-tool reconcile now genuinely grades the table path (a perturbed cached
    // value blocks the emit), no longer a vacuous net.
    let (tools, _lints) =
        build_tools(manifest, dag, output_tables, output_oracles).map_err(CompileError::Emit)?;
    // RunResult exposes `computed: HashMap<String, CellValue>`; reconcile_tools takes
    // a `&BTreeMap` — convert explicitly (there is no `computed_as_btreemap`).
    let computed: BTreeMap<String, CellValue> = run
        .computed
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let report = reconcile_tools(&computed, &tools).map_err(CompileError::Emit)?;
    if report.any_mismatch() {
        return Err(CompileError::Reconcile(report.render()));
    }
    Ok(())
}

/// Validate that EVERY `Role::Input` cell carries a callable semantic key, pushing
/// one `Severity::Error` [`LintFinding`] per defect into `report`.
///
/// This MUST run AFTER [`name_named_inputs`] (so a legitimately-named input has its
/// `name` set) and AFTER F3's [`json_key_for_role`] stripping is in effect (so the
/// served key is what a caller actually sees). Without it a `Role::Input` lacking an
/// `in_*` named range keeps `name: None` and [`json_key_for_role`] falls through to
/// the cell's `meaning` (= the cell VALUE) — a value-keyed, uncallable input the
/// reconcile/gate stages never catch (they grade OUTPUTS only). Three distinct
/// defects are flagged, each a blocking Error:
///
/// (a) an input with NO semantic `name` (no `in_*` named range);
/// (b) two inputs whose served `json_key`s COLLIDE (after prefix stripping);
/// (c) an input whose served `json_key` is empty/whitespace.
///
/// Findings feed the same [`LintReport::has_errors`] gate the rest of stage-1 uses,
/// so a defective bundle is REFUSED rather than shipped uncallable.
fn validate_input_keys(manifest: &Manifest, report: &mut LintReport) {
    use std::collections::BTreeMap;

    let mut by_key: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for role in manifest.cells.iter().filter(|c| c.role == Role::Input) {
        if role.name.is_none() {
            report.push(unnamed_input_finding(&role.cell));
            continue;
        }
        let key = json_key_for_role(role);
        if key.trim().is_empty() {
            report.push(empty_input_key_finding(&role.cell));
            continue;
        }
        // H2: a value-shaped served key (a bare number like `60000`) is the exact
        // original F1 symptom — a BA typed a VALUE in the `name` column, so the served
        // input key degenerates to that number. Reject it with a cell-precise Error.
        if is_value_shaped_key(&key) {
            report.push(value_shaped_input_key_finding(&role.cell, &key));
            continue;
        }
        by_key.entry(key).or_default().push(role.cell.clone());
    }

    for (key, coords) in by_key.iter().filter(|(_, c)| c.len() > 1) {
        report.push(duplicate_input_key_finding(key, coords));
    }
}

/// Whether a served `json_key` is VALUE-SHAPED — a finite numeric literal (integer or
/// decimal, optional leading sign): `60000`, `1.5`, `-3` are value-shaped; a valid
/// identifier (`income`, `q1_2024`) is NOT (H2/M4). The rule is NARROW (numeric-only,
/// finite) — it deliberately does NOT reject "any key starting with a digit" so a
/// legitimate identifier is never false-flagged. `NaN`/`inf` text is not value-shaped
/// (those would be a different defect, and never a meaningful served key).
fn is_value_shaped_key(key: &str) -> bool {
    let trimmed = key.trim();
    !trimmed.is_empty() && trimmed.parse::<f64>().map(f64::is_finite).unwrap_or(false)
}

/// (d) The blocking finding for a VALUE-SHAPED input served key (H2): a numeric
/// `name` (e.g. `60000`) yields a value-keyed, uncallable input — the original F1
/// symptom. Located at the offending Inputs Table row's `name` cell.
fn value_shaped_input_key_finding(cell: &str, key: &str) -> LintFinding {
    let (sheet, addr) = split_cell_key(cell);
    LintFinding::new(
        Severity::Error,
        "manifest/input-value-shaped-key",
        sheet,
        addr,
        format!(
            "input cell {cell} resolves to the value-shaped served key `{key}`; a numeric \
             name is a VALUE, not a callable identifier — the tool input is uncallable"
        ),
        format!(
            "replace the numeric `name` of the Inputs Table row at {cell} with a stable \
             identifier (e.g. `income`) so the served input carries a usable key"
        ),
    )
}

/// Split a fully-qualified `sheet!addr` cell key into `(sheet, Some(addr))` for a
/// located [`LintFinding`]; a key without `!` becomes `(key, None)`.
fn split_cell_key(cell: &str) -> (&str, Option<String>) {
    match cell.split_once('!') {
        Some((sheet, addr)) => (sheet, Some(addr.to_string())),
        None => (cell, None),
    }
}

/// (a) The blocking finding for a `Role::Input` with no semantic key (WBV2-05 §8,
/// F1 reshaped to a table-ROW lint — the repair points at the Inputs Table row's
/// `name` cell, NOT an `in_*` named range).
fn unnamed_input_finding(cell: &str) -> LintFinding {
    let (sheet, addr) = split_cell_key(cell);
    LintFinding::new(
        Severity::Error,
        "manifest/input-no-semantic-key",
        sheet,
        addr,
        format!(
            "input cell {cell} has no semantic name; without one the served input key \
             degenerates to the cell value and the tool is uncallable"
        ),
        format!(
            "fill the `name` column of the Inputs Table row at {cell} with a stable \
             identifier (e.g. `amount`) so the served tool input carries a usable key"
        ),
    )
}

/// (b) The blocking finding for two+ inputs that collide on one served `json_key`
/// (WBV2-05 §8, F1 reshaped — the repair names the Table rows, not `in_*` ranges).
fn duplicate_input_key_finding(key: &str, coords: &[String]) -> LintFinding {
    let joined = coords.join(", ");
    LintFinding::new(
        Severity::Error,
        "manifest/input-key-collision",
        "manifest",
        None,
        format!(
            "input cells {joined} all map to the same served key `{key}`; a caller \
             could not address them independently"
        ),
        format!(
            "give each Inputs Table row at {joined} a DISTINCT `name` so they resolve \
             to distinct served keys"
        ),
    )
}

/// (c) The blocking finding for an input whose served `json_key` is empty/whitespace
/// (WBV2-05 §8, F1 reshaped — the repair names the Table row's `name` cell).
fn empty_input_key_finding(cell: &str) -> LintFinding {
    let (sheet, addr) = split_cell_key(cell);
    LintFinding::new(
        Severity::Error,
        "manifest/input-empty-key",
        sheet,
        addr,
        format!(
            "input cell {cell} resolves to an empty served key; a caller would have no \
             field name to set it under"
        ),
        format!(
            "fill the `name` column of the Inputs Table row at {cell} with a non-empty \
             identifier so it gets a usable served key"
        ),
    )
}

/// Run [`validate_input_keys`] over `manifest` and, if it finds any blocking input-key
/// defect, surface them as a single [`CompileError::Lint`] (the SHARED refusal both
/// compile entry points use after `name_named_inputs`).
///
/// # Errors
/// Returns [`CompileError::Lint`] if any `Role::Input` lacks a semantic key, two
/// inputs collide on one served key, or an input's served key is empty.
fn refuse_uncallable_inputs(manifest: &Manifest) -> Result<(), CompileError> {
    let mut report = LintReport::new();
    validate_input_keys(manifest, &mut report);
    if report.has_errors() {
        let errors: Vec<&LintFinding> = report
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .collect();
        return Err(CompileError::Lint(stage1::render_aggregate(&errors)));
    }
    Ok(())
}

/// Validate that EVERY `Role::Output` cell carries a callable served key (M4 — the
/// OUTPUT mirror of [`validate_input_keys`]), pushing one `Severity::Error`
/// [`LintFinding`] per defect into `report`. Without this, two outputs stripping to
/// the same served key would silently last-writer-wins in the `outputSchema` + the
/// runtime payload (inputs were guarded; outputs were not). Three defects, each
/// blocking, mirroring the input gate:
///
/// (a) two outputs whose served `json_key`s COLLIDE (after prefix stripping);
/// (b) an output whose served `json_key` is empty/whitespace;
/// (c) an output whose served `json_key` is VALUE-SHAPED (a numeric literal).
fn validate_output_keys(manifest: &Manifest, report: &mut LintReport) {
    use std::collections::BTreeMap;

    let mut by_key: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for role in manifest.cells.iter().filter(|c| c.role == Role::Output) {
        let key = json_key_for_role(role);
        if key.trim().is_empty() {
            report.push(empty_output_key_finding(&role.cell));
            continue;
        }
        if is_value_shaped_key(&key) {
            report.push(value_shaped_output_key_finding(&role.cell, &key));
            continue;
        }
        by_key.entry(key).or_default().push(role.cell.clone());
    }

    for (key, coords) in by_key.iter().filter(|(_, c)| c.len() > 1) {
        report.push(duplicate_output_key_finding(key, coords));
    }
}

/// (a) The blocking finding for two+ outputs colliding on one served `json_key` (M4).
fn duplicate_output_key_finding(key: &str, coords: &[String]) -> LintFinding {
    let joined = coords.join(", ");
    LintFinding::new(
        Severity::Error,
        "manifest/output-key-collision",
        "manifest",
        None,
        format!(
            "output cells {joined} all map to the same served key `{key}`; the second \
             would silently overwrite the first in the served outputSchema + payload"
        ),
        format!(
            "give each output Table row at {joined} a DISTINCT `name` so they resolve \
             to distinct served output keys"
        ),
    )
}

/// (b) The blocking finding for an output whose served `json_key` is empty (M4).
fn empty_output_key_finding(cell: &str) -> LintFinding {
    let (sheet, addr) = split_cell_key(cell);
    LintFinding::new(
        Severity::Error,
        "manifest/output-empty-key",
        sheet,
        addr,
        format!(
            "output cell {cell} resolves to an empty served key; a caller would have no \
             field name to read it under"
        ),
        format!(
            "fill the `name` column of the output Table row at {cell} with a non-empty \
             identifier so it gets a usable served output key"
        ),
    )
}

/// (c) The blocking finding for a VALUE-SHAPED output served key (M4 — the output
/// mirror of [`value_shaped_input_key_finding`]).
fn value_shaped_output_key_finding(cell: &str, key: &str) -> LintFinding {
    let (sheet, addr) = split_cell_key(cell);
    LintFinding::new(
        Severity::Error,
        "manifest/output-value-shaped-key",
        sheet,
        addr,
        format!(
            "output cell {cell} resolves to the value-shaped served key `{key}`; a numeric \
             name is a VALUE, not a readable identifier — the output field is unreadable"
        ),
        format!(
            "replace the numeric `name` of the output Table row at {cell} with a stable \
             identifier (e.g. `tax_owed`) so the served output carries a usable key"
        ),
    )
}

/// Run [`validate_output_keys`] over `manifest` and surface any blocking output-key
/// defect as a single [`CompileError::Lint`] (the SHARED refusal both compile lanes
/// run next to [`refuse_uncallable_inputs`], M4).
///
/// # Errors
/// Returns [`CompileError::Lint`] if two outputs collide on one served key, an output
/// key is empty, or an output key is value-shaped.
fn refuse_uncallable_outputs(manifest: &Manifest) -> Result<(), CompileError> {
    let mut report = LintReport::new();
    validate_output_keys(manifest, &mut report);
    if report.has_errors() {
        let errors: Vec<&LintFinding> = report
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .collect();
        return Err(CompileError::Lint(stage1::render_aggregate(&errors)));
    }
    Ok(())
}

/// Reject any output-Table tool name that sanitizes to a RESERVED meta-tool name
/// (H3): a Table named so [`sanitize_tool_name`] yields one of
/// [`RESERVED_TOOL_NAMES`](pmcp_workbook_runtime::RESERVED_TOOL_NAMES) (`explain`,
/// `get_manifest`, `diff_version`, `render_workbook`, `verify_accuracy`) would silently
/// last-writer-wins
/// over the meta tool at registration. This runs ALONGSIDE
/// [`refuse_colliding_output_tables`] at stage-1 in BOTH compile lanes, surfacing a
/// cell-precise [`CompileError::Lint`] located at the Table's first output cell BEFORE
/// any bundle write. An unmappable name (no MCP-charset char) is left to the collision
/// gate's `tool-name-unmappable` finding — here we only reject reserved COLLISIONS.
///
/// # Errors
/// Returns [`CompileError::Lint`] if any output-Table name sanitizes into the reserved
/// set.
fn refuse_reserved_output_table_names(output_tables: &[OutputTable]) -> Result<(), CompileError> {
    let mut report = LintReport::new();
    for table in output_tables {
        let Ok(name) = sanitize_tool_name(&table.name) else {
            continue; // unmappable → the collision gate's unmappable finding handles it.
        };
        if RESERVED_TOOL_NAMES.contains(&name.as_str()) {
            report.push(reserved_tool_name_finding(table, &name));
        }
    }
    if report.has_errors() {
        let errors: Vec<&LintFinding> = report
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .collect();
        return Err(CompileError::Lint(stage1::render_aggregate(&errors)));
    }
    Ok(())
}

/// The blocking finding for an output Table whose name sanitizes to a reserved
/// meta-tool name (H3), located at the Table's first output cell.
fn reserved_tool_name_finding(table: &OutputTable, sanitized: &str) -> LintFinding {
    let (sheet, addr) = table
        .output_cells
        .first()
        .map_or(("manifest", None), |c| split_cell_key(c));
    LintFinding::new(
        Severity::Error,
        "manifest/output-table-reserved-name",
        sheet,
        addr,
        format!(
            "output Table '{}' sanitizes to the reserved meta-tool name `{sanitized}`; it \
             would silently overwrite the built-in `{sanitized}` tool at registration",
            table.name
        ),
        format!(
            "rename output Table '{}' so it does NOT sanitize to a reserved meta-tool name \
             (explain / get_manifest / diff_version / render_workbook)",
            table.name
        ),
    )
}

/// The gated-update CANDIDATE: everything [`gate::gate`] and
/// [`gate::accept::promote`] need to grade and (if accepted) publish a re-compile,
/// assembled by COMPOSING the existing private candidate-build internals — WITHOUT
/// writing any bundle (gate-before-write, T-94-00-WRITE).
///
/// The fields line up 1:1 with [`gate::accept::PromoteInputs`] so the thin-shell
/// CLI (94-03) assembles a `PromoteInputs` borrowing a `Candidate` without inventing
/// any field: `bundle_id`/`changelog` are the CLI's lane decision; `ir`/`manifest`/
/// `layout`/`parser_equivalence`/`version`/`candidate_workbook_hash` come straight
/// from here. The `computed` named-output map is what the gate corpus grades.
#[derive(Debug)]
pub struct Candidate {
    /// The compiled IR (`{cell_key -> Cell}`), built via [`build_ir_and_dag`] —
    /// the same shape `compile_workbook` emits, ready for `PromoteInputs::ir`.
    pub ir: HashMap<String, Cell>,
    /// The dependency [`Dag`] reconstructed from the parsed references — the CLI
    /// derives the gate corpus from this candidate IR/DAG.
    pub dag: Dag,
    /// The synthesized manifest with `out_*` outputs promoted (UN-ratified —
    /// `prepare` writes nothing; ratification is the emit step's recorded act).
    pub manifest: Manifest,
    /// The named-output region→value map (`Role::Output` cells projected from the
    /// executor's `RunResult.computed`, finite `f64` only) — the gate's grading set.
    pub computed: BTreeMap<String, f64>,
    /// The candidate workbook content hash (`sha256_hex` of the ORIGINAL bytes) —
    /// the gate's `candidate_workbook_hash` and `PromoteInputs::workbook_hash`.
    pub candidate_workbook_hash: String,
    /// The D-08 parser-equivalence evidence record (from stage 1).
    pub parser_equivalence: ParserEquivalence,
    /// The captured workbook layout descriptor (for `PromoteInputs::layout`).
    pub layout: LayoutDescriptor,
    /// The workbook-DECLARED version (via [`read_workbook_version`]) — so the CLI
    /// never supplies a `--version` flag (D-02/D-11).
    pub version: String,
    /// The harvest-derived per-Table [`OutputTable`] membership (WBV2-04): NON-EMPTY
    /// for a Table-authored workbook (the gated-update lane fans out one served Tool
    /// per output Table), EMPTY for the named-range corpus (single-tool fallback).
    /// Threads straight into [`gate::accept::PromoteInputs::output_tables`].
    pub output_tables: Vec<OutputTable>,
}

/// Build the gated-update [`Candidate`] for `workbook_path` by COMPOSING the
/// existing private candidate-build internals — WITHOUT writing any bundle.
///
/// This MIRRORS [`compile_workbook`]'s pipeline up to BUT NOT INCLUDING the
/// `promote` step: read the ORIGINAL bytes → [`ingest::ingest`] →
/// [`stage1::run_stage1`] (with [`FreshnessPolicy::Enforce`] — `prepare` relaxes NO
/// gate) → [`promote_named_outputs`] → [`build_ir_and_dag`] → [`seed_from_inputs`] +
/// [`sheet_ir::eval`] → project the `Role::Output` computed values → build the
/// layout → read the declared version. It STOPS here: the CLI (94-03) decides
/// block-vs-promote, so `prepare` writes nothing (gate-before-write).
///
/// `prepare` does NOT ratify the manifest: ratification writes a sidecar, and
/// `build_ir_and_dag` reads only the manifest's `Role`s (not the ratification
/// stamp), so skipping ratify keeps `prepare` write-free without changing the IR.
///
/// # Arguments
/// * `workbook_path` — the candidate `.xlsx` to build.
/// * `workflow` — the workflow/bundle name (NEVER a hardcoded literal — WBCO-02).
///
/// # Errors
/// Returns the SAME per-stage [`CompileError`] the seed lane returns: a stage-1
/// `Error` (lint/freshness) surfaces [`CompileError::Lint`]; a parse failure
/// surfaces [`CompileError::Lint`]; a named-output oracle mismatch surfaces
/// [`CompileError::Reconcile`]. `prepare` relaxes none of these gates.
pub fn prepare_candidate(workbook_path: &Path, workflow: &str) -> Result<Candidate, CompileError> {
    prepare_candidate_inner(workbook_path, workflow, FreshnessPolicy::Enforce, None)
}

/// The shared `prepare` body. `freshness` is [`FreshnessPolicy::Enforce`] on the
/// production path ([`prepare_candidate`]); only the TEST-ONLY
/// [`prepare_candidate_with_fixture_override`] passes
/// [`FreshnessPolicy::TrustedFixture`] (the SAME pattern [`compile_workbook_inner`]
/// uses, so the parity test can build a candidate from the committed neutral
/// fixture and compare it against the seed lane's emitted bundle).
///
/// `version_override` is `None` on the production path — the version comes SOLELY
/// from the workbook via [`read_workbook_version`] (D-02/D-11). It is `Some` ONLY
/// from the `#[cfg(test)]` fixture override, because the committed neutral fixture
/// predates the `version` named-range convention and declares no version cell; the
/// production path NEVER supplies an override, so the workbook remains the only
/// version source in production.
fn prepare_candidate_inner(
    workbook_path: &Path,
    workflow: &str,
    freshness: FreshnessPolicy,
    version_override: Option<&str>,
) -> Result<Candidate, CompileError> {
    // (1) ORIGINAL on-disk bytes (the provenance anchor — never a umya round-trip).
    let bytes = std::fs::read(workbook_path)?;

    // (2) umya ingest → owned WorkbookMap + collect-all ingest findings.
    let (map, ingest_findings) =
        ingest::ingest(workbook_path).map_err(|e| CompileError::Ingest(e.to_string()))?;

    // (2a) Resolve + validate the workbook-declared DIALECT version (WBDL-02) —
    // the SAME fail-closed step the SEED lane (`compile_workbook_inner`) runs.
    // The gated-update lane is reached by every governed re-compile through
    // `cargo pmcp workbook compile`, so without this an author bumping
    // `pmcp_dialect_version` to an incompatible value on an already-seeded workbook
    // would be silently accepted (the HI-01 D-04 fail-closed gap). Both lanes call
    // the one `validate_dialect_version_step`, so the gate cannot drift: a different
    // major OR a newer-than-supported minor → typed `CompileError::Lint`; an absent
    // declaration → baseline with NO error (D-05, zero-churn re-compile).
    dialect_version::validate_dialect_version_step(&map)?;

    // (3) Composed stage-1 pass (lint + synth + freshness), collect-all refuse —
    // the SAME gate the seed lane runs; `prepare` does NOT relax it.
    let stage1 = stage1::run_stage1(&bytes, &map, &ingest_findings, workflow, freshness)?;

    // (3a) Promote the workbook's `out_*` named-range targets to `Role::Output`.
    let mut manifest = stage1.synth_manifest;
    promote_named_outputs(&mut manifest, &map);
    // (3b) Name `in_*` named-range INPUT cells (the input analogue of `out_*`).
    name_named_inputs(&mut manifest, &map);
    // (3b-tables) ADDITIVELY promote a Table-authored workbook's harvested Tables —
    // the SAME step the seed lane runs (kept in lock-step so both lanes agree).
    promote_harvested_tables(&mut manifest, &map);

    // (3c) Refuse loudly on an uncallable input (no `in_*` named range), a served
    // input-key collision, an empty/value-shaped served key — the SAME F1/H2 gate the
    // seed lane runs after `name_named_inputs`; `prepare` does NOT relax it.
    refuse_uncallable_inputs(&manifest)?;
    // (3d) M4: the OUTPUT mirror — the SAME gate the seed lane runs (kept in lock-step
    // so the seed/update paths cannot drift on output-key collisions).
    refuse_uncallable_outputs(&manifest)?;

    // (4) The candidate content anchor. NOTE: `prepare` does NOT ratify (ratify
    // writes a sidecar) — gate-before-write means `prepare` writes NOTHING; the
    // manifest's `Role`s alone drive build_ir_and_dag.
    let candidate_workbook_hash = sha256_hex(&bytes);

    // (5) Build the IR + DAG from the parsed formulas (whitelist-at-parse, D-06).
    let (ir, dag) = build_ir_and_dag(&map, &manifest)?;

    // (6) Run the SHARED runtime executor over the IR with inputs seeded from
    // their cached values — the SAME pure-Rust path the served binary uses.
    let seed = seed_from_inputs(&map);
    let run = sheet_ir::eval(&ir, &dag, &seed)
        .map_err(|finding| CompileError::Reconcile(finding.message.clone()))?;

    // (7) Reconcile the computed outputs against the cached oracle (named-output =
    // ERROR). A named-output mismatch blocks — the seed lane's identical gate.
    let comparison = comparison_from_outputs(&map, &manifest);
    let report = reconcile::reconcile(&run.computed, &run.traces, &ir, &comparison, &manifest);
    if report.has_errors() || report.is_hard_fail() {
        return Err(CompileError::Reconcile(format!(
            "{} named-output mismatch(es) against the cached oracle",
            report
                .mismatches
                .iter()
                .filter(|m| m.severity == Severity::Error)
                .count()
        )));
    }

    // (7a) Derive + gate the per-Table membership — the SAME WBV2-04 wiring the seed
    // lane runs (collision Errors block; per-tool reconcile blocks). An empty set
    // (named-range corpus) is a no-op on both gates.
    let output_tables = output_tables_from_harvest(&map, &manifest);
    refuse_colliding_output_tables(&output_tables)?;
    // H3: the SAME reserved-name gate the seed lane runs (dual-lane, no drift).
    refuse_reserved_output_table_names(&output_tables)?;
    // M6: the SAME cached-`<v>` per-tool oracle the seed lane wires (dual-lane).
    let output_oracles = output_oracle_map(&map, &manifest);
    reconcile_output_tables(&output_tables, &dag, &manifest, &run, &output_oracles)?;

    // (8) Project the named-output computed values into the gate's grading map and
    // capture the layout + declared version. The candidate STOPS here (no promote).
    let computed = project_named_outputs(&manifest, &run.computed);
    let layout = artifact::build_layout_descriptor(&map, &candidate_workbook_hash);
    let version = match version_override {
        Some(v) => v.to_string(),
        None => read_workbook_version(workbook_path)?,
    };

    Ok(Candidate {
        ir,
        dag,
        manifest,
        computed,
        candidate_workbook_hash,
        parser_equivalence: stage1.parser_equivalence,
        layout,
        version,
        output_tables,
    })
}

/// Project the manifest's `Role::Output` cells from the executor's `computed`
/// `{cell_key -> CellValue}` into the gate's `{cell_key -> f64}` grading map,
/// keeping ONLY finite numeric outputs (a non-numeric or non-finite output is not
/// a gradable named output and is skipped — the reconcile gate already refused a
/// genuinely wrong output above).
fn project_named_outputs(
    manifest: &Manifest,
    computed: &HashMap<String, CellValue>,
) -> BTreeMap<String, f64> {
    let mut out: BTreeMap<String, f64> = BTreeMap::new();
    for role in &manifest.cells {
        if !matches!(role.role, Role::Output) {
            continue;
        }
        if let Some(CellValue::Number(n)) = computed.get(&role.cell) {
            if n.is_finite() {
                out.insert(role.cell.clone(), *n);
            }
        }
    }
    out
}

/// TEST-ONLY: build a [`Candidate`] from a committed neutral fixture, honouring its
/// trusted-fixture provenance override (the SAME `#[cfg(test)]`-only mechanism
/// [`compile_workbook_with_fixture_override`] uses — CR-01: NO publishable feature
/// arms it). The production [`prepare_candidate`] always passes
/// [`FreshnessPolicy::Enforce`], so the same bytes are REFUSED on the production
/// path. This lets the parity test build a candidate from the committed
/// `tax-calc.xlsx` and compare its IR/computed against the seed lane.
#[cfg(test)]
fn prepare_candidate_with_fixture_override(
    workbook_path: &Path,
    workflow: &str,
) -> Result<Candidate, CompileError> {
    // The committed neutral fixture predates the `version` named-range convention,
    // so the test supplies the version the seed-lane proof uses ("1.1.0"). The
    // production `prepare_candidate` NEVER reaches this path — it always reads the
    // version from the workbook.
    prepare_candidate_inner(
        workbook_path,
        workflow,
        FreshnessPolicy::TrustedFixture,
        Some("1.1.0"),
    )
}

// ---- F1: input-key validation (the uncallable-input compile gate) ----------
#[cfg(test)]
mod input_key_validation_tests {
    use super::*;
    use pmcp_workbook_runtime::Dtype;

    fn role(cell: &str, r: Role, name: Option<&str>, meaning: Option<&str>) -> CellRole {
        CellRole {
            cell: cell.to_string(),
            role: r,
            name: name.map(str::to_string),
            unit: None,
            meaning: meaning.map(str::to_string),
            dtype: Dtype::Number,
            colour_evidence: None,
            source: "test".to_string(),
            notes: None,
            tier: None,
            allowed_values: None,
        }
    }

    fn manifest(cells: Vec<CellRole>) -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "tax-calc".to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: None,
            ratified_at: None,
            cells,
            loop_block: None,
            governed_data: vec![],
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        }
    }

    #[test]
    fn named_input_passes() {
        let m = manifest(vec![
            role("1_Inputs!B2", Role::Input, Some("in_gross_income"), None),
            role("3_Outputs!B2", Role::Output, Some("out_tax"), None),
        ]);
        assert!(
            refuse_uncallable_inputs(&m).is_ok(),
            "a well-named in_* input compiles"
        );
    }

    #[test]
    fn unnamed_input_fails_with_error() {
        // name: None, meaning = the cell value (the degenerate value-keyed case).
        let m = manifest(vec![role("1_Inputs!B2", Role::Input, None, Some("240000"))]);
        let err = refuse_uncallable_inputs(&m).expect_err("unnamed input must block compile");
        match err {
            CompileError::Lint(msg) => {
                assert!(
                    msg.contains("input-no-semantic-key") && msg.contains("1_Inputs!B2"),
                    "the refusal names the rule + the offending cell: {msg}"
                );
            },
            other => panic!("expected CompileError::Lint, got {other:?}"),
        }
    }

    #[test]
    fn formatting_only_blank_input_is_removed() {
        let mut m = manifest(vec![
            role("Data!B19", Role::Input, None, None),
            role("Data!B18", Role::Input, Some("operations"), Some("300")),
        ]);
        remove_unnamed_blank_inputs(&mut m);
        assert_eq!(m.cells.len(), 1);
        assert_eq!(m.cells[0].cell, "Data!B18");
    }

    #[test]
    fn deliberately_named_blank_input_is_preserved() {
        let mut m = manifest(vec![role(
            "Data!B19",
            Role::Input,
            Some("optional_note"),
            None,
        )]);
        remove_unnamed_blank_inputs(&mut m);
        assert_eq!(m.cells.len(), 1);
        assert!(refuse_uncallable_inputs(&m).is_ok());
    }

    #[test]
    fn unnamed_populated_input_still_fails_closed() {
        let mut m = manifest(vec![role(
            "Data!B19",
            Role::Input,
            None,
            Some("unexpected value"),
        )]);
        remove_unnamed_blank_inputs(&mut m);
        assert_eq!(m.cells.len(), 1);
        assert!(refuse_uncallable_inputs(&m).is_err());
    }

    #[test]
    fn duplicate_served_keys_fail() {
        // Two inputs whose served json_key collides AFTER F3 prefix stripping
        // (`in_x` and `x` both strip/resolve to `x`).
        let m = manifest(vec![
            role("1_Inputs!B2", Role::Input, Some("in_x"), None),
            role("1_Inputs!B3", Role::Input, Some("x"), None),
        ]);
        let err = refuse_uncallable_inputs(&m).expect_err("colliding served keys must block");
        match err {
            CompileError::Lint(msg) => assert!(
                msg.contains("input-key-collision")
                    && msg.contains("1_Inputs!B2")
                    && msg.contains("1_Inputs!B3"),
                "the collision refusal names both coords: {msg}"
            ),
            other => panic!("expected CompileError::Lint, got {other:?}"),
        }
    }

    #[test]
    fn empty_served_key_fails() {
        // A name that is exactly the prefix strips to itself ("in_"), but a
        // whitespace name resolves to an empty served key.
        let m = manifest(vec![role("1_Inputs!B2", Role::Input, Some("   "), None)]);
        let err = refuse_uncallable_inputs(&m).expect_err("empty served key must block");
        match err {
            CompileError::Lint(msg) => assert!(
                msg.contains("input-empty-key") && msg.contains("1_Inputs!B2"),
                "the empty-key refusal names the cell: {msg}"
            ),
            other => panic!("expected CompileError::Lint, got {other:?}"),
        }
    }

    #[test]
    fn all_unnamed_inputs_each_get_their_own_error() {
        let m = manifest(vec![
            role("1_Inputs!B2", Role::Input, None, Some("a")),
            role("1_Inputs!B3", Role::Input, None, Some("b")),
            role("3_Outputs!B2", Role::Output, Some("out_o"), None),
        ]);
        let mut report = LintReport::new();
        validate_input_keys(&m, &mut report);
        let errors = report
            .findings
            .iter()
            .filter(|f| f.rule == "manifest/input-no-semantic-key")
            .count();
        assert_eq!(errors, 2, "each unnamed input is flagged independently");
    }

    #[test]
    fn outputs_and_constants_are_not_input_checked() {
        // An unnamed Output/Constant is NOT an input-key defect (the check is
        // scoped to Role::Input).
        let m = manifest(vec![
            role("1_Inputs!B2", Role::Input, Some("in_amount"), None),
            role("3_Outputs!B2", Role::Output, None, Some("Total")),
            role("2_Const!B2", Role::Constant, None, None),
        ]);
        assert!(
            refuse_uncallable_inputs(&m).is_ok(),
            "only Role::Input cells are input-key validated"
        );
    }

    // ---- H2: value-shaped input name ----------------------------------------

    #[test]
    fn value_shaped_input_name_fails_with_cell_located_error() {
        // A BA typed `60000` in the `name` column — the served key degenerates to a
        // VALUE. The compile must fail with a cell-located reserved-rule Error.
        let m = manifest(vec![role("1_Inputs!B2", Role::Input, Some("60000"), None)]);
        let err = refuse_uncallable_inputs(&m).expect_err("value-shaped name must block");
        match err {
            CompileError::Lint(msg) => assert!(
                msg.contains("input-value-shaped-key") && msg.contains("1_Inputs!B2"),
                "the refusal names the rule + the offending cell: {msg}"
            ),
            other => panic!("expected CompileError::Lint, got {other:?}"),
        }
    }

    #[test]
    fn decimal_and_signed_value_shaped_names_fail() {
        for bad in ["1.5", "-3", "+7", "0.0"] {
            let m = manifest(vec![role("1_Inputs!B2", Role::Input, Some(bad), None)]);
            assert!(
                refuse_uncallable_inputs(&m).is_err(),
                "value-shaped name `{bad}` must block compile"
            );
        }
    }

    #[test]
    fn valid_identifier_name_compiles() {
        for ok in ["income", "q1_2024", "tax_owed", "x1"] {
            let m = manifest(vec![role("1_Inputs!B2", Role::Input, Some(ok), None)]);
            assert!(
                refuse_uncallable_inputs(&m).is_ok(),
                "identifier `{ok}` is NOT value-shaped and compiles"
            );
        }
    }

    // ---- M4: output-key collision / empty / value-shaped --------------------

    #[test]
    fn duplicate_output_served_keys_fail() {
        // Two outputs stripping to the same served key (`out_tax` and `tax`).
        let m = manifest(vec![
            role("3_Out!B2", Role::Output, Some("out_tax"), None),
            role("3_Out!B3", Role::Output, Some("tax"), None),
        ]);
        let err = refuse_uncallable_outputs(&m).expect_err("colliding output keys must block");
        match err {
            CompileError::Lint(msg) => assert!(
                msg.contains("output-key-collision")
                    && msg.contains("3_Out!B2")
                    && msg.contains("3_Out!B3"),
                "the output collision names both coords: {msg}"
            ),
            other => panic!("expected CompileError::Lint, got {other:?}"),
        }
    }

    #[test]
    fn value_shaped_output_key_fails() {
        let m = manifest(vec![role("3_Out!B2", Role::Output, Some("18241"), None)]);
        let err = refuse_uncallable_outputs(&m).expect_err("value-shaped output key must block");
        match err {
            CompileError::Lint(msg) => assert!(
                msg.contains("output-value-shaped-key") && msg.contains("3_Out!B2"),
                "the refusal names the rule + cell: {msg}"
            ),
            other => panic!("expected CompileError::Lint, got {other:?}"),
        }
    }

    #[test]
    fn distinct_output_keys_compile() {
        let m = manifest(vec![
            role("3_Out!B2", Role::Output, Some("out_tax"), None),
            role("3_Out!B3", Role::Output, Some("out_refund"), None),
        ]);
        assert!(
            refuse_uncallable_outputs(&m).is_ok(),
            "distinct stripped output keys compile"
        );
    }

    // ---- the numeric-key reject predicate (property arm, H2/M4) --------------

    #[test]
    fn is_value_shaped_key_predicate_is_numeric_only() {
        for numeric in ["0", "60000", "1.5", "-3", "+7", "3e4", "0.182"] {
            assert!(
                is_value_shaped_key(numeric),
                "`{numeric}` is value-shaped (a finite numeric literal)"
            );
        }
        for ident in [
            "income", "q1_2024", "x1", "tax_owed", "", "  ", "NaN", "inf",
        ] {
            assert!(
                !is_value_shaped_key(ident),
                "`{ident}` is NOT value-shaped (an identifier / empty / non-finite)"
            );
        }
    }

    use proptest::prelude::*;

    proptest! {
        /// PROPERTY (H2/M4): ANY finite numeric literal, used as an input OR output
        /// `name`, is rejected by the served-key gate — the reject predicate round-trips
        /// over arbitrary finite f64s on BOTH the input and output lanes.
        #[test]
        fn finite_numeric_name_rejected_on_both_lanes(n in proptest::num::f64::NORMAL) {
            let key = n.to_string();
            prop_assume!(key.parse::<f64>().map(f64::is_finite).unwrap_or(false));
            prop_assert!(is_value_shaped_key(&key), "predicate flags `{}`", key);

            // input lane: a Role::Input whose name is the numeric string is refused.
            let mi = manifest(vec![role("1_Inputs!B2", Role::Input, Some(&key), None)]);
            prop_assert!(
                refuse_uncallable_inputs(&mi).is_err(),
                "validate_input_keys rejects value-shaped name `{}`",
                key
            );

            // output lane: a Role::Output whose name is the numeric string is refused.
            let mo = manifest(vec![role("3_Out!B2", Role::Output, Some(&key), None)]);
            prop_assert!(
                refuse_uncallable_outputs(&mo).is_err(),
                "validate_output_keys rejects value-shaped name `{}`",
                key
            );
        }
    }
}

// ---- WBV2-04: harvest-derived OutputTable membership + collision gate ----------
#[cfg(test)]
mod harvest_output_table_tests {
    use super::*;
    use crate::ingest::{CellRecord, FormulaKind, RangeRef, SheetRecord, TableRecord, WorkbookMap};
    use pmcp_workbook_runtime::Dtype;

    fn cell(addr: &str, value: Option<&str>) -> CellRecord {
        CellRecord {
            addr: addr.to_string(),
            formula: None,
            value: value.map(str::to_string),
            fill_argb: None,
            font_argb: None,
            number_format: None,
            is_formula: false,
            formula_kind: FormulaKind::Normal,
        }
    }

    fn role(cell: &str, r: Role, name: Option<&str>) -> CellRole {
        CellRole {
            cell: cell.to_string(),
            role: r,
            name: name.map(str::to_string),
            unit: None,
            meaning: None,
            dtype: Dtype::Number,
            colour_evidence: None,
            source: "test".to_string(),
            notes: None,
            tier: None,
            allowed_values: None,
        }
    }

    fn table(name: &str, sheet: &str, start: &str, end: &str) -> TableRecord {
        TableRecord {
            name: name.to_string(),
            area: RangeRef {
                sheet: sheet.to_string(),
                start: start.to_string(),
                end: end.to_string(),
            },
            columns: vec!["name".to_string(), "value".to_string()],
        }
    }

    fn sheet(name: &str, tables: Vec<TableRecord>, cells: Vec<CellRecord>) -> SheetRecord {
        SheetRecord {
            name: name.to_string(),
            state: "visible".to_string(),
            hidden_rows: vec![],
            hidden_cols: vec![],
            col_widths: vec![],
            merges: vec![],
            cf_ranges: vec![],
            tables: vec![],
            table_records: tables,
            data_validations: vec![],
            notes: vec![],
            cells,
        }
    }

    fn workbook(sheets: Vec<SheetRecord>) -> WorkbookMap {
        WorkbookMap {
            sheets,
            defined_names: vec![],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        }
    }

    fn manifest_with(cells: Vec<CellRole>) -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "tax-suite".to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: None,
            ratified_at: None,
            cells,
            loop_block: None,
            governed_data: vec![],
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        }
    }

    #[test]
    fn output_tables_from_harvest_groups_output_cells() {
        // A Calculate_Tax Table (header A9, body A10:B11) covering two Role::Output
        // value cells (B10, B11) and an Inputs Table (header A2, body A3:B4) covering
        // only Role::Input cells. The helper returns exactly ONE OutputTable named
        // Calculate_Tax with both output cell keys, and SKIPS the all-input Inputs.
        let data = sheet(
            "Data",
            vec![
                table("Inputs", "Data", "A2", "B4"),
                table("Calculate_Tax", "Data", "A9", "B11"),
            ],
            vec![
                cell("A8", Some("Compute the tax")), // caption one row above A9
                cell("A10", Some("tax_owed")),
                cell("A11", Some("effective_rate")),
            ],
        );
        let map = workbook(vec![data]);
        let manifest = manifest_with(vec![
            role("Data!B3", Role::Input, Some("income")),
            role("Data!B4", Role::Input, Some("withheld")),
            role("Data!B10", Role::Output, Some("tax_owed")),
            role("Data!B11", Role::Output, Some("effective_rate")),
        ]);

        let tables = output_tables_from_harvest(&map, &manifest);
        assert_eq!(tables.len(), 1, "the all-input Inputs Table is skipped");
        assert_eq!(tables[0].name, "Calculate_Tax");
        assert_eq!(
            tables[0].description.as_deref(),
            Some("Compute the tax"),
            "the caption one row above the area is the tool description"
        );
        let mut cells = tables[0].output_cells.clone();
        cells.sort();
        assert_eq!(
            cells,
            vec!["Data!B10".to_string(), "Data!B11".to_string()],
            "both Role::Output value cells inside the Table area are grouped"
        );
    }

    #[test]
    fn colliding_output_tables_block_compile_path() {
        // Two output Tables whose names sanitize to the SAME MCP name produce a
        // Severity::Error finding that the stage-1 aggregate render includes — the
        // exact gate compile_workbook_inner folds in (T-100-17). `Calculate Tax` and
        // `calculate_tax` both sanitize to `calculate_tax`.
        let tables = vec![
            OutputTable {
                name: "Calculate Tax".to_string(),
                description: None,
                output_cells: vec!["Data!B10".to_string()],
            },
            OutputTable {
                name: "calculate_tax".to_string(),
                description: None,
                output_cells: vec!["Data!B11".to_string()],
            },
        ];
        let err = refuse_colliding_output_tables(&tables)
            .expect_err("colliding sanitized names block the compile");
        match err {
            CompileError::Lint(rendered) => {
                assert!(
                    rendered.contains("manifest/tool-name-collision"),
                    "the collision rule is rendered into the stage-1 gate: {rendered}"
                );
            },
            other => panic!("expected CompileError::Lint, got {other:?}"),
        }
    }

    #[test]
    fn distinct_output_tables_do_not_block() {
        let tables = vec![
            OutputTable {
                name: "Calculate_Tax".to_string(),
                description: None,
                output_cells: vec!["Data!B10".to_string()],
            },
            OutputTable {
                name: "Estimate_Refund".to_string(),
                description: None,
                output_cells: vec!["Data!B17".to_string()],
            },
        ];
        assert!(
            refuse_colliding_output_tables(&tables).is_ok(),
            "two distinct sanitized names do not collide"
        );
    }

    // ---- H3: reserved meta-tool name collision ------------------------------

    #[test]
    fn output_table_sanitizing_to_reserved_name_fails() {
        // `Explain`, `explain `, `EXPLAIN` all sanitize to `explain` — a reserved
        // meta-tool name. (A space collapses to `_`, so `ex plain` → `ex_plain` is
        // NOT reserved — the sanitizer's space→`_` rule, asserted elsewhere.)
        for raw in ["Explain", "explain ", " explain", "EXPLAIN"] {
            let tables = vec![OutputTable {
                name: raw.to_string(),
                description: None,
                output_cells: vec!["Data!B10".to_string()],
            }];
            let err = match refuse_reserved_output_table_names(&tables) {
                Err(CompileError::Lint(msg)) => msg,
                other => panic!("reserved name `{raw}` should error with Lint, got {other:?}"),
            };
            assert!(
                err.contains("output-table-reserved-name")
                    && err.contains("Data!B10")
                    && err.contains("explain"),
                "the reserved-name refusal names the rule, the cell, and the reserved \
                 name for `{raw}`: {err}"
            );
        }
    }

    #[test]
    fn reserved_name_gate_covers_all_four_meta_tools() {
        // Each raw name sanitizes (lowercase + space→`_`, literal `_` kept) to one of
        // the four reserved meta-tool names.
        for raw in [
            "get manifest",
            "Get_Manifest",
            "Diff_Version",
            "Render_Workbook",
        ] {
            let tables = vec![OutputTable {
                name: raw.to_string(),
                description: None,
                output_cells: vec!["Data!B10".to_string()],
            }];
            assert!(
                refuse_reserved_output_table_names(&tables).is_err(),
                "`{raw}` sanitizes into the reserved set and must block"
            );
        }
    }

    #[test]
    fn non_reserved_output_table_name_passes_reserved_gate() {
        let tables = vec![OutputTable {
            name: "Calculate_Tax".to_string(),
            description: None,
            output_cells: vec!["Data!B10".to_string()],
        }];
        assert!(
            refuse_reserved_output_table_names(&tables).is_ok(),
            "a normal output-Table name is not reserved"
        );
    }

    #[test]
    fn reserved_set_is_derived_from_the_shared_const() {
        // The gate reads the SHARED RESERVED_TOOL_NAMES const (sourced from the
        // handler NAME constants), NOT a hand-copied literal list. The 5th entry
        // `verify_accuracy` is reserved as of Phase 100 Plan 01 (its handler lands
        // in Plan 04 — see the RESERVED_TOOL_NAMES doc).
        assert_eq!(
            pmcp_workbook_runtime::RESERVED_TOOL_NAMES,
            [
                "explain",
                "get_manifest",
                "diff_version",
                "render_workbook",
                "verify_accuracy"
            ],
        );
    }
}
