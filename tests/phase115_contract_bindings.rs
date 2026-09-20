//! Deterministic, pmat-independent binding-drift gate for the Phase 115 contract
//! equations.
//!
//! # Why this file exists
//!
//! `CLAUDE.md` § "Contract-First Development" requires the contract to be written
//! before the code. A contract that nothing resolves is a claim nobody checks:
//! before this file, **nothing in this repository read `contracts/binding.yaml`
//! at all**. `make comply`'s `comply-bindings-check` (`Makefile:818`) resolves
//! only `contracts/team-servers/binding.yaml`, and `pmat comply check --path .`
//! is informational here by design (`Makefile:797-808`, `CLAUDE.md` D-07 — the
//! repo is intentionally mid-migration at the project level, so its holistic
//! exit is never propagated into the gate). A Phase 115 binding could therefore
//! name a function nobody ever wrote and every gate would stay green.
//!
//! This file is the missing resolver, in the same shape as the Makefile gate it
//! mirrors: every `function:` must resolve to a real declaration in the crate
//! source implied by its `module_path:`.
//!
//! # The `planned` status, and why it is fenced
//!
//! Phase 115 wave 1 wrote the contract BEFORE the implementation plans ran, so
//! twelve of its thirteen bindings landed as `status: planned` — the functions did
//! not exist yet. `planned` is an honest statement of that, not an exemption:
//! [`phase115_contract_bindings_planned_entries_are_scoped_to_phase_115`] confines
//! `planned` to exactly the three Phase 115 equations, so it cannot become a
//! universal escape hatch for unrelated binding drift.
//!
//! **115-10 (wave 6) flipped every one of them to `implemented`**, so the file now
//! carries ZERO `planned` bindings and
//! [`phase115_contract_bindings_every_implemented_binding_resolves_to_real_source`]
//! is load-bearing over all fourteen Phase 115 entries (thirteen from wave 1 plus
//! `compile_for_era`, which 115-03 delivered without a contract entry).
//!
//! That end state is why the anti-vacuity assertion in
//! [`phase115_contract_bindings_planned_entries_are_scoped_to_phase_115`] cannot be
//! `planned > 0`, which is how wave 1 wrote it: that predicate was true only while
//! the implementation plans were unlanded and became FALSE at exactly the moment
//! the section reached its intended state. The invariant that survives is that the
//! Phase 115 SECTION is still present and still parses — `planned` is a transient
//! property of it, not an invariant.
//!
//! # Phase 116 extended the permission, on purpose and after observing the failure
//!
//! The `planned` scope test's own failure message says a later phase needing
//! contract-first `planned` bindings must extend the permitted list "deliberately in
//! this file — that edit is the conversation this test exists to force". Phase 116
//! (OAuth client hardening) is that phase: its wave-1 plan 116-01 authored three
//! OAuth equations and eight `status: planned` bindings BEFORE any of its `src/`
//! code existed. The plan predicted `make comply` would be unaffected — correctly,
//! `comply-bindings-check` reads only the team-servers binding file — but did not
//! know THIS file existed. So the test fired, named all eight entries, and the fix
//! was made with that failure in hand rather than in anticipation of it.
//!
//! [`PHASE_116_EQUATIONS`] is a second ENUMERATION, not a widening of the first: two
//! named lists, joined by [`planned_is_permitted`]. A phase that wants the same
//! permission adds a third list and one `||`, which is a reviewable diff; nothing
//! here derives the permitted set from the binding file itself, which would make the
//! check circular.
//!
//! # The two legacy ledgers
//!
//! Both files are read at RUNTIME — never baked in at compile time with the
//! `include_str` macro (spelled without its bang here so a grep for real uses
//! finds none) — so a contract edit
//! moves these assertions without a rebuild. Running the checks against the tree
//! as it stands today surfaced pre-existing drift that predates Phase 115:
//! one `implemented` binding whose `function:` is not a Rust identifier, and 21
//! bound equations that have no definition in `contracts/mcp-protocol-sdk-v1.yaml`
//! (the `pmcp-server-toolkit` equations from Phase 83, bound but never written
//! into a contract file). Rather than weaken the gate to accommodate them, each
//! is enumerated in a FROZEN ledger below. A ledger entry that is no longer
//! drifted fails too — so the ledgers can only shrink, and a NEW ghost binding
//! or a NEW uncontracted equation still fails immediately.
//!
//! Every test name is prefixed with this file's stem so both
//! `binary(phase115_contract_bindings)` and `test(/phase115_contract_bindings/)`
//! select them — a `test(/stem/)` selector against a file whose test names lack
//! the stem selects ZERO tests and exits 0.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// The binding file this gate resolves. Nothing else in the repo reads it.
const BINDING_FILE: &str = "contracts/binding.yaml";

/// The contract file whose `equations:` map the bindings must reference.
const CONTRACT_FILE: &str = "contracts/mcp-protocol-sdk-v1.yaml";

/// The three equations Phase 115 adds.
const PHASE_115_EQUATIONS: &[&str] = &[
    "output_schema_draft_pin",
    "structured_content_shape",
    "result_caching_hints",
];

/// The three equations Phase 116 adds, authored in its wave 1 (plan 116-01)
/// before any of its `src/` code existed.
///
/// This list is the deliberate edit the failure message of
/// [`phase115_contract_bindings_planned_entries_are_scoped_to_phase_115`] demands
/// of a phase that genuinely needs contract-first `planned` bindings — "that edit
/// is the conversation this test exists to force". The conversation happened:
/// 116-01 authored `contracts/binding.yaml`'s eight OAuth bindings, RAN this test,
/// OBSERVED it name all eight, and only then added this constant. It is an
/// enumeration of three named equations, never a predicate over a `contract:` or a
/// filename, so a fourth equation cannot join it by accident.
///
/// Plan 116-15 flips those eight bindings to `implemented` after resolving each
/// `function:` against real source, at which point every entry here is expected to
/// carry ZERO `planned` bindings — the same end state Phase 115 reached.
const PHASE_116_EQUATIONS: &[&str] = &[
    "oauth_authorization_response_validation",
    "oauth_discovery_anchor",
    "oauth_credential_binding",
];

/// Every equation on which a `planned` binding is permitted, and nothing else.
fn planned_is_permitted(equation: &str) -> bool {
    PHASE_115_EQUATIONS.contains(&equation) || PHASE_116_EQUATIONS.contains(&equation)
}

/// How many bindings each Phase 115 equation must carry, as a MINIMUM.
///
/// A minimum rather than an exact count, so a later plan that binds one more
/// function needs no edit here — but a truncated or mis-parsed file does fail.
const EXPECTED_PHASE_115_BINDINGS: &[(&str, usize)] = &[
    ("output_schema_draft_pin", 5),
    ("structured_content_shape", 2),
    ("result_caching_hints", 6),
];

/// One row per crate: the prefix as spelled in `module_path:`, the source root
/// it maps to, and an anti-vacuity floor on the file count under that root.
///
/// A `module_path:` whose crate prefix is absent from this table FAILS rather
/// than silently resolving against nothing: adding a crate to the binding file
/// must be a deliberate edit here too.
const SOURCE_ROOTS: &[(&str, &str, usize)] = &[
    ("pmcp", "src", 50),
    ("pmcp_server_toolkit", "crates/pmcp-server-toolkit/src", 5),
];

/// A floor on the parsed record count, so a silently-broken parser fails HERE
/// rather than passing every other check over an empty set. 60 records exist
/// today (47 pre-Phase-115 + 13 added by 115-11).
const MINIMUM_BINDINGS: usize = 40;

/// FROZEN ledger — `implemented` bindings that do not resolve, measured at
/// Phase 115 wave 1, before any Phase 115 production code was written.
///
/// One entry: `ErrorCode constants` is not a Rust identifier at all — the
/// `function:` value names a GROUP of associated constants in prose. Recorded
/// rather than resolved because rewriting a pre-existing binding is outside
/// 115-11's zero-production-bytes scope; 115-10 books it as a deferred item.
const LEGACY_UNRESOLVED: &[(&str, &str)] = &[("error_code_mapping", "ErrorCode constants")];

/// FROZEN ledger — equations that bindings reference but that no
/// `contracts/mcp-protocol-sdk-v1.yaml` `equations:` entry defines.
///
/// All 21 are `pmcp-server-toolkit` equations bound in Phase 83+ against a
/// contract that was never written. Measured, not invented: this is the exact
/// set present before Phase 115 touched the file. Any 22nd fails.
const LEGACY_UNCONTRACTED_EQUATIONS: &[&str] = &[
    "auth_provider_trait",
    "auth_static_provider",
    "code_mode_prompt_assembly",
    "code_mode_register_tools",
    "code_mode_validation_pipeline",
    "config_strict_parse",
    "config_strict_validated",
    "config_validate",
    "hmac_token_reexport",
    "prompt_handlers_from_config",
    "secret_value",
    "secret_value_new",
    "secrets_provider_trait",
    "server_builder_ext",
    "server_builder_ext_code_mode",
    "server_builder_ext_fallible",
    "sql_connector_trait",
    "sql_dialect",
    "static_prompt_handler",
    "static_resource_handler",
    "tool_synthesis",
];

/// The declaration forms a `function:` value may resolve to. `function:` is the
/// binding file's key name; the value is often a type or a constant, which is
/// why this is not just `fn`.
const DECLARATION_KEYWORDS: &[&str] = &[
    "fn", "enum", "struct", "trait", "const", "static", "type", "union",
];

// ===========================================================================
// Primitives
// ===========================================================================

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Path relative to the crate root, for failure messages a reader can act on.
fn rel(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

/// Read at RUNTIME, deliberately not compile-time `include_str`: a binding edit must move
/// these assertions without anyone remembering to rebuild.
fn read(relative: &str) -> String {
    let full = repo_root().join(relative);
    fs::read_to_string(&full).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}\n\
             FAILURE MODE: this gate resolves the contract bindings; if the file is gone, so is \
             every assertion in it.\n\
             WHAT TO DO: restore the file — do not delete this test.",
            rel(&full)
        )
    })
}

/// `true` when `needle` occurs in `haystack` bounded by non-identifier
/// characters on both sides (the Rust-token equivalent of `grep -w`).
fn contains_word(haystack: &str, needle: &str) -> bool {
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let bytes = haystack.as_bytes();
    let mut from = 0;
    while from <= haystack.len() {
        let Some(offset) = haystack[from..].find(needle) else {
            return false;
        };
        let start = from + offset;
        let end = start + needle.len();
        let left_ok = start == 0 || !is_ident(bytes[start - 1]);
        let right_ok = end >= bytes.len() || !is_ident(bytes[end]);
        if left_ok && right_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

/// `true` when `text` declares `symbol`, or re-exports it with `pub use`.
///
/// The declaration arm mirrors the Makefile gate's `grep -rqE "fn <name>\b"`
/// idiom, widened to the other item keywords. The re-export arm exists because
/// two measured bindings (`AuthProvider`, `HmacTokenGenerator`) name symbols
/// that `pmcp-server-toolkit` deliberately re-exports rather than defines —
/// its `module_path:` is the crate root, and a crate-root re-export is a real
/// resolution, not drift.
/// The `"{keyword} {symbol}"` needles for one symbol, built once.
///
/// Split out from [`declares_with`] because the caller tests one symbol against
/// every file in the corpus: building the needles per file made the count
/// `bindings x files x keywords` (~42k allocations per run) when it is really
/// `bindings x keywords` (~460).
fn declaration_needles(symbol: &str) -> Vec<String> {
    DECLARATION_KEYWORDS
        .iter()
        .map(|keyword| format!("{keyword} {symbol}"))
        .collect()
}

/// [`declares`], but over needles the caller already built.
fn declares_with(text: &str, needles: &[String], symbol: &str) -> bool {
    for needle in needles {
        if contains_word(text, needle) {
            return true;
        }
    }
    let mut from = 0;
    while let Some(offset) = text[from..].find("pub use") {
        let start = from + offset;
        let end = text[start..]
            .find(';')
            .map_or(text.len(), |semi| start + semi);
        if contains_word(&text[start..end], symbol) {
            return true;
        }
        from = end.max(start + 1);
    }
    false
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", rel(dir).as_str()));
    for entry in entries {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Every registered crate's `.rs` sources, discovered at RUNTIME so a NEW file
/// cannot escape the scan by nobody remembering to list it.
fn source_index() -> BTreeMap<&'static str, Vec<String>> {
    let mut index = BTreeMap::new();
    for (crate_prefix, root, floor) in SOURCE_ROOTS {
        let dir = repo_root().join(root);
        let mut files = Vec::new();
        collect_rs_files(&dir, &mut files);
        files.sort();
        assert!(
            files.len() >= *floor,
            "FAILURE MODE: walking {} found {} .rs files, fewer than the {} floor — the walk is \
             broken and every binding below would be reported as a ghost.\n\
             WHAT TO DO: fix the walk or the SOURCE_ROOTS entry; do not lower the floor.",
            rel(&dir),
            files.len(),
            floor
        );
        let contents = files
            .iter()
            .map(|path| {
                fs::read_to_string(path)
                    .unwrap_or_else(|e| panic!("cannot read {}: {e}", rel(path)))
            })
            .collect();
        index.insert(*crate_prefix, contents);
    }
    index
}

// ===========================================================================
// The binding file reader
// ===========================================================================

/// One `- contract:` record from `contracts/binding.yaml`.
#[derive(Debug, Default, Clone)]
struct Binding {
    line: usize,
    equation: String,
    function: String,
    module_path: String,
    status: String,
}

fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    let unwrapped = trimmed
        .strip_prefix('\'')
        .and_then(|v| v.strip_suffix('\''))
        .or_else(|| trimmed.strip_prefix('"').and_then(|v| v.strip_suffix('"')))
        .unwrap_or(trimmed);
    unwrapped.to_string()
}

/// A line-oriented reader for the flat two-space shape the file actually uses.
///
/// Deliberately NOT a YAML crate: this gate must add no dependency (115-11's
/// threat register books `Cargo.toml` as byte-unchanged), and the shape it
/// parses is a fixed list of scalar keys. Only EXACTLY two spaces of indent are
/// read as a field, so a four-space folded `notes:` body containing the words
/// `status: planned` cannot be mistaken for one.
fn parse_bindings(text: &str) -> Vec<Binding> {
    let mut records: Vec<Binding> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        if raw.starts_with("- contract:") {
            records.push(Binding {
                line: index + 1,
                ..Binding::default()
            });
            continue;
        }
        let Some(current) = records.last_mut() else {
            continue;
        };
        let Some(field) = raw.strip_prefix("  ") else {
            continue;
        };
        if field.starts_with(' ') || field.starts_with('#') {
            continue;
        }
        let Some((key, value)) = field.split_once(':') else {
            continue;
        };
        match key {
            "equation" => current.equation = unquote(value),
            "function" => current.function = unquote(value),
            "module_path" => current.module_path = unquote(value),
            "status" => current.status = unquote(value),
            _ => {},
        }
    }
    records
}

fn bindings() -> Vec<Binding> {
    parse_bindings(&read(BINDING_FILE))
}

/// Every key of the contract's `equations:` map, read at RUNTIME.
///
/// Scoped to the `equations:` block: a two-space key elsewhere in the file
/// (`qa_gate:`'s `checks:`, for one) is not an equation and must not be
/// mistaken for one.
fn contract_equations() -> BTreeSet<String> {
    let text = read(CONTRACT_FILE);
    let mut names = BTreeSet::new();
    let mut inside = false;
    for line in text.lines() {
        if line.starts_with("equations:") {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if !line.is_empty() && !line.starts_with(' ') {
            break;
        }
        if let Some(field) = line.strip_prefix("  ") {
            if !field.starts_with(' ') && !field.starts_with('#') {
                if let Some(name) = field.strip_suffix(':') {
                    names.insert(name.to_string());
                }
            }
        }
    }
    assert!(
        names.len() >= 10,
        "FAILURE MODE: only {} equations parsed out of {CONTRACT_FILE} — the reader is broken and \
         the equation-existence check below would pass vacuously.\n\
         WHAT TO DO: fix the reader, not the assertion.",
        names.len()
    );
    names
}

// ===========================================================================
// 1. No ghost bindings
// ===========================================================================

#[test]
fn phase115_contract_bindings_every_implemented_binding_resolves_to_real_source() {
    let records = bindings();
    let index = source_index();
    let ledger: BTreeSet<(&str, &str)> = LEGACY_UNRESOLVED.iter().copied().collect();
    let mut ghosts = String::new();
    let mut still_drifted: BTreeSet<(&str, &str)> = BTreeSet::new();

    for record in &records {
        if record.status != "implemented" {
            continue;
        }
        let crate_prefix = record.module_path.split("::").next().unwrap_or_default();
        let Some(sources) = index.get(crate_prefix) else {
            writeln!(
                ghosts,
                "  {BINDING_FILE}:{} equation `{}` function `{}` — module_path `{}` names crate \
                 `{crate_prefix}`, which has no registered source root",
                record.line, record.equation, record.function, record.module_path
            )
            .expect("writing to a String cannot fail");
            continue;
        };
        let symbol = record
            .function
            .rsplit("::")
            .next()
            .unwrap_or(record.function.as_str());
        let needles = declaration_needles(symbol);
        if sources
            .iter()
            .any(|text| declares_with(text, &needles, symbol))
        {
            continue;
        }
        let key = (record.equation.as_str(), record.function.as_str());
        if ledger.contains(&key) {
            still_drifted.insert(key);
            continue;
        }
        let root = SOURCE_ROOTS
            .iter()
            .find(|(prefix, _, _)| *prefix == crate_prefix)
            .map_or("<unregistered>", |(_, root, _)| *root);
        writeln!(
            ghosts,
            "  {BINDING_FILE}:{} equation `{}` function `{}` (module_path `{}`) — no `fn`, \
             `enum`, `struct`, `trait`, `const`, `static` or `type` named `{symbol}`, and no \
             `pub use` re-export of it, anywhere under {root}/",
            record.line, record.equation, record.function, record.module_path
        )
        .expect("writing to a String cannot fail");
    }

    assert!(
        ghosts.is_empty(),
        "FAILURE MODE: GHOST BINDING — a binding marked `status: implemented` names a symbol that \
         does not exist. The contract claims a behaviour is implemented by a function nobody \
         wrote.\n\n{ghosts}\n\
         WHAT TO DO: write the function, or fix the `function:`/`module_path:` value to name the \
         real one. Do NOT flip the entry back to `status: planned` to make this pass — `planned` \
         means \"the owning Phase 115 plan has not landed yet\", and test \
         `phase115_contract_bindings_planned_entries_are_scoped_to_phase_115` rejects it on any \
         other equation. Do NOT delete this assertion."
    );

    for entry in &ledger {
        assert!(
            still_drifted.contains(entry),
            "FAILURE MODE: STALE LEDGER — LEGACY_UNRESOLVED still lists equation `{}` function \
             `{}`, but that binding is no longer an unresolved `implemented` entry (it now \
             resolves, was renamed, or was removed).\n\
             WHAT TO DO: delete that line from LEGACY_UNRESOLVED. The ledger records measured \
             pre-existing drift and may only shrink.",
            entry.0,
            entry.1
        );
    }
}

// ===========================================================================
// 2. `planned` cannot silence unrelated drift
// ===========================================================================

#[test]
fn phase115_contract_bindings_planned_entries_are_scoped_to_phase_115() {
    let records = bindings();
    let mut offenders = String::new();
    let mut planned = 0usize;

    for record in &records {
        if record.status != "planned" {
            continue;
        }
        planned += 1;
        if planned_is_permitted(&record.equation) {
            continue;
        }
        writeln!(
            offenders,
            "  {BINDING_FILE}:{} equation `{}` function `{}`",
            record.line, record.equation, record.function
        )
        .expect("writing to a String cannot fail");
    }

    assert!(
        offenders.is_empty(),
        "FAILURE MODE: `status: planned` was used outside the enumerated contract-first phases. \
         `planned` exempts a binding from the ghost-binding check, so on any other equation it is \
         a way to silence real drift.\n\n{offenders}\n\
         WHAT TO DO: either write the function and mark the binding `implemented`, or remove the \
         binding. If a future phase genuinely needs contract-first `planned` bindings, add its \
         equations as a NEW named constant in this file and extend `planned_is_permitted` — that \
         edit is the conversation this test exists to force. Phase 116 had it; see \
         PHASE_116_EQUATIONS."
    );

    // Anti-vacuity. Wave 1 wrote this as `planned > 0`, which held only while the
    // implementation plans were unlanded and went FALSE the moment 115-10 flipped
    // the last entry to `implemented` — i.e. the moment the section reached the
    // state it exists to reach. `planned` is transient; the presence of the Phase
    // 115 section is the invariant, so that is what is asserted here. A broken
    // parser or a deleted section still fails, which is the whole point.
    let phase_115_records = records
        .iter()
        .filter(|record| PHASE_115_EQUATIONS.contains(&record.equation.as_str()))
        .count();
    assert!(
        phase_115_records >= 13,
        "FAILURE MODE: only {phase_115_records} Phase 115 bindings parsed (expected at least 13). \
         Either the parser is broken or the Phase 115 section was removed from \
         {BINDING_FILE}.\n\
         NOTE: `planned` is EXPECTED to be zero here ({planned} parsed) — 115-10 flipped every \
         Phase 115 binding to `implemented`. Do not restore a `planned` entry to satisfy an \
         anti-vacuity check.\n\
         WHAT TO DO: restore the section or fix the parser; do not delete this assertion."
    );

    // The same anti-vacuity guard for Phase 116, written the same way and for the
    // same reason: the presence of the SECTION is the invariant, not the transient
    // `planned` status of its entries. Without this, deleting the whole Phase 116
    // section would make the permission above cover nothing and pass silently.
    let phase_116_records = records
        .iter()
        .filter(|record| PHASE_116_EQUATIONS.contains(&record.equation.as_str()))
        .count();
    assert!(
        phase_116_records >= 8,
        "FAILURE MODE: only {phase_116_records} Phase 116 bindings parsed (expected at least 8). \
         Either the parser is broken or the Phase 116 section was removed from \
         {BINDING_FILE}.\n\
         WHAT TO DO: restore the section or fix the parser. If Phase 116's bindings were \
         deliberately dropped, delete PHASE_116_EQUATIONS in the same edit so `planned` stops \
         being permitted on those equations."
    );
}

// ===========================================================================
// 3. The Phase 115 equations are actually bound
// ===========================================================================

#[test]
fn phase115_contract_bindings_the_three_phase_115_equations_are_bound() {
    let records = bindings();
    let mut wrong_crate = String::new();

    for (equation, minimum) in EXPECTED_PHASE_115_BINDINGS {
        let bound: Vec<&Binding> = records
            .iter()
            .filter(|record| record.equation == *equation)
            .collect();
        assert!(
            bound.len() >= *minimum,
            "FAILURE MODE: equation `{equation}` has {} binding(s) in {BINDING_FILE}, fewer than \
             the {minimum} Phase 115 writes. A truncated or mis-parsed file would pass every \
             other check in this suite over nothing.\n\
             WHAT TO DO: restore the missing bindings, or fix the parser.",
            bound.len()
        );
        for record in bound {
            if record.module_path.starts_with("pmcp::") {
                continue;
            }
            writeln!(
                wrong_crate,
                "  {BINDING_FILE}:{} equation `{equation}` function `{}` module_path `{}`",
                record.line, record.function, record.module_path
            )
            .expect("writing to a String cannot fail");
        }
    }

    assert!(
        wrong_crate.is_empty(),
        "FAILURE MODE: a Phase 115 binding names a module outside the `pmcp` crate. All three \
         equations describe core SDK behaviour and every function they bind lives in \
         `src/`.\n\n{wrong_crate}\n\
         WHAT TO DO: fix the `module_path:` to the real one."
    );
}

// ===========================================================================
// 4. No binding to an equation that does not exist
// ===========================================================================

#[test]
fn phase115_contract_bindings_every_bound_equation_exists_in_the_contract() {
    let records = bindings();
    let defined = contract_equations();
    let ledger: BTreeSet<&str> = LEGACY_UNCONTRACTED_EQUATIONS.iter().copied().collect();
    let bound: BTreeSet<&str> = records
        .iter()
        .map(|record| record.equation.as_str())
        .filter(|equation| !equation.is_empty())
        .collect();
    let mut orphans = String::new();

    for equation in &bound {
        if defined.contains(*equation) || ledger.contains(equation) {
            continue;
        }
        writeln!(orphans, "  `{equation}`").expect("writing to a String cannot fail");
    }

    assert!(
        orphans.is_empty(),
        "FAILURE MODE: a binding references an equation that {CONTRACT_FILE} does not define. The \
         binding claims a function implements an equation nobody wrote — the mirror image of a \
         ghost binding, and equally silent before this test existed.\n\n{orphans}\n\
         WHAT TO DO: add the equation to the contract's `equations:` map, or fix the `equation:` \
         value. Do NOT add it to LEGACY_UNCONTRACTED_EQUATIONS — that ledger is frozen at the 21 \
         pre-Phase-115 toolkit equations and may only shrink."
    );

    for equation in &ledger {
        assert!(
            bound.contains(equation),
            "FAILURE MODE: STALE LEDGER — LEGACY_UNCONTRACTED_EQUATIONS lists `{equation}`, but no \
             binding references it any more.\n\
             WHAT TO DO: delete that line from the ledger."
        );
        assert!(
            !defined.contains(*equation),
            "FAILURE MODE: STALE LEDGER — LEGACY_UNCONTRACTED_EQUATIONS lists `{equation}`, but \
             {CONTRACT_FILE} now defines it. That is progress.\n\
             WHAT TO DO: delete that line from the ledger so the real check covers it."
        );
    }
}

// ===========================================================================
// 5. The parse itself is not vacuous
// ===========================================================================

#[test]
fn phase115_contract_bindings_the_parse_is_not_vacuous() {
    let records = bindings();

    assert!(
        records.len() > MINIMUM_BINDINGS,
        "FAILURE MODE: parsed {} binding record(s) from {BINDING_FILE}, at or below the \
         {MINIMUM_BINDINGS} floor. A parser that silently reads nothing makes every other test in \
         this file pass over an empty set.\n\
         WHAT TO DO: fix the reader or restore the file; do not lower the floor.",
        records.len()
    );

    let implemented = records
        .iter()
        .filter(|record| record.status == "implemented")
        .count();
    assert!(
        implemented > 0,
        "FAILURE MODE: no binding parsed with `status: implemented`, so the ghost-binding check \
         examines nothing.\n\
         WHAT TO DO: fix the `status:` parse."
    );

    let mut incomplete = String::new();
    for record in &records {
        if record.function.is_empty() || record.module_path.is_empty() {
            writeln!(
                incomplete,
                "  {BINDING_FILE}:{} equation `{}` function `{}` module_path `{}`",
                record.line, record.equation, record.function, record.module_path
            )
            .expect("writing to a String cannot fail");
        }
    }
    assert!(
        incomplete.is_empty(),
        "FAILURE MODE: a binding record parsed without a `function:` or a `module_path:`. Either \
         the file carries an incomplete entry, or the reader dropped a field — both make the \
         resolution check skip real bindings.\n\n{incomplete}\n\
         WHAT TO DO: complete the entry in {BINDING_FILE}, or fix the reader."
    );
}
