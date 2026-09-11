//! The freshness / provenance gate — the stale-cache gate + the
//! accept-as-oracle decision + the provenance record + the WBCO-07
//! umya-fabrication refusal.
//!
//! [`gate`] reads `calcPr` + app-identity from the ORIGINAL on-disk `.xlsx`
//! bytes (via the quarantined [`super::raw_parts`] reader — NEVER a
//! umya-round-tripped copy, which FABRICATES `calcId=122211` + "Microsoft
//! Excel"), classifies the provenance into a [`ProvenanceClass`], and either
//! PRODUCES the [`OracleCorpus`] (on a clean accept) or REFUSES with collect-all
//! `Severity::Error` `oracle/*` findings. It populates [`OracleProvenance`] incl.
//! the four region hashes.
//!
//! # The ProvenanceClass model (net-new, WBCO-07)
//!
//! The refuse decision and its reason-code derive from [`ProvenanceClass`], NOT
//! an ad-hoc check. Classification reads ORIGINAL bytes (`docProps/app.xml`
//! `<Application>`/`<AppVersion>` + `calcPr@calcId`):
//!
//! - [`ProvenanceClass::ExcelTrusted`] — `<Application>` exactly matches a known
//!   Microsoft Excel identity (Windows or Mac) AND a POSITIVE Excel marker is
//!   present (an `<AppVersion>` build string AND a calcId that is NOT the umya
//!   sentinel) → ACCEPT.
//! - [`ProvenanceClass::UmyaFabricated`] — `<Application>` says "Microsoft Excel"
//!   but the umya fabrication signal is present (calcId == [`UMYA_SENTINEL_CALC_ID`]
//!   AND/OR the `<AppVersion>` build string is ABSENT) → REFUSE
//!   (`oracle/non-excel-app`). This is the WBCO-07 upgrade: umya stamps exactly
//!   `<Application>Microsoft Excel</Application>` + `calcId=122211`, so the
//!   anchored `.starts_with` alone PASSES umya — we detect the fabrication.
//! - [`ProvenanceClass::NonExcel`] — `<Application>` is not "Microsoft Excel" →
//!   REFUSE (`oracle/non-excel-app`).
//! - [`ProvenanceClass::UnknownStale`] — app.xml unreadable/absent but the raw
//!   read otherwise succeeded → REFUSE fail-closed (never defaults to trusted).
//!
//! # False-positive policy
//!
//! The ONLY path to [`ProvenanceClass::ExcelTrusted`] requires BOTH a recognized
//! exact application identity AND a positive Excel marker (an `<AppVersion>`
//! build string). A real Excel file always carries an `<AppVersion>` build
//! string, so genuine Excel saves are not refused. If a real Excel file is ever
//! observed with a sentinel
//! calcId, the positive-AppVersion marker still admits it — the sentinel calcId
//! ALONE never refuses; it only contributes to UmyaFabricated when paired with
//! an absent AppVersion, OR is overridden by a present AppVersion. (umya writes
//! NEITHER a real AppVersion build NOR a non-sentinel calcId, so its fabricated
//! identity is refused while real Excel passes.)
//!
//! # No recompute, no semantic reconciliation
//!
//! The gate is OBJECTIVE-METADATA-ONLY. It detects METADATA staleness and
//! fabricated provenance; it does NOT recompute/evaluate a formula.

use serde::Serialize;

use crate::ingest::{cell_key, WorkbookMap};
use crate::{LintFinding, Manifest, Severity};

use super::raw_parts::{read_app_props, read_calc_pr};
use super::region_hash::compute_region_hashes;
use super::{OracleCorpus, OracleProvenance, ProvenanceError, RegionHashes};

/// The umya writer's fixed `calcId` fingerprint. umya hard-codes
/// `<calcPr calcId="122211"/>` on every write, so a workbook carrying this
/// calcId AND no Excel `<AppVersion>` build string is umya-fabricated (WBCO-07).
pub(crate) const UMYA_SENTINEL_CALC_ID: u32 = 122211;

/// Exact OOXML `<Application>` identities emitted by supported Microsoft Excel
/// desktop applications. Exact matching is deliberate: prefix matching would
/// admit spoofed values such as `Microsoft Excelerator`.
const MICROSOFT_EXCEL_APPLICATIONS: &[&str] = &["Microsoft Excel", "Microsoft Macintosh Excel"];

/// The genuine-STALENESS freshness rules a `#[cfg(test)]` trusted-fixture override
/// may demote from `Error` to `Warning` (so a committed fixture written by a
/// non-Excel WRITER — whose `fullCalcOnLoad=1` recalc stamp trips the staleness
/// signals even though its cached `<v>` values ARE the authored oracle — can
/// compile on the proof path; the softened signals survive as recorded evidence).
///
/// CR-01 defense-in-depth: `oracle/non-excel-app` is deliberately NOT here. The
/// fabricated-/non-Excel-identity refusal can NEVER be softened — not even by the
/// test-only override. A fixture that needs trusted Excel identity must obtain it
/// the legitimate way (carry a genuine Excel `<Application>` + `<AppVersion>` so
/// it classifies as [`ProvenanceClass::ExcelTrusted`]); the override may only
/// relax genuine staleness. The STRUCTURAL corruption rules
/// (`oracle/unreadable-provenance`, `oracle/missing-provenance`,
/// `oracle/missing-manifest`) are likewise NOT here — they stay `Error` even under
/// the override, so a malformed/unreadable workbook still hard-refuses.
const SOFTENABLE_FRESHNESS_RULES: &[&str] = &[
    "oracle/no-recalc",
    "oracle/stale-cache",
    "oracle/missing-cache",
];

/// The provenance verdict — the single authoritative classification the refuse
/// decision derives from (never an ad-hoc check). `ExcelTrusted` is the ONLY
/// accept class; every other variant REFUSES.
///
/// `Serialize`/`JsonSchema` so it rides on [`OracleProvenance`] into the evidence
/// bundle; `Eq` so tests assert the exact verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProvenanceClass {
    /// A recognized exact Microsoft Excel application identity AND a positive
    /// Excel marker (`<AppVersion>` build string AND a non-sentinel calcId) —
    /// the ONLY accept class.
    ExcelTrusted,
    /// `<Application>` is not "Microsoft Excel" → REFUSE.
    NonExcel,
    /// umya-fabricated identity: anchored "Microsoft Excel" name but the umya
    /// fingerprint (sentinel calcId AND/OR absent `<AppVersion>`) → REFUSE
    /// (WBCO-07).
    UmyaFabricated,
    /// app.xml unreadable/absent but the raw read otherwise succeeded → REFUSE
    /// fail-closed (NEVER defaults to trusted).
    UnknownStale,
}

/// Classify provenance from the ORIGINAL-bytes raw read.
///
/// `application` is `docProps/app.xml <Application>`; `app_version` is
/// `<AppVersion>`; `calc_id` is `calcPr@calcId`. The result drives the refuse
/// decision in [`gate`] (and is the SINGLE place the WBCO-07 fabrication signal
/// is decided). Pure over its inputs — no I/O, deterministic.
pub(crate) fn classify(
    application: Option<&str>,
    app_version: Option<&str>,
    calc_id: Option<u32>,
) -> ProvenanceClass {
    let Some(app) = application else {
        // app.xml absent or `<Application>` empty → fail closed.
        return ProvenanceClass::UnknownStale;
    };
    // Exact known identity, after insignificant surrounding whitespace. Do not
    // use contains/starts_with: either would admit a spoofed application name.
    let recognized_excel = MICROSOFT_EXCEL_APPLICATIONS.contains(&app.trim());
    if !recognized_excel {
        return ProvenanceClass::NonExcel;
    }

    // Recognized Excel NAME present — now require a POSITIVE Excel marker to admit.
    // A genuine Excel save always carries an `<AppVersion>` build string; umya
    // writes NONE (and stamps the sentinel calcId).
    let has_app_version = app_version.is_some_and(|v| !v.trim().is_empty());
    let is_sentinel_calc_id = calc_id == Some(UMYA_SENTINEL_CALC_ID);

    // ExcelTrusted requires BOTH the positive AppVersion marker AND a
    // non-sentinel calcId. The positive AppVersion marker is decisive: a present
    // AppVersion admits even a sentinel calcId (false-positive policy — the
    // sentinel ALONE never refuses). umya satisfies NEITHER, so it falls to
    // UmyaFabricated below.
    if has_app_version && !is_sentinel_calc_id {
        ProvenanceClass::ExcelTrusted
    } else if has_app_version && is_sentinel_calc_id {
        // Real Excel build string present but a sentinel calcId: the positive
        // marker admits it (false-positive policy — never refuse on the sentinel
        // alone).
        ProvenanceClass::ExcelTrusted
    } else {
        // Recognized Excel name but NO positive Excel marker (absent AppVersion),
        // with/without the sentinel calcId → umya-fabricated.
        ProvenanceClass::UmyaFabricated
    }
}

/// The workbook-level finding location: the first sheet name, or `"workbook"`.
pub(crate) fn workbook_sheet(map: &WorkbookMap) -> String {
    map.sheets
        .first()
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "workbook".to_string())
}

/// An all-zero/sentinel [`RegionHashes`] used ONLY when the role-less
/// `oracle/missing-manifest` finding fired (region hashing was impossible).
fn sentinel_region_hashes() -> RegionHashes {
    let zero = "0".repeat(64);
    RegionHashes {
        inputs: Some(zero.clone()),
        formulas: Some(zero.clone()),
        data: Some(zero.clone()),
        outputs: Some(zero),
    }
}

/// Convert a [`ProvenanceError`] from the quarantined raw reader into a
/// fail-closed `oracle/*` `Severity::Error` finding (a raw-reader failure has
/// SOMEWHERE to go; no panic, no silent default).
fn provenance_error_to_finding(err: &ProvenanceError, sheet: &str) -> LintFinding {
    match err {
        ProvenanceError::UnreadableZip { .. }
        | ProvenanceError::UnreadableXml { .. }
        | ProvenanceError::PartTooLarge { .. }
        | ProvenanceError::DecompressBomb { .. }
        | ProvenanceError::XmlTooDeep { .. } => LintFinding::new(
            Severity::Error,
            "oracle/unreadable-provenance",
            sheet.to_string(),
            None,
            format!("the workbook's provenance could not be read: {err}"),
            "the .xlsx is malformed/over-size/corrupt — re-export a clean \
             workbook from Microsoft Excel.",
        ),
        ProvenanceError::MissingPart { part } => LintFinding::new(
            Severity::Error,
            "oracle/missing-provenance",
            sheet.to_string(),
            None,
            format!("a required OOXML provenance part is missing: {part}"),
            "the .xlsx is missing a required OOXML part — re-save the workbook \
             from Microsoft Excel.",
        ),
    }
}

/// A committed-fixture provenance override (TEST paths ONLY).
///
/// A trusted committed fixture (authored by `rust_xlsxwriter`, whose non-Excel
/// recalc stamp trips the STALENESS freshness signals) may ship this marker so
/// reconcile tests can run. The override is honoured ONLY by
/// [`gate_with_fixture_override`] (a `#[cfg(test)]` API — CR-01: NOT reachable
/// from any publishable feature); the production [`gate`] NEVER honours it —
/// production always classifies from raw bytes. A test asserts the SAME bytes are
/// still REFUSED on the production path (`override_does_not_weaken_production`).
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub struct TrustedFixtureMarker;

/// The freshness / provenance gate (production path).
///
/// **The caller MUST supply `std::fs::read(path)?` of the ORIGINAL `.xlsx` file**
/// as `original_bytes` (the gate reads `calcPr`/app-identity from the on-disk
/// bytes, NEVER a umya-round-tripped copy). `map` is the owned [`WorkbookMap`]
/// from [`crate::ingest`]; `manifest` supplies the [`Manifest`] roles that
/// partition the region hashes.
///
/// Returns the populated [`OracleProvenance`] (for EVERY run where the raw read
/// SUCCEEDS — accept OR refuse) plus either `Ok(OracleCorpus)` (on a clean
/// accept) or `Err(Vec<LintFinding>)` (collect-all `oracle/*`, fail-closed). The
/// production path NEVER honours a [`TrustedFixtureMarker`].
pub fn gate(
    original_bytes: &[u8],
    map: &WorkbookMap,
    manifest: &Manifest,
) -> (OracleProvenance, Result<OracleCorpus, Vec<LintFinding>>) {
    gate_inner(original_bytes, map, manifest, false)
}

/// The freshness / provenance gate with an OPTIONAL trusted-fixture override
/// (TEST API only).
///
/// When `fixture_override` is `Some(TrustedFixtureMarker)`, the genuine-STALENESS
/// freshness findings ([`SOFTENABLE_FRESHNESS_RULES`]) are DEMOTED to `Warning` so
/// reconcile tests can run against committed fixtures whose non-Excel WRITER trips
/// the staleness signals. The override CANNOT soften the fabricated-/non-Excel
/// IDENTITY refusal (`oracle/non-excel-app`) — that always stays a blocking
/// `Error` (CR-01 defense-in-depth). The override CANNOT weaken the production
/// refuse path: [`gate`] passes `false` and always classifies from raw bytes.
///
/// This entry exists ONLY for the test path (gated behind `#[cfg(test)]` — CR-01:
/// there is NO publishable feature that arms it, so a default/`--all-features`
/// build of the crate as a dependency neither compiles nor exposes it); it is
/// never wired into the production `compile_workbook` driver (which always passes
/// `false`).
#[cfg(test)]
pub(crate) fn gate_with_fixture_override(
    original_bytes: &[u8],
    map: &WorkbookMap,
    manifest: &Manifest,
    fixture_override: Option<TrustedFixtureMarker>,
) -> (OracleProvenance, Result<OracleCorpus, Vec<LintFinding>>) {
    gate_inner(original_bytes, map, manifest, fixture_override.is_some())
}

/// The shared gate body. `soften_staleness` is `false` on the production path
/// ([`gate`]); only the test API ([`gate_with_fixture_override`]) may pass `true`.
/// When `true`, the genuine-staleness findings ([`SOFTENABLE_FRESHNESS_RULES`])
/// are demoted to `Warning` — but the fabricated-/non-Excel IDENTITY refusal
/// (`oracle/non-excel-app`) ALWAYS fires (CR-01: it is not softenable), and the
/// override never bypasses any other finding.
#[allow(clippy::too_many_lines)]
fn gate_inner(
    original_bytes: &[u8],
    map: &WorkbookMap,
    manifest: &Manifest,
    soften_staleness: bool,
) -> (OracleProvenance, Result<OracleCorpus, Vec<LintFinding>>) {
    let sheet = workbook_sheet(map);
    let mut findings: Vec<LintFinding> = Vec::new();

    // RAW-READER FAILURE PATH (fail-closed): read calcPr + app-identity from the
    // ORIGINAL bytes. On Err do NOT panic / default — convert to a collect-all
    // finding and refuse.
    let calc = match read_calc_pr(original_bytes) {
        Ok(c) => c,
        Err(err) => {
            findings.push(provenance_error_to_finding(&err, &sheet));
            return (unread_provenance(map), Err(findings));
        },
    };
    let app = match read_app_props(original_bytes) {
        Ok(a) => a,
        Err(err) => {
            findings.push(provenance_error_to_finding(&err, &sheet));
            return (unread_provenance(map), Err(findings));
        },
    };

    // ECMA-376 defaults: absent calcMode => "auto"; absent fullCalcOnLoad =>
    // false; calcId stays None.
    let calc_mode = calc.calc_mode.unwrap_or_else(|| "auto".to_string());
    let full_calc_on_load = calc.full_calc_on_load.unwrap_or(false);
    let calc_id = calc.calc_id;

    // WBCO-07: classify provenance from the ORIGINAL bytes. The refuse decision +
    // reason-code derive from this enum, never an ad-hoc check.
    let class = classify(
        app.application.as_deref(),
        app.app_version.as_deref(),
        calc_id,
    );

    // METADATA missing-cache: any is_formula cell whose cached value is None.
    let missing_cache_loc: Option<(String, String)> = map
        .sheets
        .iter()
        .flat_map(|s| s.cells.iter().map(move |c| (s, c)))
        .find(|(_, c)| c.is_formula && c.value.is_none())
        .map(|(s, c)| (s.name.clone(), c.addr.clone()));
    let missing_cache = missing_cache_loc.is_some();

    // Region hashes. On Err (role-less manifest) PUSH the finding into the
    // collect-all set and use a sentinel hash set; do NOT panic.
    let region_hashes = match compute_region_hashes(map, manifest) {
        Ok(h) => h,
        Err(finding) => {
            findings.push(finding);
            sentinel_region_hashes()
        },
    };

    let calc_id_ok = calc_id.is_some_and(|id| id != 0);
    let full_recalc_on_save = calc_mode == "auto" && !full_calc_on_load && calc_id_ok;
    let fresh = full_recalc_on_save && !missing_cache;
    let stale = !fresh;

    let provenance = OracleProvenance {
        authoring_app: app.application.clone(),
        app_version: app.app_version.clone(),
        calc_mode: calc_mode.clone(),
        full_calc_on_load,
        calc_id,
        save_timestamp: map.save_timestamp.clone(),
        region_hashes,
        missing_cache,
        stale,
        full_recalc_on_save,
        force_full_calc: calc.force_full_calc.unwrap_or(false),
        class,
    };

    // COLLECT-ALL findings (NOT fail-fast). Each is a Severity::Error oracle/*
    // located finding. The freshness/identity/coherence signals live in one
    // helper so this orchestrator stays a thin pipeline.
    collect_freshness_findings(
        &mut findings,
        &FreshnessSignals {
            sheet: &sheet,
            full_calc_on_load,
            calc_mode: &calc_mode,
            calc_id_ok,
            missing_cache_loc: missing_cache_loc.as_ref(),
            class,
            app_name: app.application.as_deref(),
            stale,
        },
    );

    // TRUSTED-FIXTURE FRESHNESS SOFTENING (#[cfg(test)] path ONLY): a committed
    // trusted fixture authored by a non-Excel WRITER (e.g. rust_xlsxwriter, which
    // hard-codes `fullCalcOnLoad=1` and writes no Excel recalc stamp) trips the
    // STALENESS freshness findings even though its cached `<v>` values are the
    // authored oracle. When the override is present those STALENESS signals are
    // DEMOTED to Warning so the proof can compile the fixture — they SURVIVE as
    // recorded evidence, never silently dropped. The IDENTITY refusal
    // (`oracle/non-excel-app`) and the STRUCTURAL corruption findings
    // (`oracle/unreadable-provenance`, `oracle/missing-provenance`,
    // `oracle/missing-manifest`) are NOT in the softenable set: a fabricated/
    // non-Excel/malformed workbook still hard-refuses even with the override
    // (CR-01). Production [`gate`] passes `false`, so this never runs on the
    // production path.
    if soften_staleness {
        for f in &mut findings {
            if SOFTENABLE_FRESHNESS_RULES.contains(&f.rule.as_str()) {
                f.severity = Severity::Warning;
            }
        }
    }

    // Accept/refuse decision: any Error-severity finding refuses.
    if findings.iter().any(|f| f.severity == Severity::Error) {
        return (provenance, Err(findings));
    }

    // ACCEPT: produce the full corpus over EVERY CellRecord whose value is Some(_).
    (provenance, Ok(build_oracle_corpus(map)))
}

/// The freshness/identity/coherence inputs for [`collect_freshness_findings`],
/// borrowed from the gate body so no value is copied or re-derived.
struct FreshnessSignals<'a> {
    /// The sheet name the located findings attach to.
    sheet: &'a str,
    /// `calcPr@fullCalcOnLoad` — `true` hard-refuses (`oracle/stale-cache`).
    full_calc_on_load: bool,
    /// The effective `calcMode` (ECMA-376 default applied); only `"auto"` is trusted.
    calc_mode: &'a str,
    /// `true` when a non-zero `calcId` full-recalc stamp is present.
    calc_id_ok: bool,
    /// `Some((sheet, addr))` of the first formula cell lacking a cached value.
    missing_cache_loc: Option<&'a (String, String)>,
    /// The classified provenance; anything but `ExcelTrusted` refuses identity.
    class: ProvenanceClass,
    /// `<Application>` text, for the identity refusal message.
    app_name: Option<&'a str>,
    /// `true` when the cache is classified stale (drives the coherence backstop).
    stale: bool,
}

/// Push every COLLECT-ALL `oracle/*` finding evidenced by `s` into `findings`.
///
/// Preserves the exact set, order, severity, and messages of the inlined gate
/// checks: `fullCalcOnLoad`, non-`auto` `calcMode`, absent/zero `calcId`,
/// missing-cache, the WBCO-07 identity refusal, and the stale-cache coherence
/// backstop (which fires only when no other Error was already recorded).
fn collect_freshness_findings(findings: &mut Vec<LintFinding>, s: &FreshnessSignals<'_>) {
    // fullCalcOnLoad == true → hard refuse.
    if s.full_calc_on_load {
        findings.push(LintFinding::new(
            Severity::Error,
            "oracle/stale-cache",
            s.sheet.to_string(),
            None,
            "the workbook is flagged for a full recalc on load \
             (fullCalcOnLoad=1) — its cached values are not trusted",
            "Re-open in Excel and save with a full recalc (this clears \
             fullCalcOnLoad).",
        ));
    }

    // Only calcMode == "auto" evidences a genuine full recalc.
    if s.calc_mode != "auto" {
        let calc_mode = s.calc_mode;
        findings.push(LintFinding::new(
            Severity::Error,
            "oracle/no-recalc",
            s.sheet.to_string(),
            None,
            format!(
                "calculation mode is `{calc_mode}` — not the trusted automatic \
                 full-recalc mode, so the cache may not reflect the formulas"
            ),
            "Set Calculation Options to Automatic in Excel, recalc (F9), and save.",
        ));
    }

    // calcId absent or zero → no-recalc.
    if !s.calc_id_ok {
        findings.push(LintFinding::new(
            Severity::Error,
            "oracle/no-recalc",
            s.sheet.to_string(),
            None,
            "no non-zero calcId — no full-recalc stamp is recorded for this cache",
            "Re-save the workbook from Microsoft Excel so a full-recalc stamp is \
             recorded.",
        ));
    }

    // A formula cell with no cached <v> → missing-cache, LOCATED.
    if let Some((m_sheet, m_addr)) = s.missing_cache_loc {
        findings.push(LintFinding::new(
            Severity::Error,
            "oracle/missing-cache",
            m_sheet.clone(),
            Some(m_addr.clone()),
            format!(
                "cell {m_addr} has a formula but no cached value — the oracle \
                 cache is incomplete"
            ),
            format!(
                "Cell {m_addr} has a formula but no cached value; recalc (F9) and save from Excel."
            ),
        ));
    }

    // WBCO-07 + CR-01: the provenance CLASS decides the identity refusal.
    push_identity_finding(findings, s.class, s.app_name, s.sheet);

    // Coherence backstop: a stale cache can never be admitted, even if a future
    // calc-axis slips past the enumerated findings.
    let has_errors = findings.iter().any(|f| f.severity == Severity::Error);
    if s.stale && !has_errors {
        findings.push(LintFinding::new(
            Severity::Error,
            "oracle/stale-cache",
            s.sheet.to_string(),
            None,
            "the workbook cache is classified stale but no specific signal was \
             reported — refusing fail-closed",
            "Re-save the workbook from Microsoft Excel with a full recalc.",
        ));
    }
}

/// Push the WBCO-07/CR-01 identity refusal when `class` is not `ExcelTrusted`.
///
/// A non-trusted class (`NonExcel` / `UmyaFabricated` / `UnknownStale`) is ALWAYS
/// REFUSED with `oracle/non-excel-app` — it is not in [`SOFTENABLE_FRESHNESS_RULES`]
/// so the test trusted-fixture override never demotes it. `ExcelTrusted` pushes
/// nothing.
fn push_identity_finding(
    findings: &mut Vec<LintFinding>,
    class: ProvenanceClass,
    app_name: Option<&str>,
    sheet: &str,
) {
    if class == ProvenanceClass::ExcelTrusted {
        return;
    }
    let app_name = app_name.unwrap_or("an unknown application");
    let reason = match class {
        ProvenanceClass::UmyaFabricated => format!(
            "the workbook carries a FABRICATED Excel identity ({app_name}, \
             calcId/AppVersion match the umya writer fingerprint), not a \
             genuine Excel save — its cached values were not Excel-computed"
        ),
        ProvenanceClass::NonExcel => format!(
            "the workbook was last saved by {app_name}, not a recognized \
             Microsoft Excel application — its cached values were not \
             Excel-computed"
        ),
        ProvenanceClass::UnknownStale => {
            "the workbook's authoring application could not be determined — \
             refusing fail-closed (provenance is not a genuine Excel save)"
                .to_string()
        },
        ProvenanceClass::ExcelTrusted => unreachable!("guarded by the early return"),
    };
    findings.push(LintFinding::new(
        Severity::Error,
        "oracle/non-excel-app",
        sheet.to_string(),
        None,
        reason,
        "Re-save the workbook from Microsoft Excel for Windows or Mac (a \
         genuine Excel save carries an Excel AppVersion build string and a \
         real calcId).",
    ));
}

/// Build the accept-path [`OracleCorpus`] over EVERY `CellRecord` whose value is
/// `Some(_)`, keyed by `cell_key`.
fn build_oracle_corpus(map: &WorkbookMap) -> OracleCorpus {
    let cells = map
        .sheets
        .iter()
        .flat_map(|s| s.cells.iter().map(move |c| (s, c)))
        .filter_map(|(s, c)| {
            c.value
                .as_ref()
                .map(|v| (cell_key(&s.name, &c.addr), v.clone()))
        })
        .collect();
    OracleCorpus { cells }
}

/// Build an [`OracleProvenance`] for the raw-read-FAILED path: the raw-derived
/// fields stay at their unread defaults (we do NOT fabricate Excel-looking
/// values), only the umya-surfaced `save_timestamp` is carried. `stale = true`
/// (fail-closed), the class is `UnknownStale`, and region hashes are the
/// sentinel (no roles partitioned).
fn unread_provenance(map: &WorkbookMap) -> OracleProvenance {
    OracleProvenance {
        authoring_app: None,
        app_version: None,
        calc_mode: "auto".to_string(),
        full_calc_on_load: false,
        calc_id: None,
        save_timestamp: map.save_timestamp.clone(),
        region_hashes: sentinel_region_hashes(),
        missing_cache: false,
        stale: true,
        full_recalc_on_save: false,
        force_full_calc: false,
        class: ProvenanceClass::UnknownStale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::{CellRecord, FormulaKind, SheetRecord};
    use crate::provenance::raw_parts::zip_with;
    use crate::{CellRole, Dtype, Role};
    use proptest::prelude::*;

    const WORKBOOK_PART: &str = "xl/workbook.xml";
    const APP_PART: &str = "docProps/app.xml";

    /// Build an in-memory `.xlsx` with the given workbook.xml + app.xml bodies.
    fn xlsx(workbook: &str, app: &str) -> Vec<u8> {
        zip_with(&[
            (WORKBOOK_PART, workbook.as_bytes()),
            (APP_PART, app.as_bytes()),
        ])
    }

    fn cell(addr: &str, value: Option<&str>, formula: Option<&str>) -> CellRecord {
        CellRecord {
            addr: addr.to_string(),
            formula: formula.map(|s| s.to_string()),
            value: value.map(|s| s.to_string()),
            fill_argb: None,
            font_argb: None,
            number_format: None,
            is_formula: formula.is_some(),
            formula_kind: FormulaKind::Normal,
        }
    }

    fn map_with(cells: Vec<CellRecord>) -> WorkbookMap {
        WorkbookMap {
            sheets: vec![SheetRecord {
                name: "S".to_string(),
                state: "visible".to_string(),
                hidden_rows: vec![],
                hidden_cols: vec![],
                col_widths: vec![],
                merges: vec![],
                cf_ranges: vec![],
                tables: vec![],
                table_records: vec![],
                data_validations: vec![],
                notes: vec![],
                cells,
            }],
            defined_names: vec![],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        }
    }

    fn role(cell: &str, role: Role) -> CellRole {
        CellRole {
            cell: cell.to_string(),
            role,
            name: None,
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

    fn manifest() -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "tax-calc".to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: None,
            ratified_at: None,
            cells: vec![role("S!A1", Role::Input), role("S!C1", Role::Output)],
            loop_block: None,
            governed_data: Vec::new(),
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        }
    }

    /// A clean, value-only map (no formula cells ⇒ no missing-cache).
    fn clean_map() -> WorkbookMap {
        map_with(vec![
            cell("A1", Some("10"), None),
            cell("C1", Some("99"), None),
        ])
    }

    fn has_rule(findings: &[LintFinding], rule: &str) -> bool {
        findings.iter().any(|f| f.rule == rule)
    }

    // --- classify() unit coverage (the WBCO-07 decision) ---

    #[test]
    fn classify_excel_trusted() {
        // Anchored Microsoft Excel name + a positive AppVersion build + a
        // non-sentinel calcId ⇒ ExcelTrusted.
        let c = classify(Some("Microsoft Excel"), Some("16.0300"), Some(191029));
        assert_eq!(c, ProvenanceClass::ExcelTrusted);
    }

    #[test]
    fn classify_excel_for_mac_trusted() {
        // Excel for Mac writes this exact OOXML Application identity. It is a
        // genuine Excel save and must be admitted by the same positive-marker
        // checks as Excel for Windows.
        let c = classify(
            Some("Microsoft Macintosh Excel"),
            Some("16.0300"),
            Some(191029),
        );
        assert_eq!(c, ProvenanceClass::ExcelTrusted);
    }

    #[test]
    fn classify_umya_fabricated() {
        // umya fingerprint: anchored name, NO AppVersion, sentinel calcId.
        let c = classify(Some("Microsoft Excel"), None, Some(UMYA_SENTINEL_CALC_ID));
        assert_eq!(c, ProvenanceClass::UmyaFabricated);
    }

    #[test]
    fn classify_non_excel() {
        let c = classify(Some("LibreOffice/24.2"), Some("1.0"), Some(191029));
        assert_eq!(c, ProvenanceClass::NonExcel);
    }

    #[test]
    fn classify_unknown_stale_when_app_absent() {
        let c = classify(None, None, Some(191029));
        assert_eq!(c, ProvenanceClass::UnknownStale);
    }

    #[test]
    fn classify_sentinel_calc_id_alone_does_not_refuse_real_excel() {
        // False-positive policy: a real Excel build string admits even a sentinel
        // calcId — the sentinel ALONE never refuses.
        let c = classify(
            Some("Microsoft Excel"),
            Some("16.0300"),
            Some(UMYA_SENTINEL_CALC_ID),
        );
        assert_eq!(c, ProvenanceClass::ExcelTrusted);
    }

    #[test]
    fn classify_anchored_not_contains() {
        // "Not Microsoft Excel" must NOT pass the anchored check.
        assert_eq!(
            classify(Some("Not Microsoft Excel"), Some("16.0"), Some(1)),
            ProvenanceClass::NonExcel
        );
    }

    #[test]
    fn classify_rejects_excel_prefix_spoofs() {
        for spoofed in ["Microsoft Excelerator", "Microsoft Macintosh Excelerator"] {
            assert_eq!(
                classify(Some(spoofed), Some("16.0"), Some(1)),
                ProvenanceClass::NonExcel,
                "a known application-name prefix is not sufficient: {spoofed}"
            );
        }
    }

    proptest! {
        #[test]
        fn classify_rejects_arbitrary_suffix_spoofs(
            suffix in "[A-Za-z0-9_-]{1,32}"
        ) {
            for known in MICROSOFT_EXCEL_APPLICATIONS {
                let spoofed = format!("{known}{suffix}");
                prop_assert_eq!(
                    classify(Some(&spoofed), Some("16.0"), Some(1)),
                    ProvenanceClass::NonExcel
                );
            }
        }
    }

    // --- gate() behavior tests (the seven plan-required behaviors) ---

    #[test]
    fn classify_excel_trusted_is_accepted() {
        // A genuine Excel fixture (AppVersion build + non-sentinel calcId, auto
        // calcMode, no missing cache) is ACCEPTED → Ok(OracleCorpus).
        let wb = r#"<?xml version="1.0"?><workbook><calcPr calcMode="auto" calcId="191029"/></workbook>"#;
        let app = r#"<?xml version="1.0"?><Properties><Application>Microsoft Excel</Application><AppVersion>16.0300</AppVersion></Properties>"#;
        let bytes = xlsx(wb, app);
        let (prov, result) = gate(&bytes, &clean_map(), &manifest());
        assert_eq!(prov.class, ProvenanceClass::ExcelTrusted);
        let corpus = result.expect("a trusted, fresh Excel workbook is accepted");
        assert_eq!(corpus.cells.get("S!A1").map(String::as_str), Some("10"));
    }

    #[test]
    fn classify_excel_for_mac_is_accepted() {
        // Regression: production workbooks saved by Excel for Mac identify as
        // "Microsoft Macintosh Excel", not "Microsoft Excel".
        let wb = r#"<?xml version="1.0"?><workbook><calcPr calcMode="auto" calcId="191029"/></workbook>"#;
        let app = r#"<?xml version="1.0"?><Properties><Application>Microsoft Macintosh Excel</Application><AppVersion>16.0300</AppVersion></Properties>"#;
        let bytes = xlsx(wb, app);
        let (prov, result) = gate(&bytes, &clean_map(), &manifest());
        assert_eq!(prov.class, ProvenanceClass::ExcelTrusted);
        result.expect("a trusted, fresh Excel for Mac workbook is accepted");
    }

    #[test]
    fn classify_umya_fabricated_refused() {
        // A umya-authored workbook: Microsoft Excel name + calcId=122211 + NO
        // AppVersion build ⇒ UmyaFabricated ⇒ REFUSED with oracle/non-excel-app.
        let wb = format!(
            r#"<?xml version="1.0"?><workbook><calcPr calcId="{UMYA_SENTINEL_CALC_ID}"/></workbook>"#
        );
        let app = r#"<?xml version="1.0"?><Properties><Application>Microsoft Excel</Application></Properties>"#;
        let bytes = xlsx(&wb, app);
        let (prov, result) = gate(&bytes, &clean_map(), &manifest());
        assert_eq!(prov.class, ProvenanceClass::UmyaFabricated);
        let findings = result.expect_err("umya-fabricated provenance must be refused");
        assert!(
            has_rule(&findings, "oracle/non-excel-app"),
            "got {findings:?}"
        );
        // No accepted OracleProvenance ever carries the sentinel calcId on a
        // trusted path.
        assert_eq!(prov.calc_id, Some(UMYA_SENTINEL_CALC_ID));
        assert_ne!(prov.class, ProvenanceClass::ExcelTrusted);
    }

    #[test]
    fn classify_non_excel_refused() {
        let wb = r#"<?xml version="1.0"?><workbook><calcPr calcMode="auto" calcId="191029"/></workbook>"#;
        let app = r#"<?xml version="1.0"?><Properties><Application>LibreOffice/24.2</Application><AppVersion>1.0</AppVersion></Properties>"#;
        let bytes = xlsx(wb, app);
        let (prov, result) = gate(&bytes, &clean_map(), &manifest());
        assert_eq!(prov.class, ProvenanceClass::NonExcel);
        let findings = result.expect_err("non-Excel provenance must be refused");
        assert!(
            has_rule(&findings, "oracle/non-excel-app"),
            "got {findings:?}"
        );
    }

    #[test]
    fn classify_unknown_stale_refused() {
        // app.xml present but with no <Application> ⇒ UnknownStale ⇒ REFUSE
        // fail-closed (never defaults to trusted).
        let wb = r#"<?xml version="1.0"?><workbook><calcPr calcMode="auto" calcId="191029"/></workbook>"#;
        let app = r#"<?xml version="1.0"?><Properties></Properties>"#;
        let bytes = xlsx(wb, app);
        let (prov, result) = gate(&bytes, &clean_map(), &manifest());
        assert_eq!(prov.class, ProvenanceClass::UnknownStale);
        let findings = result.expect_err("unknown provenance must fail closed");
        assert!(
            has_rule(&findings, "oracle/non-excel-app"),
            "got {findings:?}"
        );
    }

    #[test]
    fn override_admits_genuine_excel_but_stale_fixture() {
        // A GENUINE Excel identity (anchored name + AppVersion build + non-sentinel
        // calcId ⇒ ExcelTrusted) whose ONLY problem is a staleness signal
        // (fullCalcOnLoad=1 ⇒ oracle/stale-cache): the test override DEMOTES the
        // staleness signal so the committed-fixture proof can compile, but the
        // identity axis was never in question.
        let wb = r#"<?xml version="1.0"?><workbook><calcPr calcMode="auto" calcId="124519" fullCalcOnLoad="1"/></workbook>"#;
        let app = r#"<?xml version="1.0"?><Properties><Application>Microsoft Excel</Application><AppVersion>12.0000</AppVersion></Properties>"#;
        let bytes = xlsx(wb, app);
        let m = clean_map();
        let mani = manifest();

        // Production path: REFUSED on the staleness signal (override is None).
        let (_p, prod) = gate(&bytes, &m, &mani);
        let prod_findings = prod.expect_err("production must refuse a fullCalcOnLoad=1 cache");
        assert!(has_rule(&prod_findings, "oracle/stale-cache"));

        // Test override path: the genuine-Excel staleness signal is demoted, so the
        // SAME bytes accept.
        let (_p2, overridden) =
            gate_with_fixture_override(&bytes, &m, &mani, Some(TrustedFixtureMarker));
        assert!(
            overridden.is_ok(),
            "the test override demotes the genuine-Excel staleness signal, got {overridden:?}"
        );
    }

    #[test]
    fn override_cannot_soften_fabricated_identity() {
        // CR-01 defense-in-depth: even WITH the test override, umya-fabricated
        // identity (Microsoft Excel name + sentinel calcId + NO AppVersion ⇒
        // UmyaFabricated ⇒ oracle/non-excel-app) is STILL refused — the override may
        // only soften genuine staleness, never the fabricated-identity refusal.
        let wb = format!(
            r#"<?xml version="1.0"?><workbook><calcPr calcMode="auto" calcId="{UMYA_SENTINEL_CALC_ID}"/></workbook>"#
        );
        let app = r#"<?xml version="1.0"?><Properties><Application>Microsoft Excel</Application></Properties>"#;
        let bytes = xlsx(&wb, app);
        let m = clean_map();
        let mani = manifest();

        // Production path: REFUSED (override is None).
        let (_p, prod) = gate(&bytes, &m, &mani);
        let prod_findings = prod.expect_err("production must refuse umya-fabricated bytes");
        assert!(has_rule(&prod_findings, "oracle/non-excel-app"));

        // Test override path: STILL refused — oracle/non-excel-app is no longer
        // softenable, so the fabricated identity stays a blocking Error.
        let (_p2, overridden) =
            gate_with_fixture_override(&bytes, &m, &mani, Some(TrustedFixtureMarker));
        let over_findings =
            overridden.expect_err("the override must NOT soften fabricated identity (CR-01)");
        assert!(
            has_rule(&over_findings, "oracle/non-excel-app"),
            "fabricated identity stays a blocking Error even under the override, got {over_findings:?}"
        );
    }

    #[test]
    fn malformed_xlsx_fails_closed() {
        // Truncated/garbage ZIP bytes produce an oracle/* Error finding, no panic.
        let garbage = b"PK\x03\x04 not really a zip at all";
        let (prov, result) = gate(garbage, &clean_map(), &manifest());
        assert_eq!(
            prov.class,
            ProvenanceClass::UnknownStale,
            "fail-closed class"
        );
        let findings = result.expect_err("malformed bytes must fail closed");
        assert!(
            has_rule(&findings, "oracle/unreadable-provenance"),
            "got {findings:?}"
        );
    }

    #[test]
    fn zip_bomb_fails_closed() {
        // An over-size workbook.xml part yields a PartTooLarge → fail-closed
        // oracle/unreadable-provenance finding (no unbounded allocation, no panic).
        let mut big = Vec::new();
        big.extend_from_slice(br#"<?xml version="1.0"?><workbook><calcPr calcMode="auto"/>"#);
        big.resize(super::super::raw_parts::MAX_ZIP_ENTRY_BYTES + 512, b' ');
        big.extend_from_slice(b"</workbook>");
        let app = r#"<?xml version="1.0"?><Properties><Application>Microsoft Excel</Application><AppVersion>16.0</AppVersion></Properties>"#;
        let bytes = zip_with(&[(WORKBOOK_PART, big.as_slice()), (APP_PART, app.as_bytes())]);
        let (_prov, result) = gate(&bytes, &clean_map(), &manifest());
        let findings = result.expect_err("a zip bomb must fail closed");
        assert!(
            has_rule(&findings, "oracle/unreadable-provenance"),
            "got {findings:?}"
        );
    }
}
