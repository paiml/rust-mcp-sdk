//! Phase 114 source tripwires — the invariants a behavioural test cannot keep.
//!
//! # Why a source scanner rather than more behavioural tests
//!
//! This phase's correctness rests on two things a running server does not
//! observe:
//!
//! * **Gates.** Every `tasks/*` route is kept correct per era by a NAMED
//!   predicate. Deleting one changes no v1 behaviour at all — v1 is the
//!   permissive era — so every existing v1 test stays green while the v2 wire
//!   quietly reopens.
//! * **A name identity.** The five `TaskStatus` serde strings and the five
//!   strings in the vendored extension schema are the SAME set. Nothing in the
//!   type system says so.
//!
//! `tests/v2_prohibited_error_codes.rs` (plan 113-21) established the right
//! instrument for exactly this: a justified allowlist with a two-kind entry
//! model that fails on THREE distinct rot conditions — a new unlisted site, a
//! deleted guard, and a stale entry. 113-21 and 113-29 both record that the
//! instrument caught real sites the behavioural tests had missed. This file is
//! that instrument aimed at the tasks surface.
//!
//! # The scanner primitives are DELIBERATELY duplicated
//!
//! A Rust integration test is its own crate, so this file cannot import
//! `tests/v2_prohibited_error_codes.rs`'s scanner and that file cannot import
//! this one. The primitives below are therefore RESTATED rather than shared, and
//! the idiom is kept identical on purpose so the repository has one
//! source-scanning shape rather than three. Two things here are genuinely new
//! and are called out where they are defined:
//!
//! 1. [`strip_keeping_literals`] — a second stripping mode that removes comments
//!    but KEEPS string-literal contents, because two checks in this file are
//!    about wire strings (`"tasks/get"`, `"snake_case"`) and the ordinary
//!    stripper deletes exactly those.
//! 2. [`test_only_module_files`] — `cfg(test)` region exclusion is not enough
//!    here. `src/server/task_dispatch_tests.rs` carries no `#[cfg(test)]` marker
//!    of its own; the gate is on its `mod` DECLARATION in `src/server/mod.rs`.
//!    A numeric scan for `-32002` therefore hits five test-only files that the
//!    name-based scan in `v2_prohibited_error_codes.rs` never had to think
//!    about. Those files are discovered from their declarations, not from a
//!    filename convention.
//!
//! # What this file deliberately does NOT pin
//!
//! The `ext-tasks` extension is still `draft/` upstream. Phase 114's D-18 hold
//! says every *tasks* wire value is provisional until it publishes, and
//! `114-SPEC-RECHECK.md` § `## Recorded Exception` → *What this hold does NOT
//! permit* forbids treating a provisional value as authoritative. So:
//!
//! * **No tripwire over a tasks wire value** — not `ttlMs`, not
//!   `pollIntervalMs`, not `inputResponses`, not the four result shapes. The one
//!   apparent exception is the `TaskStatus` set, and it is not one: that check
//!   asserts an IDENTITY between two artifacts in this repository (the Rust
//!   source and the vendored schema), and it moves automatically when the schema
//!   is re-vendored at the D-18 gate. It never claims either side is final.
//! * **No tripwire over `resultType`'s value set.** The published core declares
//!   `ResultType = "complete" | "input_required" | string` while pmcp mints
//!   `"task"` through the open `| string` tail. That discrepancy is an OPEN
//!   QUESTION booked to plan 114-18. Pinning it here would freeze a decision
//!   nobody has made.
//!
//! What IS pinned from the published core: `-32021` and `-32602`, which the MCP
//! `2026-07-28` **core** schema published on 2026-07-29 and which this phase
//! reuses; and the absence of `-32002` from that schema.
//!
//! # When a check here fails
//!
//! Restore the guard, or move the allowlist entry and write down why. Deleting
//! the check, or widening it until it passes, is the failure mode it exists to
//! prevent.
#![cfg(not(target_arch = "wasm32"))]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use pmcp::types::protocol::error_codes;

/// A justification shorter than this is a label, not a decision.
const MIN_JUSTIFICATION_CHARS: usize = 40;

/// The dispatch file every `tasks/*` route lives in.
const DISPATCH: &str = "src/server/task_dispatch.rs";

/// The module that owns the three `tasks/*` method spellings the routing table
/// references.
const MRTR: &str = "src/types/mrtr.rs";

/// The wire types module that declares `TaskStatus`.
const TASK_TYPES: &str = "src/types/tasks.rs";

/// The vendored extension schema, read at RUNTIME.
///
/// Deliberately `read_to_string` rather than `include_str!`: the plan's own text
/// says "read the vendored file at runtime with `include_str!`", and those two
/// halves contradict each other — `include_str!` bakes the bytes in at COMPILE
/// time. Reading it at runtime is what makes a re-vendoring at the D-18 gate
/// move this test without anyone remembering to touch it.
const VENDORED_SCHEMA: &str = "schema/vendored/ext-tasks/schema.ts";

// ===========================================================================
// 1. Scanner primitives — restated, not shared. See the module docs.
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

fn read(path: &str) -> String {
    let full = repo_root().join(path);
    fs::read_to_string(&full).unwrap_or_else(|e| panic!("cannot read {}: {e}", full.display()))
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Every `.rs` file under `src/`, discovered at RUNTIME with `read_dir` so a NEW
/// file cannot escape the scan by nobody remembering to add it.
fn src_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rs_files(&repo_root().join("src"), &mut files);
    files.sort();
    assert!(
        files.len() > 50,
        "src/ carries well over fifty files; discovering {} means the walk is broken and every \
         check in this file would pass vacuously",
        files.len()
    );
    files
}

// --- stripping (comments removed; literals removed or kept, line map preserved) ---

/// Source with whitespace collapsed and comments removed.
///
/// `lines[i]` is the 1-based source line of `text`'s i-th byte.
#[derive(Default)]
struct Stripped {
    text: String,
    lines: Vec<u32>,
}

impl Stripped {
    fn push_char(&mut self, ch: char, line: u32) {
        self.text.push(ch);
        for _ in 0..ch.len_utf8() {
            self.lines.push(line);
        }
    }

    fn push_delims(&mut self, delims: &str, line: u32) {
        for ch in delims.chars() {
            self.push_char(ch, line);
        }
    }
}

fn line_of(stripped: &Stripped, index: usize) -> u32 {
    stripped.lines.get(index).copied().unwrap_or(0)
}

struct Construct {
    end: usize,
    delims: &'static str,
}

fn is_ident_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn line_numbers(chars: &[char]) -> Vec<u32> {
    let mut lines = Vec::with_capacity(chars.len());
    let mut line: u32 = 1;
    for &ch in chars {
        lines.push(line);
        if ch == '\n' {
            line += 1;
        }
    }
    lines
}

fn end_of_line(chars: &[char], from: usize) -> usize {
    let mut j = from;
    while j < chars.len() && chars[j] != '\n' {
        j += 1;
    }
    j
}

/// End of a block comment, honouring Rust's comment nesting.
fn end_of_block_comment(chars: &[char], from: usize) -> usize {
    let mut depth: usize = 0;
    let mut j = from;
    while j < chars.len() {
        if chars[j] == '/' && chars.get(j + 1) == Some(&'*') {
            depth += 1;
            j += 2;
        } else if chars[j] == '*' && chars.get(j + 1) == Some(&'/') {
            depth -= 1;
            j += 2;
            if depth == 0 {
                return j;
            }
        } else {
            j += 1;
        }
    }
    chars.len()
}

fn end_of_string(chars: &[char], from: usize) -> usize {
    let mut j = from + 1;
    while j < chars.len() {
        match chars[j] {
            '\\' => j += 2,
            '"' => return j + 1,
            _ => j += 1,
        }
    }
    chars.len()
}

/// End of an `r"..."` / `r#"..."#` raw string starting at `from`.
fn raw_string_end(chars: &[char], from: usize) -> Option<usize> {
    let mut hashes: usize = 0;
    let mut j = from + 1;
    while chars.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    if chars.get(j) != Some(&'"') {
        return None;
    }
    j += 1;
    while j < chars.len() {
        if chars[j] == '"' && (1..=hashes).all(|k| chars.get(j + k) == Some(&'#')) {
            return Some(j + 1 + hashes);
        }
        j += 1;
    }
    Some(chars.len())
}

/// End of a char literal, or `None` when the tick opens a LIFETIME.
fn end_of_char_literal(chars: &[char], from: usize) -> Option<usize> {
    let c1 = *chars.get(from + 1)?;
    if c1 == '\\' {
        let mut j = from + 3;
        while j < chars.len() && chars[j] != '\'' {
            j += 1;
        }
        return Some((j + 1).min(chars.len()));
    }
    if chars.get(from + 2) == Some(&'\'') {
        return Some(from + 3);
    }
    None
}

fn skip_construct(chars: &[char], i: usize, prev_ident: bool) -> Option<Construct> {
    let next = chars.get(i + 1).copied();
    match chars[i] {
        '/' if next == Some('/') => Some(Construct {
            end: end_of_line(chars, i),
            delims: "",
        }),
        '/' if next == Some('*') => Some(Construct {
            end: end_of_block_comment(chars, i),
            delims: "",
        }),
        '"' => Some(Construct {
            end: end_of_string(chars, i),
            delims: "\"\"",
        }),
        '\'' => end_of_char_literal(chars, i).map(|end| Construct { end, delims: "''" }),
        'r' if !prev_ident => raw_string_end(chars, i).map(|end| Construct {
            end,
            delims: "\"\"",
        }),
        'b' if !prev_ident && next == Some('r') => {
            raw_string_end(chars, i + 1).map(|end| Construct {
                end,
                delims: "\"\"",
            })
        },
        _ => None,
    }
}

/// Strip `source` to scannable text plus a byte-to-line map.
///
/// Comments always vanish. String and char literal CONTENTS vanish too unless
/// `keep_literals`, in which case the literal is copied through verbatim.
///
/// # Whitespace collapses to a single space rather than vanishing
///
/// `tests/v2_bounded_reads_tripwire.rs` removes whitespace entirely, because its
/// needles are method chains that rustfmt breaks across lines. This scanner
/// matches IDENTIFIERS, which need word boundaries: removing whitespace turns
/// `pub const V1_TASK_PENDING` into `pubconstV1_TASK_PENDING`, whose preceding
/// character is an identifier character, so the whole-token filter would reject
/// the DEFINITION site and silently lose coverage of the file being scanned.
/// That was measured in `v2_prohibited_error_codes.rs`, not predicted.
fn strip_with(source: &str, keep_literals: bool) -> Stripped {
    let chars: Vec<char> = source.chars().collect();
    let lines = line_numbers(&chars);
    let mut out = Stripped::default();
    let mut i: usize = 0;
    let mut prev_ident = false;
    let mut pending_space = false;
    while i < chars.len() {
        if let Some(construct) = skip_construct(&chars, i, prev_ident) {
            if pending_space {
                out.push_char(' ', lines[i]);
                pending_space = false;
            }
            if keep_literals && !construct.delims.is_empty() {
                for (j, ch) in chars.iter().enumerate().take(construct.end).skip(i) {
                    out.push_char(*ch, lines[j]);
                }
            } else {
                out.push_delims(construct.delims, lines[i]);
            }
            i = construct.end.max(i + 1);
            prev_ident = false;
            continue;
        }
        let ch = chars[i];
        if ch.is_whitespace() {
            prev_ident = false;
            pending_space = true;
        } else {
            if pending_space {
                out.push_char(' ', lines[i]);
                pending_space = false;
            }
            out.push_char(ch, lines[i]);
            prev_ident = is_ident_char(ch);
        }
        i += 1;
    }
    out
}

/// Comments and literal CONTENTS removed — the mode for identifier scans.
fn strip(source: &str) -> Stripped {
    strip_with(source, false)
}

/// Comments removed, literal contents KEPT — the mode for wire-string scans.
///
/// Three checks in this file are about strings that live inside literals: the
/// `tasks/*` method spellings, `#[serde(rename_all = "snake_case")]`, and the
/// `#[path = "…"]` on a `cfg(test)` module declaration. [`strip`] deletes
/// exactly those, so a second mode is required rather than optional.
fn strip_keeping_literals(source: &str) -> Stripped {
    strip_with(source, true)
}

// --- `cfg(test)` region exclusion ---

fn balanced_end(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let (opener, closer) = match bytes.get(open)? {
        b'(' => (b'(', b')'),
        b'[' => (b'[', b']'),
        b'{' => (b'{', b'}'),
        _ => return None,
    };
    let mut depth: usize = 0;
    for (offset, byte) in bytes.iter().enumerate().skip(open) {
        if *byte == opener {
            depth += 1;
        } else if *byte == closer {
            depth -= 1;
            if depth == 0 {
                return Some(offset);
            }
        }
    }
    None
}

fn split_top_level(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut start: usize = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(inner[start..idx].trim());
                start = idx + 1;
            },
            _ => {},
        }
    }
    parts.push(inner[start..].trim());
    parts
}

/// Whether a `cfg` predicate can only hold when `test` is enabled.
fn cfg_requires_test(predicate: &str) -> bool {
    let predicate = predicate.trim();
    if predicate == "test" {
        return true;
    }
    let Some(inner) = predicate
        .strip_prefix("all(")
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        return false;
    };
    split_top_level(inner).into_iter().any(cfg_requires_test)
}

fn item_span(text: &str, from: usize) -> Option<Range<usize>> {
    let bytes = text.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' => i = balanced_end(text, i)? + 1,
            b';' | b',' => return Some(from..i + 1),
            b'{' => return balanced_end(text, i).map(|end| from..end + 1),
            _ => i += 1,
        }
    }
    None
}

/// Every region of `stripped` that only compiles under `cfg(test)`.
///
/// Per-item brace matching, NOT truncation at the first marker: truncating would
/// drop thousands of production lines from `streamable_http_server.rs`, which
/// `tests/v2_bounded_reads_tripwire.rs` measured.
fn cfg_test_spans(stripped: &Stripped) -> Vec<Range<usize>> {
    let text = &stripped.text;
    let mut spans = Vec::new();
    let mut search: usize = 0;
    while let Some(found) = text[search..].find("#[cfg(") {
        let paren = search + found + "#[cfg".len();
        let Some(close) = balanced_end(text, paren) else {
            break;
        };
        let predicate = &text[paren + 1..close];
        search = close + 1;
        if !cfg_requires_test(predicate) {
            continue;
        }
        if let Some(span) = item_span(text, search) {
            search = span.end.max(search);
            spans.push(span);
        }
    }
    spans
}

fn is_excluded(spans: &[Range<usize>], index: usize) -> bool {
    spans.iter().any(|span| span.contains(&index))
}

fn occurrences(text: &str, needle: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut from: usize = 0;
    while let Some(found) = text[from..].find(needle) {
        let at = from + found;
        out.push(at);
        from = at + 1;
    }
    out
}

/// A whole-token match: `INTERNAL_ERROR` must not match `INTERNAL_ERROR_CODES`.
fn token_hits(text: &str, needle: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    occurrences(text, needle)
        .into_iter()
        .filter(|at| {
            let before_ok = *at == 0 || !is_ident_char(char::from(bytes[at - 1]));
            let after = at + needle.len();
            let after_ok = after >= bytes.len() || !is_ident_char(char::from(bytes[after]));
            before_ok && after_ok
        })
        .collect()
}

/// Is `needle` present in `text`?
///
/// Whole-token when the needle is a bare identifier, plain substring otherwise
/// (`trigger.fired()` is not an identifier and has no boundary question).
fn needle_present(text: &str, needle: &str) -> bool {
    if needle.chars().all(is_ident_char) {
        !token_hits(text, needle).is_empty()
    } else {
        text.contains(needle)
    }
}

// ===========================================================================
// 2. Test-only module files — the exclusion `cfg(test)` regions cannot make.
// ===========================================================================

/// Every `src/` file whose whole content only compiles under `test`, discovered
/// from its `mod` DECLARATION rather than from a filename convention.
///
/// `src/server/task_dispatch_tests.rs` carries no `#[cfg(test)]` marker inside
/// it; the gate is `#[cfg(test)] mod task_dispatch_tests;` in
/// `src/server/mod.rs`. A per-file `cfg(test)`-region scan therefore treats the
/// whole file as shipped source, which is harmless for a scan over SYMBOLS the
/// test files do not name and wrong for a scan over the NUMBER `-32002`, which
/// three of them assert on.
///
/// `#[path = "…"]` is honoured, because `src/server/wasm_core.rs` uses it.
fn test_only_module_files() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for path in src_files() {
        let source = fs::read_to_string(&path).expect("readable source");
        let stripped = strip_keeping_literals(&source);
        let text = &stripped.text;
        let dir = path.parent().expect("a file has a parent").to_path_buf();
        let mut search: usize = 0;
        while let Some(found) = text[search..].find("#[cfg(") {
            let paren = search + found + "#[cfg".len();
            let Some(close) = balanced_end(text, paren) else {
                break;
            };
            let predicate = &text[paren + 1..close];
            search = close + 1;
            if !cfg_requires_test(predicate) {
                continue;
            }
            let Some(span) = item_span(text, search) else {
                continue;
            };
            search = span.end.max(search);
            if let Some(file) = declared_module_file(&text[span.clone()]) {
                out.insert(rel(&dir.join(file)));
            }
        }
    }
    out
}

/// The file a `mod <name>;` item declares, honouring `#[path = "…"]`.
///
/// `None` for a `mod name { … }` with an inline body — that body is already
/// covered by the ordinary `cfg(test)` region exclusion.
fn declared_module_file(item: &str) -> Option<String> {
    if item.contains('{') {
        return None;
    }
    let at = token_hits(item, "mod").first().copied()?;
    let rest = item[at + "mod".len()..].trim_start();
    let name: String = rest.chars().take_while(|c| is_ident_char(*c)).collect();
    if name.is_empty() {
        return None;
    }
    if let Some(path_at) = item.find("#[path") {
        let quoted = &item[path_at..];
        let open = quoted.find('"')?;
        let close = quoted[open + 1..].find('"')? + open + 1;
        return Some(quoted[open + 1..close].to_string());
    }
    Some(format!("{name}.rs"))
}

/// Every `src/` file that ships, i.e. is not a `cfg(test)`-declared module file.
fn shipped_files() -> Vec<PathBuf> {
    let test_only = test_only_module_files();
    src_files()
        .into_iter()
        .filter(|path| !test_only.contains(&rel(path)))
        .collect()
}

// ===========================================================================
// 3. Function spans — so a guard is checked WHERE it must live.
// ===========================================================================

/// The body span of every named `fn` in `stripped`, innermost-resolvable.
///
/// A declaration with no body (`fn f(&self) -> bool;` in a trait) contributes
/// nothing, so the next function's body is never mistaken for it.
fn fn_spans(stripped: &Stripped) -> Vec<(String, Range<usize>)> {
    let text = &stripped.text;
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    for at in token_hits(text, "fn") {
        let rest = text[at + 2..].trim_start();
        let name: String = rest.chars().take_while(|c| is_ident_char(*c)).collect();
        if name.is_empty() {
            continue;
        }
        let after_name = at + 2 + (text.len() - at - 2 - rest.len()) + name.len();
        let Some(paren) = text[after_name..].find('(').map(|o| o + after_name) else {
            continue;
        };
        let Some(params_end) = balanced_end(text, paren) else {
            continue;
        };
        let mut i = params_end + 1;
        while i < bytes.len() && bytes[i] != b'{' && bytes[i] != b';' {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b';' {
            continue;
        }
        if let Some(end) = balanced_end(text, i) {
            spans.push((name, i..end + 1));
        }
    }
    spans
}

/// The body of `name` in `stripped`, or `None` when there is no such function.
fn fn_body<'a>(stripped: &'a Stripped, name: &str) -> Option<&'a str> {
    fn_spans(stripped)
        .into_iter()
        .find(|(found, _)| found == name)
        .map(|(_, span)| &stripped.text[span])
}

/// The INNERMOST function whose body contains `index`.
fn enclosing_fn(spans: &[(String, Range<usize>)], index: usize) -> Option<&str> {
    spans
        .iter()
        .filter(|(_, span)| span.contains(&index))
        .min_by_key(|(_, span)| span.end - span.start)
        .map(|(name, _)| name.as_str())
}

/// The balanced span of `header` (e.g. `impl std::fmt::Display for TaskStatus`).
fn block_after<'a>(stripped: &'a Stripped, header: &str) -> Option<&'a str> {
    let text = &stripped.text;
    let at = text.find(header)?;
    let brace = text[at..].find('{')? + at;
    let end = balanced_end(text, brace)?;
    Some(&text[brace..=end])
}

// ===========================================================================
// 4. TASK 1 — every `tasks/*` route carries a NAMED era guard.
// ===========================================================================

/// A predicate that must appear in a NAMED function's body.
///
/// The site matters. `route_tasks_list`'s retirement gate does not live in
/// `route_tasks_list` at all — it fires in `retired_method`, one frame up — and
/// a check that merely asked "does this FILE still contain the token" would stay
/// green after the call site was deleted. Naming the site is what makes a
/// deleted guard fail the ONE route it belonged to.
struct GuardRef {
    needle: &'static str,
    site: &'static str,
}

/// The wire method a route answers, when it answers one.
enum RouteMethod {
    /// A `tasks/*` request method. Every one of these must be discovered by the
    /// wire-name scan and vice versa.
    Wire(&'static str),
    /// A route with no `tasks/*` method of its own: the endpoint dispatcher and
    /// the `tools/call` create gate. Excluded from the wire-name identity.
    NotAWireMethod,
}

struct RouteEntry {
    /// The function, as spelled in [`DISPATCH`].
    function: &'static str,
    method: RouteMethod,
    guards: &'static [GuardRef],
    why: &'static str,
}

/// Every `tasks/*` route in [`DISPATCH`] and the predicate(s) that keep it
/// correct per era.
///
/// Three rot conditions fail here, and each has its own recorded negative
/// control:
///
/// 1. a route function present in the source but ABSENT from this list — a new
///    route that forgot its guard;
/// 2. an allowlisted route whose named guard no longer appears in its named
///    site — the guard was deleted;
/// 3. an entry for a function that no longer exists — a stale entry, which is
///    how a real new route hides under a name set for a route since removed.
const ROUTES: &[RouteEntry] = &[
    RouteEntry {
        function: "route_tasks_endpoint",
        method: RouteMethod::NotAWireMethod,
        guards: &[
            GuardRef {
                needle: "retired_method",
                site: "route_tasks_endpoint",
            },
            GuardRef {
                needle: "declares_tasks_extension",
                site: "route_tasks_endpoint",
            },
            GuardRef {
                needle: "resolve_owner",
                site: "route_tasks_endpoint",
            },
        ],
        why:
            "The ordered refusal chain every ClientRequest-shaped tasks method enters: retirement \
              first, then the declaration gate, then the identity table, and only then the params. \
              The ORDER is the contract — a retired method answered `authenticate yourself` would \
              imply that authenticating would enumerate something (T-114-32).",
    },
    RouteEntry {
        function: "route_tasks_get",
        method: RouteMethod::Wire("tasks/get"),
        guards: &[
            GuardRef {
                needle: "is_v1_task_era",
                site: "route_tasks_get",
            },
            GuardRef {
                needle: "store_error_response",
                site: "route_tasks_get",
            },
        ],
        why: "Survives on BOTH eras, so its guard is not a retirement: the era decides the SHAPE \
              (nested GetTaskResult on v1, the flat DetailedTask on v2) and the era-aware store \
              mapping decides the not-found CODE. Losing either turns a v2 answer into a v1 one \
              without any v1 test noticing.",
    },
    RouteEntry {
        function: "route_tasks_cancel",
        method: RouteMethod::Wire("tasks/cancel"),
        guards: &[
            GuardRef {
                needle: "is_v1_task_era",
                site: "route_tasks_cancel",
            },
            GuardRef {
                needle: "store_error_response",
                site: "route_tasks_cancel",
            },
        ],
        why: "Also survives both eras. v1 returns the nested task body; v2 returns an EMPTY \
              acknowledgement because CancelTaskResult = Result in the extension schema. Deleting \
              the era read would put a task body back on a v2 cancel ack, which pmcp's own v2 \
              client tolerates silently.",
    },
    RouteEntry {
        function: "route_tasks_list",
        method: RouteMethod::Wire("tasks/list"),
        guards: &[GuardRef {
            needle: "tasks_list_serves_on_era",
            site: "retired_method",
        }],
        why: "RETIRED on 2026-07-28. Its guard deliberately lives in `retired_method`, one frame \
              above the route, so the refusal happens before any owner binding, any store `list` \
              and any router call — which is what makes enumeration impossible rather than merely \
              refused. TASK-03 and TASK-05 are that one removal seen from two angles.",
    },
    RouteEntry {
        function: "handle_tasks_result",
        method: RouteMethod::Wire("tasks/result"),
        guards: &[
            GuardRef {
                needle: "tasks_result_serves_on_era",
                site: "retired_method",
            },
            GuardRef {
                needle: "tasks_result_serves_on_era",
                site: "handle_tasks_result",
            },
        ],
        why: "RETIRED on 2026-07-28, and the ONLY route with its guard named twice on purpose: \
              the in-body read is what lets a negative control that disables the predicate open \
              the WHOLE gate, which a second independently-keyed era test would have masked. It \
              is also the last v2-reachable path to the prohibited -32002.",
    },
    RouteEntry {
        function: "route_tasks_update",
        method: RouteMethod::Wire("tasks/update"),
        guards: &[
            GuardRef {
                needle: "is_v1_task_era",
                site: "route_tasks_update",
            },
            GuardRef {
                needle: "declares_tasks_extension",
                site: "route_tasks_update",
            },
            GuardRef {
                needle: "resolve_owner",
                site: "route_tasks_update",
            },
        ],
        why: "A 2026-07-28-ONLY method with no ClientRequest variant, so it cannot enter the \
              shared chain and restates it: era, backend, declaration, identity, params. Its era \
              guard runs in the OPPOSITE direction from the retirements — v1 gets `not a method \
              yet`, not `retired` — and those two call for opposite fixes.",
    },
    RouteEntry {
        function: "create_gate",
        method: RouteMethod::NotAWireMethod,
        guards: &[GuardRef {
            needle: "trigger.fired()",
            site: "create_gate",
        }],
        why:
            "The ONE expression deciding whether a tools/call becomes a task. Era-awareness lives \
              in CreateTrigger: v1 fires on the `task` field, v2 on the client's declaration and \
              IGNORES the field. Reading the field on v2 would hand a task handle to a client with \
              no rule for reading one, which the extension forbids.",
    },
    RouteEntry {
        function: "maybe_build_task_created",
        method: RouteMethod::NotAWireMethod,
        guards: &[GuardRef {
            needle: "self.create_gate(",
            site: "maybe_build_task_created",
        }],
        why: "The Server dispatcher's only entry to the create gate. It is listed separately from \
              create_gate because a caller that inlined a partial copy of the gate would leave \
              create_gate present and unused, and the whole point of T-114-58 is that the complete \
              rule is enforced INSIDE one function that no caller pre-checks.",
    },
];

/// Route functions discovered from the source rather than from this list.
///
/// The verb family is deliberately wider than the four `route_tasks_*` names
/// that exist today, so `serve_tasks_pause` or `dispatch_task_resume` is caught
/// as well. The complementary discovery axis —
/// [`every_v2_tasks_wire_method_maps_to_an_allowlisted_route`] — finds a new
/// route by its WIRE NAME instead, so a route with a name outside every verb
/// family still fails as long as it answers a `tasks/*` method.
fn discovered_routes(stripped: &Stripped) -> BTreeSet<String> {
    const VERBS: [&str; 4] = ["route_", "handle_", "serve_", "dispatch_"];
    fn_spans(stripped)
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| {
            VERBS.iter().any(|verb| {
                name.strip_prefix(verb)
                    .is_some_and(|rest| rest.starts_with("tasks_") || rest.starts_with("task_"))
            })
        })
        .collect()
}

/// ROT CONDITION 1 — a route function the allowlist has never heard of.
///
/// # Why the three rot conditions are three TESTS
///
/// They began as one, which is the shape
/// `tests/v2_prohibited_error_codes.rs` uses. The masking check then FIRED:
/// three separate negative controls — a deleted guard, a new unlisted route and
/// a stale entry — produced the IDENTICAL failing set at the IDENTICAL
/// `assert!` line, separable only by reading the collected message text. That is
/// the weaker separation 114-19 had to force its own split out of, so this one
/// is split up front: each condition now fails a test nobody has to read a
/// string to identify.
#[test]
fn every_tasks_route_in_the_source_is_allowlisted() {
    let source = read(DISPATCH);
    let stripped = strip(&source);
    let discovered = discovered_routes(&stripped);
    let listed: BTreeSet<String> = ROUTES.iter().map(|e| e.function.to_string()).collect();
    let unlisted: Vec<&String> = discovered.difference(&listed).collect();

    assert!(
        unlisted.is_empty(),
        "UNLISTED route(s) in {DISPATCH}: {unlisted:?}\n  Every tasks route must name the \
         predicate that keeps it correct per era. Add a ROUTES entry naming that predicate and \
         the function it runs in, or the era gate can be deleted without failing anything."
    );
}

/// ROT CONDITION 3 — an entry for a route that no longer exists.
///
/// A stale entry is how a real new route hides under a name set for a route
/// since removed: the population check above compares against a list that still
/// mentions the dead name, so a rename reads as "no change".
#[test]
fn every_allowlisted_route_still_exists() {
    let source = read(DISPATCH);
    let stripped = strip(&source);
    let stale: Vec<&str> = ROUTES
        .iter()
        .filter(|entry| fn_body(&stripped, entry.function).is_none())
        .map(|entry| entry.function)
        .collect();

    assert!(
        stale.is_empty(),
        "STALE ROUTES entr(ies): {stale:?} — {DISPATCH} no longer declares them.\n  Delete the \
         entry, or point it at the function the route was renamed to."
    );
}

/// ROT CONDITION 2 — a named guard that no longer runs where it must.
///
/// The SITE is the point. `route_tasks_list`'s retirement gate fires in
/// `retired_method`, one frame above the route, and `is_v1_task_era` appears in
/// eight places in this file — so a check that only asked "does the FILE still
/// contain the token" would stay green after any single call site was deleted.
#[test]
fn every_allowlisted_routes_era_guard_is_present_in_its_named_site() {
    let source = read(DISPATCH);
    let stripped = strip(&source);
    let mut failures = String::new();

    for entry in ROUTES {
        if fn_body(&stripped, entry.function).is_none() {
            // Owned by `every_allowlisted_route_still_exists`; skipping here
            // keeps that control's failing set disjoint from this one's.
            continue;
        }
        for guard in entry.guards {
            let Some(body) = fn_body(&stripped, guard.site) else {
                let _ = writeln!(
                    failures,
                    "\n  MISSING guard SITE: route `{}` names `{}` as living in `{}`, which \
                     {DISPATCH} no longer declares.",
                    entry.function, guard.needle, guard.site
                );
                continue;
            };
            if !needle_present(body, guard.needle) {
                let _ = writeln!(
                    failures,
                    "\n  MISSING era guard: route `{}` is kept correct by `{}` inside `{}`, and \
                     that expression is gone.\n    Without it this route answers a v2 request the \
                     way it answers a v1 one, and no v1 test can see the difference.",
                    entry.function, guard.needle, guard.site
                );
            }
        }
    }

    assert!(
        failures.is_empty(),
        "a tasks route lost its guard:{failures}"
    );
}

/// ANTI-VACUITY — a scanner that matches nothing is not a guard.
///
/// 113-19 records the failure this prevents: `cargo public-api` omits
/// `doc(hidden)` items, so an absence check was green BEFORE the fix as well as
/// after it. A route scan that discovered zero routes would pass every check
/// above while measuring nothing at all.
#[test]
fn the_route_scan_is_not_vacuous_and_every_entry_is_justified() {
    let source = read(DISPATCH);
    let stripped = strip(&source);
    let discovered = discovered_routes(&stripped);

    assert!(
        !discovered.is_empty(),
        "the route scan discovered ZERO route functions in {DISPATCH}. Either the file moved or \
         the scanner is broken; in both cases every check above passes over an empty set."
    );
    assert!(
        discovered.len() >= 4,
        "the route scan discovered only {} route functions in {DISPATCH}; wave 9 left at least \
         four (get / list / cancel / update). Observed: {discovered:?}",
        discovered.len()
    );
    assert!(
        !fn_spans(&stripped).is_empty(),
        "function-span discovery found no functions at all in {DISPATCH}"
    );

    assert_justifications(
        "ROUTES",
        &ROUTES
            .iter()
            .map(|entry| (entry.function, entry.why))
            .collect::<Vec<_>>(),
    );
    assert!(
        ROUTES
            .iter()
            .any(|e| matches!(e.method, RouteMethod::Wire(_))),
        "no ROUTES entry names a wire method, so the wire-name identity below compares two empty \
         sets"
    );
}

/// Every `tasks/*` method NAME the dispatch surface spells maps to exactly one
/// allowlisted route, and vice versa.
///
/// The second discovery axis. [`discovered_routes`] finds a new route by its
/// FUNCTION name; this finds one by its WIRE name, which is the axis a route
/// called `frobnicate` would still trip. The scan reads
/// [`strip_keeping_literals`] output because the method names live inside string
/// literals that [`strip`] deletes.
#[test]
fn every_v2_tasks_wire_method_maps_to_an_allowlisted_route() {
    let mut discovered = BTreeSet::new();
    for path in [DISPATCH, MRTR] {
        let source = read(path);
        let stripped = strip_keeping_literals(&source);
        let excluded = cfg_test_spans(&stripped);
        for at in occurrences(&stripped.text, "\"tasks/") {
            if is_excluded(&excluded, at) {
                continue;
            }
            let rest = &stripped.text[at + "\"tasks/".len()..];
            let name: String = rest.chars().take_while(|c| is_ident_char(*c)).collect();
            if !name.is_empty() {
                discovered.insert(format!("tasks/{name}"));
            }
        }
    }
    let listed: BTreeSet<String> = ROUTES
        .iter()
        .filter_map(|entry| match entry.method {
            RouteMethod::Wire(method) => Some(method.to_string()),
            RouteMethod::NotAWireMethod => None,
        })
        .collect();

    assert!(
        !discovered.is_empty(),
        "no `tasks/*` method literal was found in {DISPATCH} or {MRTR}; the wire-name scan is \
         measuring nothing"
    );
    assert_eq!(
        discovered, listed,
        "the `tasks/*` wire-method population changed.\n  A method spelled in {DISPATCH} or \
         {MRTR} with no ROUTES entry is a route whose era guard nobody named; a ROUTES entry with \
         no spelling left is a stale entry."
    );
}

// ===========================================================================
// 5. TASK 2(a) — no v2-reachable `NotFound` becomes `-32603`.
// ===========================================================================

/// Why an `INTERNAL_ERROR` in the tasks dispatch is not a v2 `NotFound`.
enum Disposition {
    /// EVERY hit in this function is unreachable on v2, and `guard` in
    /// `guard_site` is what keeps it that way.
    V1Only {
        guard: &'static str,
        guard_site: &'static str,
    },
    /// The function IS v2-reachable, but a store `NotFound` never reaches these
    /// hits — `why` names the arm that takes it instead.
    NotFoundRoutedElsewhere {
        guard: &'static str,
        guard_site: &'static str,
    },
    /// An out-of-tree [`TaskRouter`] error, which is not a `TaskStoreError` at
    /// all and carries no not-found discriminant this crate can read.
    ///
    /// **RECORDED GAP, not a clean bill of health.** See the entries' `why`.
    RouterLeg,
}

struct InternalErrorEntry {
    function: &'static str,
    hits: usize,
    disposition: Disposition,
    why: &'static str,
}

/// Every `INTERNAL_ERROR` (`-32603`) emission in [`DISPATCH`], by enclosing
/// function, with the MEASURED hit count.
///
/// The count is part of the entry on purpose: a second `INTERNAL_ERROR` added
/// inside an already-allowlisted function is exactly the shape a regression
/// takes, and a file-level or function-level presence check cannot see it.
///
/// The rule this defends: the extension makes `-32602` a MUST for a `tasks/get`
/// naming an invalid or nonexistent `taskId`, and pmcp deliberately surfaces an
/// OWNER MISMATCH as `NotFound` so that wrong-owner and absent are
/// indistinguishable. A `-32603` on that path would therefore read a different
/// failure CLASS to a conformant client for the one case the anti-oracle design
/// most cares about — and Phase 118's conformance run grades exactly this.
const INTERNAL_ERROR_SITES: &[InternalErrorEntry] = &[
    InternalErrorEntry {
        function: "store_error_response",
        hits: 2,
        disposition: Disposition::NotFoundRoutedElsewhere {
            guard: "is_v1_task_era",
            guard_site: "store_error_response",
        },
        why: "The era-aware mapping itself. Hit 1 is the v1 blanket arm, where EVERY store error \
              is -32603 byte-for-byte as it always was. Hit 2 is the v2 arm for InvalidTransition \
              and Internal, neither of which is a not-found. NotFound and Expired are folded onto \
              one -32602 above it, which the ordering assertion in this test pins separately.",
    },
    InternalErrorEntry {
        function: "handle_tasks_result",
        hits: 2,
        disposition: Disposition::V1Only {
            guard: "tasks_result_serves_on_era",
            guard_site: "retired_method",
        },
        why: "tasks/result is retired on v2, so both hits are unreachable there. They are also \
              not-found-free on v1: the store leg matches TaskStoreError::NotFound explicitly and \
              FALLS THROUGH to the router rather than erroring, and the second hit renders a \
              router error, which is not a TaskStoreError.",
    },
    InternalErrorEntry {
        function: "route_tasks_list",
        hits: 2,
        disposition: Disposition::V1Only {
            guard: "tasks_list_serves_on_era",
            guard_site: "retired_method",
        },
        why: "tasks/list is retired on v2 and its gate fires before this function is entered at \
              all, so neither the store `list` error nor the router error can reach a v2 caller. \
              Enumeration has no v2 surface, which is why this route needs no v2 code mapping of \
              its own rather than having one that happens to be unused.",
    },
    InternalErrorEntry {
        function: "route_tasks_get",
        hits: 1,
        disposition: Disposition::RouterLeg,
        why: "The TaskRouter fall-through. RECORDED GAP (D-114-P): a router-backed v2 server \
              answers -32603 for a task its router cannot find, where the extension requires \
              -32602. The dispatch cannot fix it unilaterally — TaskRouter returns pmcp::Error \
              with no not-found discriminant — so closing it means widening a legacy trait, which \
              a coverage-only plan may not do. STORE-backed servers, which is every backend in \
              this repository, take store_error_response and are correct.",
    },
    InternalErrorEntry {
        function: "route_tasks_cancel",
        hits: 1,
        disposition: Disposition::RouterLeg,
        why: "The same TaskRouter fall-through as tasks/get, and the same recorded gap, with one \
              mitigating difference the extension itself supplies: -32602 is a SHOULD for \
              tasks/cancel rather than the MUST it is for tasks/get, so a router-backed \
              deployment is non-ideal here and non-conformant there.",
    },
    InternalErrorEntry {
        function: "deliver_tasks_update",
        hits: 1,
        disposition: Disposition::RouterLeg,
        why: "The tasks/update router leg. Its store leg cannot reach this hit: \
              store_error_or_fall_through returns None for a NotFound while a router is \
              configured, and routes every other store error through the era-aware mapping. \
              -32602 is a SHOULD for tasks/update, as for cancel.",
    },
];

/// Every `INTERNAL_ERROR` hit in [`DISPATCH`], grouped by enclosing function.
fn internal_error_hits() -> BTreeMap<String, Vec<u32>> {
    let source = read(DISPATCH);
    let stripped = strip(&source);
    let spans = fn_spans(&stripped);
    let excluded = cfg_test_spans(&stripped);
    let mut out: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for at in token_hits(&stripped.text, "INTERNAL_ERROR") {
        if is_excluded(&excluded, at) {
            continue;
        }
        let owner = enclosing_fn(&spans, at)
            .unwrap_or("<file scope>")
            .to_string();
        out.entry(owner).or_default().push(line_of(&stripped, at));
    }
    out
}

/// The `-32603` population in the tasks dispatch equals the allowlist, function
/// by function and COUNT by count.
#[test]
fn the_minus_32603_population_in_the_tasks_dispatch_is_enumerated() {
    let observed = internal_error_hits();
    let mut failures = String::new();

    assert!(
        !observed.is_empty(),
        "no INTERNAL_ERROR emission was found in {DISPATCH}; the scan is vacuous"
    );

    for (function, lines) in &observed {
        let Some(entry) = INTERNAL_ERROR_SITES
            .iter()
            .find(|e| e.function == *function)
        else {
            let _ = writeln!(
                failures,
                "\n  UNLISTED -32603 site: `{function}` at line(s) {lines:?}.\n    Either it is \
                 unreachable on v2 (name the era guard) or a store NotFound never reaches it \
                 (name the arm that takes NotFound instead). A v2-reachable NotFound -> -32603 is \
                 the conformance defect this list exists to prevent."
            );
            continue;
        };
        if entry.hits != lines.len() {
            let _ = writeln!(
                failures,
                "\n  COUNT CHANGED: `{function}` was recorded with {} INTERNAL_ERROR emission(s) \
                 and now has {} at line(s) {lines:?}.\n    A new -32603 inside an \
                 already-allowlisted function is exactly the shape this regression takes; \
                 re-derive the entry rather than raising the number.",
                entry.hits,
                lines.len()
            );
        }
    }

    for entry in INTERNAL_ERROR_SITES {
        if !observed.contains_key(entry.function) {
            let _ = writeln!(
                failures,
                "\n  STALE -32603 entry: `{}` no longer emits INTERNAL_ERROR. Delete the entry.",
                entry.function
            );
            continue;
        }
        let source = read(DISPATCH);
        let stripped = strip(&source);
        let (guard, guard_site) = match entry.disposition {
            Disposition::V1Only { guard, guard_site }
            | Disposition::NotFoundRoutedElsewhere { guard, guard_site } => (guard, guard_site),
            Disposition::RouterLeg => continue,
        };
        let present =
            fn_body(&stripped, guard_site).is_some_and(|body| needle_present(body, guard));
        if !present {
            let _ = writeln!(
                failures,
                "\n  MISSING guard: `{}`'s -32603 emissions are kept off the v2 path by `{guard}` \
                 inside `{guard_site}`, and that expression is gone.",
                entry.function
            );
        }
    }

    assert!(
        failures.is_empty(),
        "the tasks dispatch -32603 population changed:{failures}"
    );
    assert_justifications(
        "INTERNAL_ERROR_SITES",
        &INTERNAL_ERROR_SITES
            .iter()
            .map(|entry| (entry.function, entry.why))
            .collect::<Vec<_>>(),
    );
}

/// A v2 `NotFound` maps to `-32602`, positionally, inside the one function that
/// decides it.
///
/// The count check above notices that the POPULATION changed. This notices that
/// the not-found ARM changed while the population did not — which is what a
/// straight swap of `INVALID_PARAMS` for `INTERNAL_ERROR` looks like on the one
/// arm that matters. The measure is order: in `store_error_response`, the first
/// error-code constant appearing after the first `TaskStoreError::NotFound` must
/// be `INVALID_PARAMS`.
#[test]
fn the_v2_store_not_found_arm_still_maps_to_minus_32602() {
    let source = read(DISPATCH);
    let stripped = strip(&source);
    let body_span = fn_spans(&stripped)
        .into_iter()
        .find(|(name, _)| name == "store_error_response")
        .map(|(_, span)| span)
        .expect("store_error_response is the era-aware store mapping and must exist");
    let body = &stripped.text[body_span.clone()];

    let not_found = token_hits(body, "NotFound")
        .first()
        .copied()
        .expect("store_error_response must still match TaskStoreError::NotFound");

    let next_invalid = token_hits(body, "INVALID_PARAMS")
        .into_iter()
        .find(|at| *at > not_found);
    let next_internal = token_hits(body, "INTERNAL_ERROR")
        .into_iter()
        .find(|at| *at > not_found);

    let line = |at: usize| line_of(&stripped, body_span.start + at);
    match (next_invalid, next_internal) {
        (Some(invalid), Some(internal)) => assert!(
            invalid < internal,
            "the v2 NotFound arm of store_error_response now reaches INTERNAL_ERROR (-32603) at \
             {DISPATCH}:{} before INVALID_PARAMS (-32602) at {DISPATCH}:{}.\n  The extension \
             makes -32602 a MUST for a tasks/get naming a nonexistent taskId, and pmcp surfaces \
             an owner MISMATCH as NotFound on purpose, so this arm is the one a cross-caller \
             probe reads.",
            line(internal),
            line(invalid)
        ),
        (Some(_), None) => {},
        (None, _) => panic!(
            "no INVALID_PARAMS follows the NotFound arm in store_error_response \
             ({DISPATCH}:{}); the v2 not-found mapping is gone",
            line(not_found)
        ),
    }
}

// ===========================================================================
// 6. TASK 2(b) — the status identity, at the SOURCE level.
// ===========================================================================

/// The five `TaskStatus` serde strings, derived from the SOURCE TEXT.
///
/// Deriving them by serializing `TaskStatus::Working` and its four siblings
/// would require naming all five variants here, and a SIXTH variant added to the
/// enum would then be invisible to this test — which is precisely the drift it
/// exists to catch. So the variants are parsed out of the declaration, the
/// container's `rename_all` is read and asserted, and any per-variant
/// `#[serde(rename = "…")]` is honoured.
fn task_status_wire_strings() -> BTreeSet<String> {
    let source = read(TASK_TYPES);
    let stripped = strip_keeping_literals(&source);
    let text = &stripped.text;

    let at = text
        .find("enum TaskStatus")
        .expect("src/types/tasks.rs must declare `enum TaskStatus`");
    let container = &text[at.saturating_sub(200)..at];
    assert!(
        container.contains("rename_all = \"snake_case\""),
        "TaskStatus no longer carries `#[serde(rename_all = \"snake_case\")]`. This test derives \
         the wire strings by applying that exact transform, so a different one silently makes \
         every string below wrong. Observed attributes: {container:?}"
    );

    let brace = text[at..].find('{').expect("an enum has a body") + at;
    let end = balanced_end(text, brace).expect("balanced enum body");
    let body = &text[brace + 1..end];

    let mut out = BTreeSet::new();
    for part in body.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let rename = part.find("rename = \"").map(|r| {
            let rest = &part[r + "rename = \"".len()..];
            rest[..rest.find('"').expect("a closed rename literal")].to_string()
        });
        let ident: String = part
            .rsplit(']')
            .next()
            .unwrap_or(part)
            .trim()
            .chars()
            .take_while(|c| is_ident_char(*c))
            .collect();
        if ident.is_empty() {
            continue;
        }
        out.insert(rename.unwrap_or_else(|| snake_case(&ident)));
    }
    out
}

/// `InputRequired` -> `input_required`, the transform `rename_all` names.
fn snake_case(ident: &str) -> String {
    let mut out = String::new();
    for (index, ch) in ident.chars().enumerate() {
        if ch.is_uppercase() && index > 0 {
            out.push('_');
        }
        out.extend(ch.to_lowercase());
    }
    out
}

/// The `TaskStatus` union of the vendored extension schema, read at runtime.
fn schema_task_status_strings() -> BTreeSet<String> {
    let schema = read(VENDORED_SCHEMA);
    let at = schema
        .find("export type TaskStatus =")
        .expect("the vendored ext-tasks schema must declare `export type TaskStatus`");
    let rest = &schema[at..];
    let end = rest.find(';').expect("the union declaration is terminated");
    let union = &rest[..end];

    let mut out = BTreeSet::new();
    let mut chars = union.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch != '"' {
            continue;
        }
        let rest = &union[index + 1..];
        let close = rest.find('"').expect("a closed schema string literal");
        out.insert(rest[..close].to_string());
        while let Some((next, _)) = chars.peek() {
            if *next <= index + close + 1 {
                chars.next();
            } else {
                break;
            }
        }
    }
    out
}

/// The Rust source and the vendored schema declare the SAME SET of status
/// strings — exact set equality, no subset, no wildcard.
///
/// # How this differs from its behavioural twin
///
/// `tests/v2_tasks_shapes.rs :: task_status_wire_strings_match_the_extension_schema`
/// (plan 114-11) asserts what a RUNNING server emits. This asserts what the
/// SOURCE declares, and the two fail for different reasons: this one fires for a
/// status that no route currently returns, and for a variant added with the arms
/// needed to compile but with no wire path yet. A behavioural test cannot
/// observe either, because there is nothing to observe.
///
/// # Why this pair is the whole of TASK-04, and no conversion table is needed
///
/// F15 measured that the v1 five-state enum is NAME-IDENTICAL to the v2 one:
/// the extension did not rename, reorder or repartition the statuses. So
/// TASK-04's "maps deterministically" is satisfied by exactly two locks — the
/// wire strings are set-equal to the published union (here) and the running
/// server emits them (the behavioural twin) — and NOT by a translation layer.
/// A future reader who adds a v1->v2 status conversion function is adding a
/// second place for the mapping to be wrong; there is nothing for it to convert.
#[test]
fn the_task_status_wire_strings_are_set_equal_to_the_vendored_schema() {
    let declared = task_status_wire_strings();
    let published = schema_task_status_strings();

    assert_eq!(
        declared.len(),
        5,
        "TaskStatus no longer declares exactly five wire strings. Observed: {declared:?}"
    );
    assert_eq!(
        declared, published,
        "the TaskStatus wire strings drifted from {VENDORED_SCHEMA}.\n  Rust declares \
         {declared:?}\n  the vendored schema declares {published:?}\n  These are compared as \
         SETS: a subset would pass while a status silently went unrepresented."
    );
}

/// Nothing in the status mapping can absorb an unknown value.
///
/// A `_ =>` arm is how a sixth status stops being a compile error and starts
/// being a silent default. Both total mappings over `TaskStatus` are checked:
/// its `Display` impl (which produces the strings the test above derives) and
/// `Task::poll_decision` (which every client poll loop branches on).
#[test]
fn the_task_status_mappings_carry_no_wildcard_arm() {
    let source = read(TASK_TYPES);
    let stripped = strip(&source);

    let display = block_after(&stripped, "impl std::fmt::Display for TaskStatus")
        .expect("TaskStatus must still implement Display");
    assert!(
        !display.contains("_ =>"),
        "the Display impl for TaskStatus gained a wildcard arm; a sixth status would then render \
         as whatever that arm says instead of failing to compile"
    );

    let poll = fn_body(&stripped, "poll_decision").expect("Task::poll_decision must still exist");
    assert!(
        !poll.contains("_ =>"),
        "Task::poll_decision gained a wildcard arm; every client poll loop branches on it, so an \
         unrecognised status would silently take one existing branch"
    );
}

// ===========================================================================
// 7. TASK 2(c) — every wire value this phase introduced has an attribution.
// ===========================================================================

/// How walkable a constant's provenance attribution is.
enum Attribution {
    /// The rustdoc names an artifact the D-18 gate can OPEN: the vendored schema
    /// path, or the recheck record.
    Pinned,
    /// The rustdoc cites the extension by name but points at no file. Weaker on
    /// purpose, and recorded rather than papered over: `records` names the
    /// deferral that tracks it.
    ProseOnly { records: &'static str },
}

/// Tokens that make an attribution mechanically walkable.
const PINNED_TOKENS: [&str; 2] = ["schema/vendored/ext-tasks", "114-SPEC-RECHECK"];

/// The weakest attribution this file accepts at all.
const PROSE_TOKEN: &str = "ext-tasks";

struct ProvenanceEntry {
    constant: &'static str,
    path: &'static str,
    /// The literal start of the declaration line, so a rename fails here.
    decl: &'static str,
    /// The item whose rustdoc CARRIES the attribution: the constant itself, or
    /// the table it is declared as a row of.
    attribution_decl: &'static str,
    attribution: Attribution,
    why: &'static str,
}

/// Every wire value this phase introduced into `src/`, with the item whose
/// rustdoc attributes it.
///
/// The discipline: a value cannot enter the tree without an attribution the D-18
/// gate can walk. The lock is two-directional — a `Pinned` entry whose rustdoc
/// loses its artifact reference fails, and a `ProseOnly` entry whose rustdoc
/// GAINS one fails too, telling the next reader to promote the entry and close
/// the deferral.
const PROVENANCE: &[ProvenanceEntry] = &[
    ProvenanceEntry {
        constant: "TASKS_EXTENSION_KEY",
        path: "src/types/capabilities.rs",
        decl: "pub const TASKS_EXTENSION_KEY",
        attribution_decl: "pub const TASKS_EXTENSION_KEY",
        attribution: Attribution::Pinned,
        why: "The negotiation identifier the whole extension hangs off. Its rustdoc names the \
              vendored schema, the pinned upstream commit, a second corroborating artifact in the \
              core repository, AND the D-18 hold with the recheck record to re-verify against — \
              the fullest attribution in the phase, and the model the other two are graded on.",
    },
    ProvenanceEntry {
        constant: "V2_TASKS_METHOD_RETIRED",
        path: DISPATCH,
        decl: "pub(crate) const V2_TASKS_METHOD_RETIRED",
        attribution_decl: "pub(crate) const V2_TASKS_METHOD_RETIRED",
        attribution: Attribution::Pinned,
        why: "The refusal sentence for tasks/list and tasks/result. It is a wire value derived \
              from an ABSENCE — the vendored schema declares three request methods and not these \
              two — which is the one kind of claim a reader cannot check by reading the code, so \
              the rustdoc names the artifact the absence was measured in.",
    },
    ProvenanceEntry {
        constant: "TASKS_UPDATE_METHOD",
        path: MRTR,
        decl: "pub(crate) const TASKS_UPDATE_METHOD",
        attribution_decl: "pub(crate) const TASK_NAME_BEARING_METHODS",
        attribution: Attribution::ProseOnly { records: "D-114-Q" },
        why: "The method string itself carries only `See [TASKS_GET_METHOD]`; the attribution \
              lives on the routing table it is a row of, which cites the extension's Streamable \
              HTTP routing-header section in PROSE and names no file. Measured, not assumed: the \
              constant's own doc block contains neither walkable token. Recorded as a deferral \
              because closing it edits production, which this coverage-only plan may not do.",
    },
];

/// The contiguous `///` / attribute block immediately above `decl`.
fn doc_block_above(source: &str, decl: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.trim_start().starts_with(decl))?;
    let mut start = at;
    while start > 0 {
        let prev = lines[start - 1].trim_start();
        if prev.starts_with("///") || prev.starts_with("#[") {
            start -= 1;
        } else {
            break;
        }
    }
    Some(lines[start..at].join("\n"))
}

/// Every wire-value constant this phase introduced carries an attribution, at
/// the strength its entry claims.
#[test]
fn every_wire_value_constant_this_phase_introduced_carries_an_attribution() {
    let mut failures = String::new();

    for entry in PROVENANCE {
        let source = read(entry.path);
        if doc_block_above(&source, entry.decl).is_none() {
            let _ = writeln!(
                failures,
                "\n  MISSING declaration: {} no longer declares `{}` as `{}`.\n    Either it was \
                 renamed (update the entry) or deleted (delete the entry); a stale entry means \
                 this check silently stops covering a live wire value.",
                entry.path, entry.constant, entry.decl
            );
            continue;
        }
        let Some(doc) = doc_block_above(&source, entry.attribution_decl) else {
            let _ = writeln!(
                failures,
                "\n  MISSING attribution site: `{}`'s provenance is recorded on `{}` in {}, which \
                 no longer exists.",
                entry.constant, entry.attribution_decl, entry.path
            );
            continue;
        };
        let pinned = PINNED_TOKENS.iter().any(|token| doc.contains(token));
        match entry.attribution {
            Attribution::Pinned if !pinned => {
                let _ = writeln!(
                    failures,
                    "\n  LOST PROVENANCE: `{}` in {} no longer names {:?} anywhere in the rustdoc \
                     of `{}`.\n    A wire value with no walkable attribution cannot be re-verified \
                     when the extension publishes, which is the entire mechanism of the D-18 gate.",
                    entry.constant, entry.path, PINNED_TOKENS, entry.attribution_decl
                );
            },
            Attribution::Pinned => {},
            Attribution::ProseOnly { records } => {
                if pinned {
                    let _ = writeln!(
                        failures,
                        "\n  PROVENANCE IMPROVED: `{}` is recorded as prose-only under {records}, \
                         and `{}`'s rustdoc now names a walkable artifact.\n    Promote the entry \
                         to Attribution::Pinned and close the deferral.",
                        entry.constant, entry.attribution_decl
                    );
                } else if !doc.contains(PROSE_TOKEN) {
                    let _ = writeln!(
                        failures,
                        "\n  NO ATTRIBUTION AT ALL: `{}` in {} has neither a walkable artifact \
                         reference nor the `{PROSE_TOKEN}` prose citation on `{}`.\n    Recorded \
                         under {records}; losing the prose citation too makes the value \
                         unattributable.",
                        entry.constant, entry.path, entry.attribution_decl
                    );
                }
            },
        }
    }

    assert!(
        failures.is_empty(),
        "the phase's provenance discipline changed:{failures}"
    );
    assert_justifications(
        "PROVENANCE",
        &PROVENANCE
            .iter()
            .map(|entry| (entry.constant, entry.why))
            .collect::<Vec<_>>(),
    );
}

// ===========================================================================
// 8. The published-core codes this phase reused (amendment, 2026-07-29).
// ===========================================================================

/// `-32021` and `-32602` are bound to the NAMES this phase emits them under.
///
/// The MCP `2026-07-28` **core** specification published on 2026-07-29 with
/// `LATEST_PROTOCOL_VERSION = "2026-07-28"` and declares
/// `MISSING_REQUIRED_CLIENT_CAPABILITY = -32021` and `INVALID_PARAMS = -32602`.
/// (`ext-tasks` did NOT publish; it is still `draft/`, so the D-18 hold stays
/// engaged and every *tasks* wire value stays provisional. Full record:
/// `114-SPEC-RECHECK.md` § `### Verdict re-verification` -> `#### 2026-07-29`.)
///
/// These two are therefore no longer provisional, and both halves are asserted:
/// the NUMBER each name carries, and — for `-32021` — that this is the constant
/// the non-declaring-client refusal actually reaches for. A renumbering upstream
/// or a re-point to a different constant fails here rather than on a customer's
/// wire.
#[test]
fn the_published_core_codes_this_phase_reused_are_pinned_by_name() {
    assert_eq!(
        error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY,
        -32021,
        "MISSING_REQUIRED_CLIENT_CAPABILITY is the published core's own allocation; the tasks \
         declaration gate emits it and the conformance suite reads the NUMBER"
    );
    assert_eq!(
        error_codes::INVALID_PARAMS,
        -32602,
        "INVALID_PARAMS is the code the extension makes a MUST for a tasks/get naming a \
         nonexistent taskId"
    );

    let source = read(DISPATCH);
    let stripped = strip(&source);
    let body = fn_body(&stripped, "missing_tasks_declaration_refusal")
        .expect("the non-declaring-client refusal must still exist");
    assert!(
        needle_present(body, "MISSING_REQUIRED_CLIENT_CAPABILITY"),
        "missing_tasks_declaration_refusal no longer names MISSING_REQUIRED_CLIENT_CAPABILITY. \
         Pinning the constant's VALUE proves nothing if the refusal reaches for a different one."
    );
    for other in [
        "INTERNAL_ERROR",
        "INVALID_PARAMS",
        "METHOD_NOT_FOUND",
        "AUTHENTICATION_REQUIRED",
        "V1_TASK_PENDING",
    ] {
        assert!(
            !needle_present(body, other),
            "missing_tasks_declaration_refusal now also names {other}; case 3 of the ordered chain \
             answers exactly one code, and a second one there means the refusal branched"
        );
    }
}

/// `-32002` has no v2-reachable emission site, measured over BOTH its names and
/// the bare number.
///
/// The published core omits `-32002` entirely, which turns this from a pmcp
/// policy into a spec-grounded invariant. `tests/v2_prohibited_error_codes.rs`
/// already tracks the NAME `V1_TASK_PENDING` per FILE and proves the two
/// emission sites v1-only by EXECUTION; this adds the two axes that file does
/// not have: the second name `UNSUPPORTED_CAPABILITY` and the bare numeric
/// literal, so a future site writing `-32002` directly is caught, and a MEASURED
/// per-file hit count rather than mere presence.
#[test]
fn minus_32002_has_no_v2_reachable_emission_site() {
    let mut observed: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for path in shipped_files() {
        let source = fs::read_to_string(&path).expect("readable source");
        let stripped = strip(&source);
        let excluded = cfg_test_spans(&stripped);
        let mut hits: Vec<usize> = Vec::new();
        for name in ["V1_TASK_PENDING", "UNSUPPORTED_CAPABILITY"] {
            hits.extend(token_hits(&stripped.text, name));
        }
        hits.extend(numeric_hits(&stripped.text, "-32002"));
        hits.retain(|at| !is_excluded(&excluded, *at));
        if !hits.is_empty() {
            hits.sort_unstable();
            observed.insert(
                rel(&path),
                hits.into_iter().map(|at| line_of(&stripped, at)).collect(),
            );
        }
    }

    let declaring = "src/types/protocol/error_codes.rs";
    assert!(
        observed.contains_key(declaring),
        "the -32002 declaration site vanished from the scan; the scanner is over-stripping and \
         every claim below would pass vacuously. Observed: {observed:?}"
    );

    let mut failures = String::new();
    for (path, lines) in &observed {
        let guard = match path.as_str() {
            "src/types/protocol/error_codes.rs" | "src/error/mod.rs" => continue,
            "src/server/core.rs" => "v1_initialize_gate_applies",
            DISPATCH => "is_v1_task_era",
            _ => {
                let _ = writeln!(
                    failures,
                    "\n  UNLISTED -32002 site: {path} at line(s) {lines:?}.\n    Protocol version \
                     2026-07-28 MUST NOT emit -32002 and the published core does not declare it \
                     at all. Era-gate the site and record it here, or do not emit it."
                );
                continue;
            },
        };
        if !read(path).contains(guard) {
            let _ = writeln!(
                failures,
                "\n  MISSING era guard: {path} emits -32002 at line(s) {lines:?} and no longer \
                 contains `{guard}`."
            );
        }
    }
    assert!(
        failures.is_empty(),
        "the -32002 population changed:{failures}"
    );

    // The count is MEASURED against the tree, and pinned so that a new emission
    // hiding inside an already-listed file cannot pass as "the same set".
    let dispatch_hits = observed.get(DISPATCH).map_or(0, Vec::len);
    assert_eq!(
        dispatch_hits,
        1,
        "the tasks dispatch was measured with exactly ONE -32002 emission (the v1 tasks/result \
         pending refusal). Now: {:?}",
        observed.get(DISPATCH)
    );
}

/// Occurrences of a NEGATIVE numeric literal, not embedded in a longer number.
fn numeric_hits(text: &str, number: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    occurrences(text, number)
        .into_iter()
        .filter(|at| {
            let after = at + number.len();
            let after_ok = after >= bytes.len() || !bytes[after].is_ascii_digit();
            let before_ok = *at == 0 || !bytes[at - 1].is_ascii_digit();
            before_ok && after_ok
        })
        .collect()
}

// ===========================================================================
// 9. Shared allowlist discipline + anti-vacuity for the exclusions.
// ===========================================================================

/// Every entry in a justified allowlist carries a real, distinct reason.
///
/// Length alone is trivially defeated by padding; pairwise distinctness alone is
/// defeated by five one-word labels. Both together mean a copy-pasted or empty
/// justification fails.
fn assert_justifications(label: &str, entries: &[(&str, &str)]) {
    let mut seen: Vec<&str> = Vec::new();
    for (name, why) in entries {
        let why = why.trim();
        assert!(
            why.len() >= MIN_JUSTIFICATION_CHARS,
            "{label} entry `{name}` needs a real justification, not {why:?} ({} chars, minimum \
             {MIN_JUSTIFICATION_CHARS})",
            why.len()
        );
        assert!(
            !seen.contains(&why),
            "{label} entry `{name}` reuses another entry's justification verbatim; a copy-pasted \
             reason is not a reason"
        );
        seen.push(why);
    }
    assert!(
        !entries.is_empty(),
        "{label} is EMPTY, so every check keyed on it passes over nothing"
    );
}

/// ANTI-VACUITY for the two exclusions this file depends on.
///
/// Both can fail silently in opposite directions, and both would make the checks
/// above pass over the wrong set:
///
/// * The `cfg(test)`-region exclusion could stop working (test assertions on
///   `-32002` would enter the shipped population) or over-reach by truncating at
///   the first marker (thousands of production lines would leave it).
/// * The test-only MODULE-FILE exclusion could stop discovering declarations, in
///   which case five whole test files rejoin the shipped population.
#[test]
fn the_test_only_exclusions_are_load_bearing() {
    let test_only = test_only_module_files();
    for required in [
        "src/server/task_dispatch_tests.rs",
        "src/server/core_tests.rs",
        "src/server/wasm_core_tests.rs",
    ] {
        assert!(
            test_only.contains(required),
            "{required} is declared `#[cfg(test)] mod …` and asserts on -32002, yet the \
             module-declaration scan did not find it. Observed: {test_only:?}"
        );
    }

    let shipped: BTreeSet<String> = shipped_files().iter().map(|p| rel(p)).collect();
    assert!(
        !shipped.contains("src/server/task_dispatch_tests.rs"),
        "a cfg(test)-declared module file entered the shipped population"
    );
    assert!(
        shipped.contains(DISPATCH),
        "{DISPATCH} left the shipped population; every check over it would pass vacuously"
    );
    assert!(
        shipped.len() > 50,
        "the shipped population collapsed to {} files",
        shipped.len()
    );

    // Every `*_tests.rs` file in the tree must be one of the discovered
    // declarations. A new test-only file that is NOT cfg(test)-gated is a
    // decision someone has to make deliberately.
    for path in src_files() {
        let name = rel(&path);
        if name.ends_with("_tests.rs") {
            assert!(
                test_only.contains(&name),
                "{name} looks test-only but no `#[cfg(test)] mod` declaration was found for it"
            );
        }
    }
}

// ===========================================================================
// 10. Tests for the scanner itself — without these every check can pass
//     vacuously, which is the failure mode plan 113-09 found twice.
// ===========================================================================

mod scanner {
    use super::{
        block_after, cfg_requires_test, cfg_test_spans, declared_module_file, enclosing_fn,
        fn_body, fn_spans, is_excluded, line_of, needle_present, numeric_hits, snake_case, strip,
        strip_keeping_literals, token_hits,
    };

    fn find_token(source: &str, needle: &str) -> Option<u32> {
        let stripped = strip(source);
        let at = *token_hits(&stripped.text, needle).first()?;
        Some(line_of(&stripped, at))
    }

    #[test]
    fn a_bare_emission_is_counted() {
        let source = "fn f() -> i32 {\n    error_codes::INTERNAL_ERROR\n}\n";
        assert_eq!(find_token(source, "INTERNAL_ERROR"), Some(2));
    }

    #[test]
    fn the_token_only_inside_a_comment_is_not_counted() {
        let line = "fn f() {\n    // returns INTERNAL_ERROR on a store failure\n    g();\n}\n";
        assert!(find_token(line, "INTERNAL_ERROR").is_none());

        let doc = "/// Locked to [`INTERNAL_ERROR`] by the tripwire.\nfn f() {}\n";
        assert!(find_token(doc, "INTERNAL_ERROR").is_none());

        let block = "fn f() {\n/* INTERNAL_ERROR /* nested */ still comment */\nlet y = 1;\n}\n";
        assert!(find_token(block, "INTERNAL_ERROR").is_none());
        assert!(
            find_token(block, "let").is_some(),
            "a nested block comment must end where Rust says it ends"
        );
    }

    #[test]
    fn a_literal_is_stripped_in_one_mode_and_kept_in_the_other() {
        let source = "fn f() {\n    let m = \"tasks/get\";\n}\n";
        assert!(
            !strip(source).text.contains("tasks/get"),
            "the identifier mode must delete literal contents"
        );
        assert!(
            strip_keeping_literals(source)
                .text
                .contains("\"tasks/get\""),
            "the wire-string mode must keep them; the tasks method scan depends on it"
        );
    }

    #[test]
    fn a_longer_identifier_is_not_a_hit() {
        let source = "const INTERNAL_ERROR_CODES: &[i32] = &[];\n";
        assert!(
            find_token(source, "INTERNAL_ERROR").is_none(),
            "a substring of a longer identifier must not count as an emission site"
        );
    }

    #[test]
    fn the_token_inside_a_cfg_test_block_is_excluded_but_later_code_is_not() {
        let source = "#[cfg(test)]\nmod tests {\n    const A: i32 = INTERNAL_ERROR;\n}\n\n\
                      fn shipped() -> i32 {\n    INTERNAL_ERROR\n}\n";
        let stripped = strip(source);
        let spans = cfg_test_spans(&stripped);
        let hits = token_hits(&stripped.text, "INTERNAL_ERROR");
        assert_eq!(hits.len(), 2, "both occurrences are lexically present");
        assert!(is_excluded(&spans, hits[0]));
        assert!(
            !is_excluded(&spans, hits[1]),
            "production code AFTER a cfg(test) item must still be scanned — truncating at the \
             first marker is the under-scan this file exists to prevent"
        );
    }

    #[test]
    fn cfg_requires_test_classifies_the_documented_predicate_shapes() {
        assert!(cfg_requires_test("test"));
        assert!(cfg_requires_test(
            "all(test, not(target_arch = \"wasm32\"))"
        ));
        assert!(
            !cfg_requires_test("any(feature = \"fuzzing\", test)"),
            "an any(...) item compiles WITHOUT test, so it ships and stays in scope"
        );
        assert!(!cfg_requires_test("feature = \"testing\""));
    }

    #[test]
    fn a_module_declaration_resolves_to_its_file_honouring_a_path_attribute() {
        assert_eq!(
            declared_module_file("] mod core_tests;").as_deref(),
            Some("core_tests.rs")
        );
        assert_eq!(
            declared_module_file("] #[path = \"wasm_core_tests.rs\"] mod wasm_core_tests;")
                .as_deref(),
            Some("wasm_core_tests.rs")
        );
        assert_eq!(
            declared_module_file("] mod inline { fn f() {} }"),
            None,
            "an inline module body is already covered by the cfg(test) region exclusion"
        );
    }

    #[test]
    fn function_spans_are_innermost_and_skip_bodiless_declarations() {
        let source = "trait T {\n    fn declared(&self) -> bool;\n}\n\
                      fn outer() {\n    let a = 1;\n    fn inner() {\n        let b = 2;\n    }\n}\n";
        let stripped = strip(source);
        let spans = fn_spans(&stripped);
        let names: Vec<&str> = spans.iter().map(|(n, _)| n.as_str()).collect();
        assert!(
            !names.contains(&"declared"),
            "a trait method with no body must not claim the next function's braces"
        );
        assert!(names.contains(&"outer") && names.contains(&"inner"));

        let at = *token_hits(&stripped.text, "b")
            .first()
            .expect("b is present");
        assert_eq!(
            enclosing_fn(&spans, at),
            Some("inner"),
            "the INNERMOST enclosing function wins"
        );
        assert!(fn_body(&stripped, "outer")
            .expect("outer")
            .contains("inner"));
    }

    #[test]
    fn a_guard_is_found_in_its_own_body_and_not_in_a_sibling() {
        let source = "fn guarded() {\n    if is_v1_task_era(era) { return; }\n}\n\
                      fn other() {\n    g();\n}\n";
        let stripped = strip(source);
        assert!(needle_present(
            fn_body(&stripped, "guarded").expect("guarded"),
            "is_v1_task_era"
        ));
        assert!(
            !needle_present(
                fn_body(&stripped, "other").expect("other"),
                "is_v1_task_era"
            ),
            "a guard in a SIBLING function must not satisfy this route's entry; that is the \
             whole reason each GuardRef names a site"
        );
    }

    #[test]
    fn block_after_returns_the_balanced_impl_body() {
        let source = "impl std::fmt::Display for TaskStatus {\n    fn fmt(&self) {\n        \
                      match self { Self::A => 1 }\n    }\n}\nfn after() { let _ = 0; }\n";
        let stripped = strip(source);
        let block = block_after(&stripped, "impl std::fmt::Display for TaskStatus")
            .expect("the impl block");
        assert!(block.contains("Self::A"));
        assert!(
            !block.contains("after"),
            "the block must stop at its own closing brace"
        );
    }

    #[test]
    fn snake_case_matches_what_rename_all_produces() {
        assert_eq!(snake_case("Working"), "working");
        assert_eq!(snake_case("InputRequired"), "input_required");
        assert_eq!(snake_case("Cancelled"), "cancelled");
    }

    #[test]
    fn a_numeric_literal_is_matched_whole() {
        assert_eq!(numeric_hits("let a = -32002;", "-32002").len(), 1);
        assert!(
            numeric_hits("let a = -320021;", "-32002").is_empty(),
            "-32002 must not match inside a longer number"
        );
    }
}
