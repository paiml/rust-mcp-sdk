//! Phase 115 tripwires — the invariants configuration alone cannot keep.
//!
//! # What this file fences
//!
//! * **SEP-2106 (SCHM-01)** — output-schema validation must never fetch an
//!   external `$ref`. That property is currently STRUCTURAL: `jsonschema` is
//!   declared `default-features = false` everywhere, so its `resolve-http` /
//!   `resolve-file` retrievers are not compiled in and an external `$ref`
//!   compiles down to a hard `Err`. Cargo feature unification is decided by the
//!   WHOLE dependency graph, though, so one workspace member, example or
//!   dev-dependency declaring `jsonschema` with default features turns the
//!   refusal into a live outbound fetch — and a behavioural test would still
//!   pass, indeed it would pass *better*, because the fetch would succeed. Only
//!   a dependency-graph check catches that.
//! * **D-12 (SCHM-03)** — the caching hints are written at exactly ONE shared
//!   projection point, in the `cfg`-free `src/types/caching.rs`.
//! * **The wasm dispatcher's strip call** — `src/server/wasm_server.rs` is
//!   `cfg(target_arch = "wasm32")`, so no native gate compiles it and no gate at
//!   all executes it. 115-06 MEASURED that deleting its `project_caching_hints`
//!   call leaves `make wasm-build` at exit 0, which makes the source assertion
//!   here the only automated gate that can catch the removal.
//! * **The projection-versus-middleware ordering** — pinned by measurement, so a
//!   silent reorder fails a named test.
//!
//! # Manifests are NEVER read as text
//!
//! The pre-review shape of this file scanned `Cargo.toml` dependency LINES with
//! string matching. That misses a table-style declaration, a multiline
//! declaration, a dependency renamed via `package = "jsonschema"`, and any
//! future `[workspace.dependencies]` inheritance. This file parses cargo's own
//! output instead, in two layers:
//!
//! 1. `cargo metadata --no-deps` → every workspace package's DECLARED
//!    dependency, with `rename`, `optional`, `uses_default_features` and
//!    `features` as structured fields;
//! 2. `cargo metadata --features validation` → the RESOLVED graph's
//!    `resolve.nodes[].features`, which is the definitive unification answer and
//!    the only layer that sees a dev-dependency or an example turning a feature
//!    on.
//!
//! This needs no new dependency: `std::process::Command` plus `serde_json`.
//!
//! # The scanner primitives are duplicated — and this is a KNOWN DEBT, not a design
//!
//! It is true that a Rust integration test is its own crate, so this file cannot
//! import `tests/v2_tasks_tripwires.rs`'s scanner and that file cannot import
//! this one. An earlier version of this note concluded from that premise that
//! the primitives therefore had to be RESTATED. **That conclusion is wrong**:
//! sharing between test binaries does not require one binary to import another,
//! it requires a module under `tests/common/` pulled in per-crate with
//! `#[path = "common/<name>.rs"] mod <name>;`. This repository already does
//! exactly that — `tests/common/duplex.rs` is shared that way, by
//! `tests/v2_caching_hints.rs` and `tests/structured_tool_output.rs` among
//! others.
//!
//! Measured: ~386 lines of the block below are byte-identical with
//! `tests/v2_tasks_tripwires.rs`, and the same primitives now exist in four test
//! files. The intent behind the original note is still right — the repository
//! should have ONE source-scanning shape rather than several divergent ones —
//! but duplication is the wrong mechanism for it, because nothing keeps the
//! copies identical and the doc comments have already begun to drift.
//!
//! The extraction to `tests/common/rust_source_scan.rs` is deferred rather than
//! declined: it touches test files outside this phase's diff and wants its own
//! change with its own green run.
//!
//! # Every test name carries the file stem
//!
//! Every test function here begins with `v2_schema_tripwires_`, so BOTH
//! `binary(v2_schema_tripwires)` and `test(/v2_schema_tripwires/)` select this
//! suite. The nextest `test(...)` selector matches the test NAME, not the binary
//! name, and silently selects zero tests when the two differ — which is not a
//! failure, it is a green run over nothing.
//!
//! # When a check here fails
//!
//! Restore the invariant, or move the allowlist entry and write down why.
//! Deleting the check, or widening it until it passes, is the failure mode it
//! exists to prevent.
#![cfg(not(target_arch = "wasm32"))]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// A justification shorter than this is a label, not a decision.
const MIN_JUSTIFICATION_CHARS: usize = 40;

/// The crate whose feature set decides whether output validation can fetch.
const JSONSCHEMA: &str = "jsonschema";

/// The module that owns the era-keyed validator compilation.
const OUTPUT_VALIDATION: &str = "src/server/output_validation.rs";

/// `jsonschema`'s DEFAULT features, verbatim from `cargo info jsonschema`, plus
/// the two other resolver/TLS features a manifest could reach for.
///
/// `resolve-http` pulls in `reqwest` + `rustls`; `resolve-async` pulls in
/// `referencing/retrieve-async` and `reqwest`. Any of them turns an external
/// `$ref` inside output validation into an outbound request.
const RESOLVER_FEATURES: &[&str] = &[
    "resolve-http",
    "resolve-file",
    "resolve-async",
    "tls-aws-lc-rs",
    "tls-ring",
];

/// Identifiers that install a `$ref` retriever, in any of its spellings.
///
/// `with_retriever` / `with_http_options` are the builder entry points;
/// `Retrieve` / `AsyncRetrieve` / `Retriever` catch a hand-written
/// implementation of the trait, which is the same capability arrived at from the
/// other direction.
const RETRIEVER_NEEDLES: &[&str] = &[
    "with_retriever",
    "with_http_options",
    "Retrieve",
    "AsyncRetrieve",
    "Retriever",
];

/// The two identifiers that construct a `jsonschema` validator.
const VALIDATOR_NEEDLES: &[&str] = &["validator_for", "draft202012"];

/// The remedy every SEP-2106 failure message points at.
const SEP_2106_WHY: &str = "\
SEP-2106: output-schema validation MUST NOT fetch an external `$ref`.\n\
  `jsonschema`'s DEFAULT features are [\"resolve-http\", \"resolve-file\", \"tls-aws-lc-rs\"], and \
cargo feature unification is GRAPH-WIDE: one workspace member, example or dev-dependency that \
enables any of them enables it for every crate in the graph, including the MCP output-validation \
path.\n  The refusal this phase measured (~60 microseconds, no socket) is STRUCTURAL — the \
retriever is not compiled in, so an external `$ref` is a hard `Err`. Enabling a resolver feature \
converts that into a live outbound fetch performed by the SERVER on a schema an untrusted tool \
author supplied: server-side request forgery from inside a validation path.\n  A behavioural test \
would NOT catch this. It would pass better, because the fetch would succeed.";

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
/// Whitespace collapses to a single space rather than vanishing, because this
/// scanner matches IDENTIFIERS, which need word boundaries: removing whitespace
/// entirely turns `pub const FOO` into `pubconstFOO`, whose preceding character
/// is an identifier character, so a whole-token filter would reject the
/// DEFINITION site and silently lose coverage of the file being scanned.
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
/// The caching-hint checks are about the WIRE strings `"ttlMs"` and
/// `"cacheScope"` and about serde attribute VALUES such as
/// `skip_serializing_if = "Option::is_none"`. All of those live inside string
/// literals, which [`strip`] deletes, so a second mode is required rather than
/// merely convenient.
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
/// drop thousands of production lines from the larger server modules.
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

/// A whole-token match: `Retrieve` must not match `Retriever`.
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

// --- function spans, so a site is attributed to the function that owns it ---

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

/// The INNERMOST function whose body contains `index`.
fn enclosing_fn(spans: &[(String, Range<usize>)], index: usize) -> Option<&str> {
    spans
        .iter()
        .filter(|(_, span)| span.contains(&index))
        .min_by_key(|(_, span)| span.end - span.start)
        .map(|(name, _)| name.as_str())
}

/// The body of `name` in `stripped`, or `None` when there is no such function.
fn fn_body<'a>(stripped: &'a Stripped, name: &str) -> Option<&'a str> {
    fn_spans(stripped)
        .into_iter()
        .find(|(found, _)| found == name)
        .map(|(_, span)| &stripped.text[span])
}

/// The index of the `[` that matches the `]` at `close`.
fn matching_open_bracket(text: &str, close: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth: usize = 0;
    let mut i = close + 1;
    while i > 0 {
        i -= 1;
        match bytes[i] {
            b']' => depth += 1,
            b'[' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            },
            _ => {},
        }
    }
    None
}

/// The `#[…]` attributes attached to the item starting at `at`, innermost first.
///
/// Walks backwards from the item, accepting only whitespace between one
/// attribute's `]` and the next construct, so an attribute belonging to an
/// EARLIER item is never mis-attributed to this one.
fn attrs_before(text: &str, at: usize) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut j = at;
    loop {
        while j > 0 && bytes[j - 1] == b' ' {
            j -= 1;
        }
        if j == 0 || bytes[j - 1] != b']' {
            break;
        }
        let close = j - 1;
        let Some(open) = matching_open_bracket(text, close) else {
            break;
        };
        if open == 0 || bytes[open - 1] != b'#' {
            break;
        }
        out.push(text[open + 1..close].to_string());
        j = open - 1;
    }
    out
}

/// The first quoted value assigned to `key` inside an attribute body.
fn attr_value(attr: &str, key: &str) -> Option<String> {
    let at = attr.find(key)?;
    let rest = &attr[at + key.len()..];
    let open = rest.find('"')?;
    let close = rest[open + 1..].find('"')? + open + 1;
    Some(rest[open + 1..close].to_string())
}

// ===========================================================================
// 2. Cargo's own dependency graph — declared AND resolved.
// ===========================================================================

/// Run `cargo metadata` with `args` and parse its stdout.
///
/// Fails LOUDLY on a non-zero exit, naming the command, its status and its
/// stderr, rather than returning an empty document. A broken invocation that
/// returned "no dependencies" would make every check keyed on it pass over
/// nothing, which is the exact failure mode these tripwires exist to prevent.
fn cargo_metadata(args: &[&str]) -> Value {
    let cargo = env!("CARGO");
    let rendered = args.join(" ");
    let output = Command::new(cargo)
        .args(args)
        .current_dir(repo_root())
        .output()
        .unwrap_or_else(|e| panic!("cannot run `{cargo} {rendered}`: {e}"));
    assert!(
        output.status.success(),
        "`{cargo} {rendered}` exited with {}; a broken invocation must fail loudly rather than \
         yield an empty dependency graph every check below would pass over.\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!("`{cargo} {rendered}` produced output that is not JSON metadata: {e}")
    })
}

fn string_array(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// One DECLARED dependency on [`JSONSCHEMA`], as cargo itself reports it.
///
/// `rename` is the field that catches a `package = "jsonschema"` alias: a
/// renamed dependency still reports `"name": "jsonschema"` with a non-null
/// `rename`, which a text scan of the manifest KEY would miss entirely.
#[derive(Debug)]
struct DeclaredDep {
    package: String,
    rename: Option<String>,
    optional: bool,
    uses_default_features: bool,
    features: Vec<String>,
    req: String,
    kind: Option<String>,
}

impl DeclaredDep {
    /// How this declaration should be named in a failure message.
    fn describe(&self) -> String {
        format!(
            "package `{}` (rename: {:?}, kind: {:?}, req: {}, optional: {})",
            self.package, self.rename, self.kind, self.req, self.optional
        )
    }
}

fn declared_dep(package: &str, dep: &Value) -> DeclaredDep {
    DeclaredDep {
        package: package.to_string(),
        rename: dep["rename"].as_str().map(ToString::to_string),
        optional: dep["optional"].as_bool().unwrap_or(false),
        uses_default_features: dep["uses_default_features"].as_bool().unwrap_or(true),
        features: string_array(&dep["features"]),
        req: dep["req"].as_str().unwrap_or("<unknown>").to_string(),
        kind: dep["kind"].as_str().map(ToString::to_string),
    }
}

/// Every workspace package's DECLARED dependency on [`JSONSCHEMA`].
///
/// Matching on the dependency's `name` rather than on the manifest key is what
/// catches a `package = "jsonschema"` alias, a table-style declaration, a
/// multiline declaration and a future `[workspace.dependencies]` inheritance —
/// every case a text scan of `Cargo.toml` misses.
fn declared_jsonschema_deps() -> Vec<DeclaredDep> {
    let meta = cargo_metadata(&["metadata", "--format-version", "1", "--no-deps"]);
    let packages = meta["packages"]
        .as_array()
        .expect("`cargo metadata` always reports a `packages` array");
    assert!(
        !packages.is_empty(),
        "`cargo metadata --no-deps` reported ZERO workspace packages; the manifest scan would \
         pass over nothing"
    );
    let mut out = Vec::new();
    for package in packages {
        let name = package["name"].as_str().unwrap_or("<unnamed>");
        let Some(deps) = package["dependencies"].as_array() else {
            continue;
        };
        for dep in deps {
            if dep["name"].as_str() == Some(JSONSCHEMA) {
                out.push(declared_dep(name, dep));
            }
        }
    }
    out.sort_by(|a, b| a.package.cmp(&b.package));
    out
}

/// One RESOLVED [`JSONSCHEMA`] node in the unified dependency graph.
#[derive(Debug)]
struct ResolvedNode {
    version: String,
    features: Vec<String>,
    id: String,
}

/// Every resolved [`JSONSCHEMA`] node, with the features unification settled on.
///
/// This is the definitive answer: `.resolve.nodes[].features` is what will
/// actually be compiled, after every workspace member, example, dev-dependency
/// and transitive dependency has had its say.
fn resolved_jsonschema_nodes() -> Vec<ResolvedNode> {
    let meta = cargo_metadata(&[
        "metadata",
        "--format-version",
        "1",
        "--features",
        "validation",
    ]);
    let packages = meta["packages"]
        .as_array()
        .expect("`cargo metadata` always reports a `packages` array");
    let by_id: BTreeMap<&str, &Value> = packages
        .iter()
        .filter_map(|package| package["id"].as_str().map(|id| (id, package)))
        .collect();
    let nodes = meta["resolve"]["nodes"]
        .as_array()
        .expect("a resolving `cargo metadata` run always reports `resolve.nodes`");
    assert!(
        !nodes.is_empty(),
        "`cargo metadata --features validation` resolved ZERO nodes; the unification check would \
         pass over nothing"
    );
    let mut out = Vec::new();
    for node in nodes {
        let Some(id) = node["id"].as_str() else {
            continue;
        };
        let Some(package) = by_id.get(id) else {
            continue;
        };
        if package["name"].as_str() != Some(JSONSCHEMA) {
            continue;
        }
        out.push(ResolvedNode {
            version: package["version"]
                .as_str()
                .unwrap_or("<unknown>")
                .to_string(),
            features: string_array(&node["features"]),
            id: id.to_string(),
        });
    }
    out
}

/// Every workspace package's `src/` directory, discovered from cargo metadata.
///
/// Runtime discovery rather than a hardcoded list: a NEW workspace member that
/// declared `jsonschema` or installed a retriever would otherwise escape both
/// scans by nobody remembering to add it here.
fn workspace_src_dirs() -> Vec<PathBuf> {
    let meta = cargo_metadata(&["metadata", "--format-version", "1", "--no-deps"]);
    let mut out: Vec<PathBuf> = meta["packages"]
        .as_array()
        .expect("`cargo metadata` always reports a `packages` array")
        .iter()
        .filter_map(|package| package["manifest_path"].as_str())
        .filter_map(|manifest| Path::new(manifest).parent().map(|dir| dir.join("src")))
        .filter(|dir| dir.is_dir())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Every `.rs` file under every workspace package's `src/`.
fn workspace_rs_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in workspace_src_dirs() {
        collect_rs_files(&dir, &mut files);
    }
    files.sort();
    files.dedup();
    assert!(
        files.len() > 300,
        "the workspace carries well over three hundred source files; discovering {} means the \
         walk is broken and every workspace-wide check here would pass vacuously",
        files.len()
    );
    files
}

// ===========================================================================
// 3. Shared allowlist discipline.
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

// ===========================================================================
// 4. TASK 1 — SEP-2106, over cargo's DECLARED graph.
// ===========================================================================

/// No workspace manifest may declare [`JSONSCHEMA`] with a resolver enabled.
#[test]
fn v2_schema_tripwires_no_manifest_declares_jsonschema_with_default_features() {
    let declared = declared_jsonschema_deps();
    let mut failures = String::new();

    for dep in &declared {
        if dep.uses_default_features {
            let _ = writeln!(
                failures,
                "\n  DEFAULT FEATURES ON: {} declares `{JSONSCHEMA}` WITHOUT \
                 `default-features = false`.\n    Add `default-features = false` to that \
                 declaration.",
                dep.describe()
            );
        }
        for feature in &dep.features {
            if RESOLVER_FEATURES.contains(&feature.as_str()) {
                let _ = writeln!(
                    failures,
                    "\n  RESOLVER FEATURE ON: {} enables `{JSONSCHEMA}/{feature}`.\n    Remove it \
                     from that declaration's `features` list.",
                    dep.describe()
                );
            }
        }
    }

    assert!(failures.is_empty(), "{SEP_2106_WHY}\n{failures}");
}

/// The RESOLVED graph — the definitive unification answer.
#[test]
fn v2_schema_tripwires_the_resolved_graph_enables_no_jsonschema_resolver_feature() {
    let nodes = resolved_jsonschema_nodes();

    assert_eq!(
        nodes.len(),
        1,
        "expected EXACTLY ONE resolved `{JSONSCHEMA}` node and found {}: {nodes:#?}.\n  Two nodes \
         means two copies are compiled into the same graph, which is the state 115-03's \
         workspace-wide bump to a single `0.49` requirement exists to prevent: one validator \
         could then be pinned and the other not.",
        nodes.len()
    );

    for node in &nodes {
        assert!(
            node.features.is_empty(),
            "{SEP_2106_WHY}\n  RESOLVED node `{}` compiles with features {:?}, not [].\n  This is \
             the ONLY check that sees the effect of a DEV-dependency, an example, or a sibling \
             workspace crate turning a feature on: unification is graph-wide, so the declared \
             dependency check above would still pass while the retriever is compiled in.",
            node.id,
            node.features
        );
        assert!(
            node.version.starts_with("0.49"),
            "the resolved `{JSONSCHEMA}` version is {}, not 0.49.x.\n  115-03 pinned the whole \
             workspace to `0.49` and MEASURED the Draft 2020-12 divergence case \
             (`contentEncoding`) against 0.49.2. A different major/minor may restore \
             `resolve-http` to a different default set or change the dialect behaviour, so the \
             measurement has to be redone before this pin moves.",
            node.version
        );
    }
}

/// ANTI-VACUITY for the two `cargo metadata` scans.
#[test]
fn v2_schema_tripwires_the_manifest_scan_is_not_vacuous() {
    let declared = declared_jsonschema_deps();
    assert!(
        declared.len() >= 3,
        "expected at least three DECLARED `{JSONSCHEMA}` dependencies and found {}: {declared:#?}\
         \n  Without this the two checks above would pass over an empty set, which is a green run \
         over nothing rather than a clean bill of health.",
        declared.len()
    );

    let packages: BTreeSet<&str> = declared.iter().map(|dep| dep.package.as_str()).collect();
    for required in ["pmcp", "pmcp-agent", "pmcp-server-toolkit"] {
        assert!(
            packages.contains(required),
            "`{required}` declares `{JSONSCHEMA}` (measured 2026-08-01) yet the metadata scan did \
             not find it. Observed: {packages:?}.\n  Either the declaration moved — in which case \
             update this list deliberately — or the scan is broken."
        );
    }

    let resolved = resolved_jsonschema_nodes();
    assert!(
        !resolved.is_empty(),
        "the RESOLVED scan found no `{JSONSCHEMA}` node at all; the unification check above would \
         then iterate over an empty vector and pass"
    );
}

/// No source file may install a `$ref` retriever, in any spelling.
#[test]
fn v2_schema_tripwires_no_source_installs_a_ref_retriever() {
    let mut failures = String::new();

    for path in workspace_rs_files() {
        let raw = fs::read_to_string(&path).expect("readable source");
        if !RETRIEVER_NEEDLES.iter().any(|needle| raw.contains(needle)) {
            continue;
        }
        let stripped = strip(&raw);
        for needle in RETRIEVER_NEEDLES {
            for at in token_hits(&stripped.text, needle) {
                let _ = writeln!(
                    failures,
                    "\n  RETRIEVER: `{needle}` at {}:{}",
                    rel(&path),
                    line_of(&stripped, at)
                );
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{SEP_2106_WHY}\n  A retriever was installed in source:{failures}\n  Today the refusal is \
         STRUCTURAL — with `default-features = false` the retriever compiles down to a hard \
         `Err`, so no code path can fetch. Installing one converts that structural guarantee into \
         a POLICY we hope holds, reachable from any schema an untrusted tool author supplies."
    );
}

/// Why a `jsonschema` validator construction site is accounted for.
enum ValidatorDisposition {
    /// The v2 arm: explicitly pinned to Draft 2020-12 (D-02 / SCHM-01).
    PinnedByPolicy,
    /// The v1 arm: frozen at today's auto-detecting behaviour (D-01).
    EraFrozenV1,
    /// Not the MCP `outputSchema` seam at all — a recorded, deferred exception.
    OutOfScopeAllowlisted,
}

/// One allowlisted validator construction site.
struct ValidatorSite {
    file: &'static str,
    function: &'static str,
    hits: usize,
    disposition: ValidatorDisposition,
    why: &'static str,
}

/// Every `validator_for(` / `draft202012::` construction site in the workspace.
///
/// The `hits` count is part of the entry on purpose: a SECOND construction added
/// inside an already-allowlisted function is exactly the shape a regression
/// takes, and a file-level or function-level presence check cannot see it.
const VALIDATOR_SITES: &[ValidatorSite] = &[
    ValidatorSite {
        file: "src/server/output_validation.rs",
        function: "compile_2020_12",
        hits: 1,
        disposition: ValidatorDisposition::PinnedByPolicy,
        why: "The v2 arm. MCP 2026-07-28 pins outputSchema to JSON Schema Draft 2020-12, so this \
              site must construct through `draft202012::new` on a document whose `$schema` has \
              been normalized first — never through the auto-detecting `validator_for`, which \
              would silently honour a tool author's declared dialect instead of the spec's.",
    },
    ValidatorSite {
        file: "src/server/output_validation.rs",
        function: "compile_for_era",
        hits: 1,
        disposition: ValidatorDisposition::EraFrozenV1,
        why: "The v1 arm, deliberately frozen by D-01 at today's behaviour: the dialect is \
              auto-detected from the document's own `$schema` declaration. Changing this site \
              would alter validation outcomes for every existing 2025-11-25 server, which is a \
              breaking change this phase explicitly declined to make.",
    },
    ValidatorSite {
        file: "crates/pmcp-agent/src/iteration/decide.rs",
        function: "evaluate_submit_result",
        hits: 1,
        disposition: ValidatorDisposition::OutOfScopeAllowlisted,
        why: "Out of scope, recorded rather than fixed. This validates an AGENT's submit-result \
              payload against a caller-supplied schema; it is not the MCP outputSchema seam. \
              SCHM-01 scopes to the server output-validation path, and pinning the draft here \
              would be a behaviour change to a different surface with its own users, so it is \
              booked as a deferred item instead of being changed inside a schema-pinning phase.",
    },
];

/// Every validator construction site, grouped by file and enclosing function.
fn validator_sites() -> BTreeMap<(String, String), Vec<u32>> {
    let mut out: BTreeMap<(String, String), Vec<u32>> = BTreeMap::new();
    for path in workspace_rs_files() {
        let raw = fs::read_to_string(&path).expect("readable source");
        if !VALIDATOR_NEEDLES.iter().any(|needle| raw.contains(needle)) {
            continue;
        }
        let stripped = strip(&raw);
        let spans = fn_spans(&stripped);
        let excluded = cfg_test_spans(&stripped);
        for needle in VALIDATOR_NEEDLES {
            for at in token_hits(&stripped.text, needle) {
                if is_excluded(&excluded, at) {
                    continue;
                }
                let owner = enclosing_fn(&spans, at)
                    .unwrap_or("<file scope>")
                    .to_string();
                out.entry((rel(&path), owner))
                    .or_default()
                    .push(line_of(&stripped, at));
            }
        }
    }
    out
}

/// The validator construction population equals the allowlist, count by count.
#[test]
fn v2_schema_tripwires_validator_construction_sites_are_accounted_for() {
    let observed = validator_sites();
    let mut failures = String::new();

    for ((file, function), lines) in &observed {
        let Some(entry) = VALIDATOR_SITES
            .iter()
            .find(|site| site.file == file && site.function == function)
        else {
            let _ = writeln!(
                failures,
                "\n  UNKNOWN validator construction site: `{function}` in {file} at line(s) \
                 {lines:?}.\n    Every site that builds a `{JSONSCHEMA}` validator has to state \
                 which dialect policy it is under: pinned to 2020-12 (v2), frozen at \
                 auto-detection (v1), or out of scope with a written reason."
            );
            continue;
        };
        if entry.hits != lines.len() {
            let _ = writeln!(
                failures,
                "\n  COUNT CHANGED: `{function}` in {file} was recorded with {} construction \
                 site(s) and now has {} at line(s) {lines:?}.\n    A second construction inside \
                 an already-allowlisted function is exactly the shape this regression takes; \
                 re-derive the entry rather than raising the number.",
                entry.hits,
                lines.len()
            );
        }
    }

    for site in VALIDATOR_SITES {
        let key = (site.file.to_string(), site.function.to_string());
        if !observed.contains_key(&key) {
            let _ = writeln!(
                failures,
                "\n  STALE entry: `{}` in {} no longer constructs a validator. Delete the entry.",
                site.function, site.file
            );
        }
    }

    assert!(
        failures.is_empty(),
        "the `{JSONSCHEMA}` validator construction population changed:{failures}"
    );

    // The v2 arm must construct through the PINNED constructor, and the v1 arm
    // through the auto-detecting one. A straight swap keeps the counts identical.
    let pinned = read(OUTPUT_VALIDATION);
    let stripped = strip(&pinned);
    let spans = fn_spans(&stripped);
    for site in VALIDATOR_SITES {
        let needle = match site.disposition {
            ValidatorDisposition::PinnedByPolicy => "draft202012",
            ValidatorDisposition::EraFrozenV1 => "validator_for",
            ValidatorDisposition::OutOfScopeAllowlisted => continue,
        };
        let Some((_, span)) = spans.iter().find(|(name, _)| name == site.function) else {
            panic!(
                "`{}` must still exist in {}; the dialect-policy check cannot run over a \
                 function that is gone",
                site.function, site.file
            )
        };
        let body = &stripped.text[span.clone()];
        assert!(
            !token_hits(body, needle).is_empty(),
            "`{}` no longer constructs through `{needle}`. The dialect policy of that arm was \
             swapped while the site COUNT stayed the same, which the population check above \
             cannot see.",
            site.function
        );
    }

    assert_justifications(
        "VALIDATOR_SITES",
        &VALIDATOR_SITES
            .iter()
            .map(|site| (site.function, site.why))
            .collect::<Vec<_>>(),
    );
}

/// ANTI-VACUITY for the source scans — a green run must mean "checked".
#[test]
fn v2_schema_tripwires_the_source_scan_is_not_vacuous() {
    assert!(
        src_files().len() > 50,
        "the `src/` walk collapsed; every source check here would pass over nothing"
    );
    assert!(
        workspace_rs_files().len() > 300,
        "the workspace walk collapsed; the retriever and validator scans would pass over nothing"
    );

    let observed = validator_sites();
    assert!(
        !observed.is_empty(),
        "the validator construction scan found NO site at all; the allowlist check would then \
         iterate over an empty map and pass"
    );

    let source = read(OUTPUT_VALIDATION);
    for needle in VALIDATOR_NEEDLES {
        assert!(
            source.contains(needle),
            "{OUTPUT_VALIDATION} no longer mentions `{needle}`; the era-keyed compilation this \
             file fences has moved and the scan is looking in the wrong place"
        );
    }

    assert!(
        !VALIDATOR_SITES.is_empty(),
        "VALIDATOR_SITES is empty, so the population check passes over nothing"
    );
    assert!(
        !RETRIEVER_NEEDLES.is_empty() && !RESOLVER_FEATURES.is_empty(),
        "the retriever and resolver-feature needle lists must be non-empty or their scans are \
         no-ops"
    );
}

// ===========================================================================
// 5. TASK 2 — D-12: the caching hints have exactly ONE writer.
// ===========================================================================

/// The `cfg`-free module that owns the single projection point.
const CACHING: &str = "src/types/caching.rs";

/// The task wire types, which carry a DIFFERENT `ttlMs` (D-10).
const TASKS_TYPES: &str = "src/types/tasks.rs";

/// The one function allowed to write the caching-hint wire keys.
const PROJECTOR: &str = "project_caching_hints";

/// The native chokepoint module.
const CORE: &str = "src/server/core.rs";

/// The `wasm32`-only dispatcher no native gate compiles.
const WASM_SERVER: &str = "src/server/wasm_server.rs";

/// The local helper `wasm_server.rs` routes every cacheable result through.
const WASM_HELPER: &str = "cacheable_result_to_value";

/// The shared v2 result-envelope helper.
const ENVELOPE: &str = "inject_v2_result_envelope";

/// The response-middleware entry point that runs AFTER the projection.
const MIDDLEWARE: &str = "process_response_with_context";

/// The two caching-hint WIRE keys.
const HINT_KEYS: &[&str] = &["ttlMs", "cacheScope"];

/// The two caching-hint RUST field declarations.
const HINT_FIELDS: &[&str] = &["pub ttl_ms:", "pub cache_scope:"];

/// The modules declaring the six `CacheableResult` extenders.
const RESULT_MODULES: &[&str] = &[
    "src/types/tools.rs",
    "src/types/resources.rs",
    "src/types/prompts.rs",
    "src/types/protocol/mod.rs",
];

/// The six results that extend `CacheableResult` in the `2026-07-28` schema.
const CACHEABLE_RESULT_TYPES: &[&str] = &[
    "ListToolsResult",
    "ListResourcesResult",
    "ListResourceTemplatesResult",
    "ReadResourceResult",
    "ListPromptsResult",
    "ServerDiscoverResult",
];

/// Method names that WRITE a key into a `serde_json` object.
///
/// Reads (`get`, `contains_key`) are deliberately absent: a test asserting the
/// key's ABSENCE is not a projection, and folding reads in would make the check
/// fire on every assertion in the tree.
const WRITE_CALLS: &[&str] = &[
    "insert",
    "entry",
    "or_insert",
    "or_insert_with",
    "remove",
    "swap_remove",
    "shift_remove",
    "append",
];

/// The D-12 rationale every single-projection failure message points at.
const D_12_WHY: &str = "\
D-12: the era projection happens at ONE shared point, so there is one place to test and one place \
to rot. A per-result-type or per-dispatcher projection is exactly what this test exists to \
prevent: it multiplies the number of sites that must agree about a v1 wire carrying no v2 key \
(D-11), and every one of them is a place the agreement can quietly lapse.\n  \
That one place is `src/types/caching.rs` and NOT `src/server/core.rs` for a structural reason: \
`core.rs` is `#[cfg(not(target_arch = \"wasm32\"))]` while `wasm_server.rs` is \
`#[cfg(target_arch = \"wasm32\")]`, so those two cfg sets are DISJOINT and a projector living in \
either server module would be unreachable from the other — leaving the wasm dispatcher with no \
strip at all. Do not \"simplify\" the projector back into a server module.";

/// One site that writes a caching-hint wire key.
struct HintWrite {
    file: String,
    line: u32,
    key: String,
    function: String,
}

/// Is the literal at `at` in a position that WRITES the key into a JSON object?
///
/// Two shapes count: a write METHOD call (`obj.insert("ttlMs", …)`,
/// `obj.entry("ttlMs")`, `obj.remove("ttlMs")`) and an index ASSIGNMENT
/// (`value["ttlMs"] = …`). A read (`value.get("ttlMs")`, `value["ttlMs"]` in a
/// comparison) is not a projection and does not count.
fn is_write_position(text: &str, at: usize, needle_len: usize) -> bool {
    let prefix = text[..at].trim_end();
    if let Some(head) = prefix.strip_suffix('(') {
        let reversed: String = head
            .chars()
            .rev()
            .take_while(|c| is_ident_char(*c))
            .collect();
        let name: String = reversed.chars().rev().collect();
        if WRITE_CALLS.contains(&name.as_str()) {
            return true;
        }
    }
    if prefix.ends_with('[') {
        let rest = text[at + needle_len..].trim_start();
        if let Some(after) = rest.strip_prefix(']') {
            let after = after.trim_start();
            return after.starts_with('=') && !after.starts_with("==");
        }
    }
    false
}

/// Every site in `src/` that WRITES a caching-hint wire key, `cfg(test)` aside.
fn hint_write_sites() -> Vec<HintWrite> {
    let mut out = Vec::new();
    for path in src_files() {
        let raw = fs::read_to_string(&path).expect("readable source");
        if !HINT_KEYS.iter().any(|key| raw.contains(key)) {
            continue;
        }
        let stripped = strip_keeping_literals(&raw);
        let spans = fn_spans(&stripped);
        let excluded = cfg_test_spans(&stripped);
        for key in HINT_KEYS {
            let quoted = format!("\"{key}\"");
            for at in occurrences(&stripped.text, &quoted) {
                if is_excluded(&excluded, at)
                    || !is_write_position(&stripped.text, at, quoted.len())
                {
                    continue;
                }
                out.push(HintWrite {
                    file: rel(&path),
                    line: line_of(&stripped, at),
                    key: (*key).to_string(),
                    function: enclosing_fn(&spans, at)
                        .unwrap_or("<file scope>")
                        .to_string(),
                });
            }
        }
    }
    out
}

/// The caching-hint wire keys are written in exactly one function.
#[test]
fn v2_schema_tripwires_caching_hints_are_written_in_exactly_one_place() {
    let sites = hint_write_sites();
    let mut failures = String::new();

    for site in &sites {
        if site.file != CACHING || site.function != PROJECTOR {
            let _ = writeln!(
                failures,
                "\n  OUT-OF-PLACE hint write: `\"{}\"` written by `{}` at {}:{}",
                site.key, site.function, site.file, site.line
            );
        }
    }

    assert!(
        failures.is_empty(),
        "{D_12_WHY}\n  The only permitted writer is `{PROJECTOR}` in {CACHING}.{failures}"
    );
}

/// One `ttl_ms` / `cache_scope` field declaration with its serde attributes.
struct HintField {
    module: &'static str,
    line: u32,
    field: &'static str,
    attrs: Vec<String>,
}

/// Every caching-hint FIELD declaration in the six result types' modules.
fn hint_field_attributes() -> Vec<HintField> {
    let mut out = Vec::new();
    for module in RESULT_MODULES {
        let source = read(module);
        let stripped = strip_keeping_literals(&source);
        for field in HINT_FIELDS {
            for at in occurrences(&stripped.text, field) {
                out.push(HintField {
                    module,
                    line: line_of(&stripped, at),
                    field,
                    attrs: attrs_before(&stripped.text, at),
                });
            }
        }
    }
    out
}

/// No result type may perform the era projection as a serde change.
///
/// The anti-pattern is named in `project_capabilities_for_v1`'s rustdoc in
/// `src/server/core.rs`: doing an era projection in the TYPES module — a
/// `serialize_with`, a bespoke `skip_serializing_if`, a hand-written `Serialize`
/// impl — changes the wire of every existing server on EVERY era, because a
/// serde attribute has no idea which era it is serializing for. The projection
/// has to happen where the era is known, which is the dispatcher chokepoint.
#[test]
fn v2_schema_tripwires_no_result_type_projects_independently() {
    let fields = hint_field_attributes();
    let mut failures = String::new();

    for field in &fields {
        if field.attrs.is_empty() {
            let _ = writeln!(
                failures,
                "\n  NO ATTRIBUTE: `{}` at {}:{} carries no serde attribute at all; it can no \
                 longer be omitted when unset, so a v1 wire would gain a `null`-valued v2 key",
                field.field, field.module, field.line
            );
        }
        for attr in &field.attrs {
            if attr.contains("serialize_with") || attr.contains("deserialize_with") {
                let _ = writeln!(
                    failures,
                    "\n  CUSTOM SERDE: `{}` at {}:{} carries `{attr}`",
                    field.field, field.module, field.line
                );
            }
            if let Some(value) = attr_value(attr, "skip_serializing_if") {
                if value != "Option::is_none" {
                    let _ = writeln!(
                        failures,
                        "\n  BESPOKE SKIP: `{}` at {}:{} skips on `{value}`, not \
                         `Option::is_none`",
                        field.field, field.module, field.line
                    );
                }
            }
        }
    }

    for module in RESULT_MODULES {
        let stripped = strip(&read(module));
        for ty in CACHEABLE_RESULT_TYPES {
            for trait_name in ["Serialize", "Deserialize"] {
                let needle = format!("{trait_name} for {ty}");
                if stripped.text.contains(&needle) {
                    let _ = writeln!(
                        failures,
                        "\n  HAND-WRITTEN IMPL: `impl {needle}` in {module}; the six \
                         `CacheableResult` extenders must serialize BY DERIVE so the only thing \
                         deciding the wire is the projection"
                    );
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{D_12_WHY}\n  A per-type serde projection alters the wire of every existing server on \
         EVERY era, because a serde attribute cannot see the negotiated era. See \
         `project_capabilities_for_v1`'s rustdoc in {CORE}, which names this anti-pattern \
         directly.{failures}"
    );
}

/// Why a `serde_json::to_value(` site in the wasm dispatcher is accounted for.
enum WasmDisposition {
    /// The site that serializes a `CacheableResult` and strips the hints.
    RoutesThroughProjector,
    /// The site serializes a result that does not extend `CacheableResult`.
    NotCacheable,
}

/// One allowlisted serialization site in [`WASM_SERVER`].
struct WasmSite {
    function: &'static str,
    hits: usize,
    disposition: WasmDisposition,
    why: &'static str,
}

/// Every `serde_json::to_value(` site in the wasm dispatcher, by function.
const WASM_SERIALIZATION_SITES: &[WasmSite] = &[
    WasmSite {
        function: "cacheable_result_to_value",
        hits: 1,
        disposition: WasmDisposition::RoutesThroughProjector,
        why: "The single stripping serializer. Every cacheable result in this era-less dispatcher \
              is funnelled through here so the strip is applied once rather than per handler, and \
              it passes `None` as the era, which selects the projector's STRIP arm.",
    },
    WasmSite {
        function: "handle_initialize",
        hits: 1,
        disposition: WasmDisposition::NotCacheable,
        why: "`InitializeResult` does not extend `CacheableResult` in the 2026-07-28 schema — the \
              sixth cacheable result is `DiscoverResult`, which this dispatcher does not serve at \
              all — so there is no hint to strip and the projection would be a no-op.",
    },
    WasmSite {
        function: "handle_call_tool",
        hits: 2,
        disposition: WasmDisposition::NotCacheable,
        why: "Both arms serialize a `CallToolResult`, which carries no caching hints: a tool call \
              is not a list or read. Hit 1 is the success arm, hit 2 the rejection/error arm, and \
              neither can acquire a hint because the type has no slot for one.",
    },
    WasmSite {
        function: "handle_get_prompt",
        hits: 1,
        disposition: WasmDisposition::NotCacheable,
        why: "`GetPromptResult` is not a `CacheableResult` extender; only `ListPromptsResult` is, \
              and that handler routes through the stripping serializer. Rendering one prompt is \
              not an enumeration, so it carries no freshness hint.",
    },
];

/// The wasm handlers that MUST route through the stripping serializer.
const WASM_CACHEABLE_HANDLERS: &[&str] = &[
    "handle_list_tools",
    "handle_list_resources",
    "handle_read_resource",
    "handle_list_prompts",
];

/// The reason the wasm source assertion is load-bearing rather than redundant.
const WASM_WHY: &str = "\
`src/server/wasm_server.rs` is `#[cfg(target_arch = \"wasm32\")]` (`src/server/mod.rs`), so NO \
native build and NO native test compiles it, and its own \
`#[cfg(all(test, target_arch = \"wasm32\"))]` module does not compile at all. `make wasm-build` \
compiles it — but 115-06 MEASURED that deleting the `project_caching_hints` call still leaves \
`make wasm-build` at exit 0, because the call is a statement whose removal breaks nothing.\n  \
This SOURCE assertion is therefore the ONLY automated gate in the repository that catches the \
removal. Deleting this test silently re-opens a D-11 v1 leak: `WasmResource::read` returns a \
handler-constructed `ReadResourceResult` that this file serializes verbatim, so a handler calling \
`with_cache_scope(CacheScope::Public)` would put a v2-only key straight onto an era-less v1 wire \
(T-115-36).";

/// Every `serde_json::to_value(` site in [`WASM_SERVER`], by enclosing function.
fn wasm_serialization_sites() -> BTreeMap<String, Vec<u32>> {
    let source = read(WASM_SERVER);
    let stripped = strip(&source);
    let spans = fn_spans(&stripped);
    let excluded = cfg_test_spans(&stripped);
    let mut out: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for at in occurrences(&stripped.text, "serde_json::to_value(") {
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

/// Compare the observed wasm serialization sites against the allowlist.
fn wasm_site_population_failures(observed: &BTreeMap<String, Vec<u32>>) -> String {
    let mut failures = String::new();
    for (function, lines) in observed {
        let Some(entry) = WASM_SERIALIZATION_SITES
            .iter()
            .find(|site| site.function == function)
        else {
            let _ = writeln!(
                failures,
                "\n  UNLISTED serialization site: `{function}` at line(s) {lines:?}.\n    Either \
                 the result it serializes extends `CacheableResult` — in which case it MUST route \
                 through `{WASM_HELPER}` — or it does not, in which case say which type it is and \
                 why."
            );
            continue;
        };
        if entry.hits != lines.len() {
            let _ = writeln!(
                failures,
                "\n  COUNT CHANGED: `{function}` was recorded with {} serialization site(s) and \
                 now has {} at line(s) {lines:?}.\n    A second serialization inside an \
                 already-allowlisted function is exactly the shape this regression takes.",
                entry.hits,
                lines.len()
            );
        }
    }
    for site in WASM_SERIALIZATION_SITES {
        if !observed.contains_key(site.function) {
            let _ = writeln!(
                failures,
                "\n  STALE entry: `{}` no longer serializes anything. Delete the entry.",
                site.function
            );
        }
    }
    failures
}

/// Every cacheable serialization in the wasm dispatcher routes through the
/// projector — the only gate that can catch the strip call's removal.
#[test]
fn v2_schema_tripwires_every_cacheable_serialization_site_routes_through_the_projector() {
    let source = read(WASM_SERVER);
    let stripped = strip(&source);
    let excluded = cfg_test_spans(&stripped);

    let references_projector = token_hits(&stripped.text, PROJECTOR)
        .into_iter()
        .any(|at| !is_excluded(&excluded, at));
    assert!(
        references_projector,
        "{WASM_WHY}\n  {WASM_SERVER} no longer references `{PROJECTOR}` at all."
    );

    let observed = wasm_serialization_sites();
    let mut failures = wasm_site_population_failures(&observed);

    for site in WASM_SERIALIZATION_SITES {
        if !matches!(site.disposition, WasmDisposition::RoutesThroughProjector) {
            continue;
        }
        let routes = fn_body(&stripped, site.function)
            .is_some_and(|body| !token_hits(body, PROJECTOR).is_empty());
        if !routes {
            let _ = writeln!(
                failures,
                "\n  STRIP GONE: `{}` no longer calls `{PROJECTOR}`.",
                site.function
            );
        }
    }

    for handler in WASM_CACHEABLE_HANDLERS {
        let Some(body) = fn_body(&stripped, handler) else {
            let _ = writeln!(
                failures,
                "\n  HANDLER GONE: `{handler}` is no longer in {WASM_SERVER}; if it moved, the \
                 new site has to be fenced deliberately."
            );
            continue;
        };
        if token_hits(body, WASM_HELPER).is_empty() {
            let _ = writeln!(
                failures,
                "\n  BYPASS: `{handler}` serves a `CacheableResult` and no longer routes through \
                 `{WASM_HELPER}`."
            );
        }
        if body.contains("serde_json::to_value(") {
            let _ = writeln!(
                failures,
                "\n  DIRECT SERIALIZATION: `{handler}` calls `serde_json::to_value` itself \
                 instead of routing through `{WASM_HELPER}`."
            );
        }
    }

    // A NEW handler that constructs one of the six cacheable results and
    // serializes it directly is the same defect arrived at from a fresh site.
    for (name, span) in fn_spans(&stripped) {
        let body = &stripped.text[span];
        let names_cacheable = CACHEABLE_RESULT_TYPES
            .iter()
            .any(|ty| !token_hits(body, ty).is_empty());
        if names_cacheable
            && body.contains("serde_json::to_value(")
            && token_hits(body, WASM_HELPER).is_empty()
            && token_hits(body, PROJECTOR).is_empty()
        {
            let _ = writeln!(
                failures,
                "\n  UNROUTED CACHEABLE: `{name}` constructs a `CacheableResult` extender and \
                 serializes it without the projector."
            );
        }
    }

    assert!(failures.is_empty(), "{WASM_WHY}{failures}");

    assert_justifications(
        "WASM_SERIALIZATION_SITES",
        &WASM_SERIALIZATION_SITES
            .iter()
            .map(|site| (site.function, site.why))
            .collect::<Vec<_>>(),
    );
}

/// One allowlisted production call of [`ENVELOPE`].
struct EnvelopeSite {
    file: &'static str,
    function: &'static str,
    hits: usize,
    why: &'static str,
}

/// Every PRODUCTION call site of the shared v2 result envelope.
///
/// The plan's text said four; the MEASURED population on 2026-08-01 is SIX —
/// 115-06 already recorded that the original map missed
/// `streamable_http_server.rs` and `testing/mod.rs`. The list below is the
/// measurement, not the prediction.
const ENVELOPE_SITES: &[EnvelopeSite] = &[
    EnvelopeSite {
        file: "src/server/core.rs",
        function: "build_discover_response",
        hits: 1,
        why: "`server/discover` is the MEASURED sixth `CacheableResult` extender and the FIRST \
              call a v2 client makes, so it names itself cacheable; it mints no reserved MRTR or \
              tasks field, so it owns none of them.",
    },
    EnvelopeSite {
        file: "src/server/core.rs",
        function: "build_skills_list_response",
        hits: 1,
        why: "The SEP-2640 `skills/list` egress (Phase 125). Like `server/discover` beside it, it \
              rides the crate-private `InternalClientRequest` route and therefore never reaches \
              `request_is_cacheable`, so it names `Cacheable::Yes` HERE — the extension says a \
              2026-07-28 `skills/list` result carries the base protocol's list-caching \
              attributes. It mints no reserved MRTR or tasks field, so it owns none of them, and \
              it is a complete listing rather than a task, so its disposition is `Complete`.",
    },
    EnvelopeSite {
        file: "src/server/core.rs",
        function: "build_skills_get_response",
        hits: 1,
        why: "The SEP-2640 `skills/get` egress (Phase 125 plan 02). It rides the same \
              crate-private `InternalClientRequest` route as its `skills/list` sibling and so \
              never reaches `request_is_cacheable` either — but it names `Cacheable::No`, not \
              `Yes`. The extension gives `skills/list` the base protocol's list-caching \
              attributes EXPLICITLY and leaves the equivalent question OPEN for `skills/get`, so \
              pmcp claims nothing and the result carries neither `ttlMs` nor `cacheScope` on \
              either era. The two entries differing on exactly this argument is the point: it is \
              a decision with a stated reason, not an omission. It mints no reserved MRTR or \
              tasks field, so it owns none of them, and a single fetched entry is complete rather \
              than a task, so its disposition is `Complete`.",
    },
    EnvelopeSite {
        file: "src/server/core.rs",
        function: "handle_request",
        hits: 1,
        why: "The native chokepoint. Every dispatched result passes through here exactly once, \
              which is why the cacheability decision is taken by the shared `request_is_cacheable` \
              classifier before the request is moved rather than re-derived per method.",
    },
    EnvelopeSite {
        file: "src/server/mod.rs",
        function: "handle_tasks_update",
        hits: 1,
        why: "The `tasks/update` egress path, which mints a reserved tasks field and therefore \
              names an owner other than `None`. A task update is not a list or read, so it names \
              itself NOT cacheable.",
    },
    EnvelopeSite {
        file: "src/server/mod.rs",
        function: "handle_request_with_context",
        hits: 1,
        why:
            "The `Server` twin of the `ServerCore` chokepoint. It exists because `Server` has its \
              own dispatch loop; routing it through the SAME shared helper is what stops the two \
              native dispatchers from drifting apart on the envelope.",
    },
    EnvelopeSite {
        file: "src/server/streamable_http_server.rs",
        function: "listen_terminal_result_frame",
        hits: 1,
        why: "The SSE terminal-result frame, which is a second egress for an already-dispatched \
              result and would otherwise reach the wire without the envelope the same result \
              carries over a plain POST.",
    },
    EnvelopeSite {
        file: "src/testing/mod.rs",
        function: "run_envelope",
        hits: 1,
        why:
            "The `testing` feature's harness seam, shipped rather than `cfg(test)`, so integration \
              tests exercise the REAL helper instead of a re-implementation that could agree with \
              a bug. Feature-gated behind `testing`, folded into `full`.",
    },
];

/// Every production call of [`ENVELOPE`], grouped by file and function.
fn envelope_call_sites() -> BTreeMap<(String, String), Vec<u32>> {
    let mut out: BTreeMap<(String, String), Vec<u32>> = BTreeMap::new();
    for path in src_files() {
        let raw = fs::read_to_string(&path).expect("readable source");
        if !raw.contains(ENVELOPE) {
            continue;
        }
        let stripped = strip(&raw);
        let spans = fn_spans(&stripped);
        let excluded = cfg_test_spans(&stripped);
        for at in token_hits(&stripped.text, ENVELOPE) {
            if is_excluded(&excluded, at) || !is_call_of(&stripped.text, at, ENVELOPE) {
                continue;
            }
            let owner = enclosing_fn(&spans, at)
                .unwrap_or("<file scope>")
                .to_string();
            out.entry((rel(&path), owner))
                .or_default()
                .push(line_of(&stripped, at));
        }
    }
    out
}

/// Is the token at `at` a CALL of `name` rather than its definition?
fn is_call_of(text: &str, at: usize, name: &str) -> bool {
    if !text[at + name.len()..].starts_with('(') {
        return false;
    }
    let prefix = text[..at].trim_end();
    let reversed: String = prefix
        .chars()
        .rev()
        .take_while(|c| is_ident_char(*c))
        .collect();
    let last: String = reversed.chars().rev().collect();
    last != "fn"
}

/// Every envelope call site names its cacheability deliberately.
#[test]
fn v2_schema_tripwires_every_envelope_call_site_names_its_cacheability() {
    let observed = envelope_call_sites();
    let mut failures = String::new();

    for ((file, function), lines) in &observed {
        let Some(entry) = ENVELOPE_SITES
            .iter()
            .find(|site| site.file == file && site.function == function)
        else {
            let _ = writeln!(
                failures,
                "\n  UNLISTED envelope call site: `{function}` in {file} at line(s) {lines:?}.\n  \
                   `{ENVELOPE}` takes `owner` and `cacheable` with NO default precisely so a new \
                 egress has to DECIDE both rather than inherit them by omission. Add the entry \
                 and say what it decided."
            );
            continue;
        };
        if entry.hits != lines.len() {
            let _ = writeln!(
                failures,
                "\n  COUNT CHANGED: `{function}` in {file} was recorded with {} envelope call(s) \
                 and now has {} at line(s) {lines:?}.",
                entry.hits,
                lines.len()
            );
        }
    }

    for site in ENVELOPE_SITES {
        let key = (site.file.to_string(), site.function.to_string());
        if !observed.contains_key(&key) {
            let _ = writeln!(
                failures,
                "\n  STALE entry: `{}` in {} no longer calls `{ENVELOPE}`. Delete the entry.",
                site.function, site.file
            );
        }
    }

    assert!(
        failures.is_empty(),
        "the production `{ENVELOPE}` population changed:{failures}"
    );

    let total: usize = observed.values().map(Vec::len).sum();
    assert_eq!(
        total,
        ENVELOPE_SITES.iter().map(|site| site.hits).sum::<usize>(),
        "the total production envelope call count moved; observed {observed:#?}"
    );

    assert_justifications(
        "ENVELOPE_SITES",
        &ENVELOPE_SITES
            .iter()
            .map(|site| (site.function, site.why))
            .collect::<Vec<_>>(),
    );
}

/// The projection currently runs BEFORE response middleware — a MEASUREMENT of
/// a KNOWN LIMITATION, not an assertion that the ordering is desirable.
///
/// `src/shared/middleware.rs`'s `process_response_with_context` takes
/// `response: &mut JSONRPCResponse`, so a registered response middleware CAN
/// add, alter or remove `ttlMs`, `cacheScope`, `resultType` or `serverInfo`
/// AFTER the projection has run. Three artifacts already state that:
///
/// * `inject_v2_result_envelope`'s rustdoc carries the imperative prohibition
///   ("Response middleware MUST NOT mutate …");
/// * 115-06's
///   `response_middleware_still_runs_after_the_projection_and_this_is_a_known_limitation`
///   MEASURES the behaviour;
/// * 115-10 books it as a deferred item.
///
/// Reordering was CONSIDERED and deliberately NOT done, because moving the
/// injection after the middleware chain would change what middleware observes
/// about Phase 114's `resultType` / `serverInfo` — a v2 behaviour change outside
/// SCHM-03's scope. If a future phase DOES reorder, this test failing is the
/// intended signal that the deferred item was addressed; the remedy is then to
/// update this test, the rustdoc prohibition and the limitation test together,
/// deliberately, rather than to discover the change from a conformance report.
#[test]
fn v2_schema_tripwires_the_projection_precedes_response_middleware_by_measurement() {
    let source = read(CORE);
    let stripped = strip(&source);
    let excluded = cfg_test_spans(&stripped);

    let candidates: Vec<Range<usize>> = fn_spans(&stripped)
        .into_iter()
        .filter(|(name, span)| {
            name == "handle_request"
                && !is_excluded(&excluded, span.start)
                && stripped.text[span.clone()].contains(MIDDLEWARE)
        })
        .map(|(_, span)| span)
        .collect();
    assert_eq!(
        candidates.len(),
        1,
        "expected EXACTLY ONE production `handle_request` in {CORE} running the response \
         middleware chain and found {}; the ordering measurement has no unambiguous subject",
        candidates.len()
    );

    let span = candidates.into_iter().next().expect("checked above");
    let body = &stripped.text[span.clone()];
    let inject = token_hits(body, ENVELOPE)
        .first()
        .copied()
        .unwrap_or_else(|| {
            panic!(
                "`{ENVELOPE}` is no longer called in {CORE}'s `handle_request`; the v2 envelope \
                and the caching projection have left the native chokepoint entirely"
            )
        });
    let middleware = occurrences(body, MIDDLEWARE)
        .first()
        .copied()
        .expect("filtered on its presence above");

    assert!(
        inject < middleware,
        "the caching projection at {CORE}:{} now runs AFTER `{MIDDLEWARE}` at {CORE}:{}.\n  This \
         test pins a KNOWN LIMITATION by measurement: middleware takes `&mut JSONRPCResponse` and \
         can therefore forge or strip `ttlMs` / `cacheScope` / `resultType` / `serverInfo` after \
         the projection. The reorder was CONSIDERED and declined because it changes what \
         middleware observes about Phase 114's `resultType` / `serverInfo`, and it is booked as a \
         DEFERRED ITEM by 115-10.\n  If this reorder is the deferred item finally being \
         addressed, update this test, `{ENVELOPE}`'s rustdoc prohibition and 115-06's \
         `response_middleware_still_runs_after_the_projection_and_this_is_a_known_limitation` \
         TOGETHER. If it is not, put the call back.",
        line_of(&stripped, span.start + inject),
        line_of(&stripped, span.start + middleware)
    );
}

/// ANTI-VACUITY for every projection-side scan — green must mean "checked".
#[test]
fn v2_schema_tripwires_the_projection_scan_is_not_vacuous() {
    let writes = hint_write_sites();
    assert!(
        writes.len() >= 2,
        "the caching-hint write scan found {} site(s); `{PROJECTOR}` ensures BOTH keys on v2 and \
         removes BOTH on every other era, so at least two writes must exist or the location \
         assertion is passing over nothing",
        writes.len()
    );
    for key in HINT_KEYS {
        assert!(
            writes.iter().any(|site| site.key == *key),
            "no write of `\"{key}\"` was found at all; the scan is looking for the wrong string"
        );
    }

    let wasm = wasm_serialization_sites();
    let wasm_total: usize = wasm.values().map(Vec::len).sum();
    assert!(
        wasm_total >= 4,
        "the {WASM_SERVER} serialization scan found {wasm_total} `serde_json::to_value(` site(s); \
         the dispatcher serializes far more than that, so the scan is broken"
    );

    let envelopes = envelope_call_sites();
    for site in ENVELOPE_SITES {
        let key = (site.file.to_string(), site.function.to_string());
        assert!(
            envelopes.contains_key(&key),
            "`{}` in {} was not located by the envelope scan; every entry in the allowlist must \
             correspond to a site the scan actually finds, or the population check is vacuous",
            site.function,
            site.file
        );
    }

    let fields = hint_field_attributes();
    assert_eq!(
        fields.len(),
        12,
        "expected 12 caching-hint field declarations (six `CacheableResult` extenders times two \
         fields) and found {}: {:?}",
        fields.len(),
        fields
            .iter()
            .map(|field| format!("{}:{}", field.module, field.line))
            .collect::<Vec<_>>()
    );
    assert!(
        fields.iter().all(|field| !field.attrs.is_empty()),
        "at least one caching-hint field parsed with NO attributes; the attribute walk is broken \
         and the serde checks would pass over nothing"
    );
}

/// D-10, structurally: the two `ttlMs` definitions stay in separate modules.
///
/// The `ttlMs` in `types::caching` is a cache-FRESHNESS hint (how long a client
/// may reuse a body); the `ttlMs` in `types::tasks` is a task LIFETIME (how long
/// the server retains a record). Copying a long task lifetime into a cache hint
/// would make stale data look fresh, and copying a short cache hint into a task
/// lifetime would expire live work.
///
/// 115-CONTEXT.md leaves the tripwire to the planner's discretion under D-10 and
/// 115-05 exercised that discretion by DECLINING it at the types layer, in
/// favour of reciprocal rustdoc. This is the cheap STRUCTURAL half, added here
/// at the tripwire layer instead: it asserts only that neither module reaches
/// into the other, which is what makes an accidental cross-import impossible
/// rather than merely discouraged.
#[test]
fn v2_schema_tripwires_ttl_ms_definitions_stay_in_separate_modules() {
    let caching = strip(&read(CACHING));
    let tasks = strip(&read(TASKS_TYPES));

    assert!(
        caching.text.len() > 1_000 && tasks.text.len() > 1_000,
        "one of {CACHING} / {TASKS_TYPES} stripped down to almost nothing ({} / {} bytes); this \
         check would then pass over an empty file",
        caching.text.len(),
        tasks.text.len()
    );

    if let Some(at) = occurrences(&caching.text, "crate::types::tasks")
        .first()
        .copied()
    {
        panic!(
            "{CACHING}:{} reaches into `crate::types::tasks`.\n  D-10: the cache-freshness \
             `ttlMs` and the task-lifetime `ttlMs` are deliberately separate. Sharing a constant \
             or a type between them is how a task LIFETIME ends up presented to a client as a \
             cache FRESHNESS window, which makes stale data look fresh.",
            line_of(&caching, at)
        );
    }
    if let Some(at) = occurrences(&tasks.text, "crate::types::caching")
        .first()
        .copied()
    {
        panic!(
            "{TASKS_TYPES}:{} reaches into `crate::types::caching`.\n  D-10: see above — the \
             dependency is forbidden in BOTH directions, because either import is enough to let \
             one meaning of `ttlMs` be substituted for the other.",
            line_of(&tasks, at)
        );
    }
}
