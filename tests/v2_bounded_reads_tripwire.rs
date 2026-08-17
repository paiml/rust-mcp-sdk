//! HTTP-09 / AUTH-03 tripwire — the mechanical half of "every peer-controlled
//! read on the v2 transport path **and on the auth surface** is memory-bounded".
//!
//! # What the requirements say
//!
//! This file has TWO owners. It was written for the first and widened for the
//! second, and it enforces the union of their scopes.
//!
//! > **HTTP-09** (first owner, phase 113.1): Every peer-controlled read on the v2
//! > transport path is memory-bounded. Closure is **enumerable, not narrative**: a
//! > tripwire test asserts that no unbounded whole-body read (`.collect()`,
//! > `read_to_end`) and no unbounded accumulation over peer-supplied bytes exists
//! > in `src/shared/`, `src/client/subscriptions.rs`, or
//! > `src/server/streamable_http_server.rs` outside an explicit reviewed
//! > allowlist, and that no scan over peer-chosen input is worse than O(n).
//!
//! > **AUTH-03 / D-15** (second owner, phase 116): the same property on the OAuth
//! > / OIDC surface — `src/client/auth.rs`, `src/client/oauth.rs`,
//! > `src/server/auth/providers/generic_oidc.rs` and
//! > `src/server/auth/providers/cognito.rs`. Those bodies come from an identity
//! > provider the deployment configured rather than from an arbitrary remote peer,
//! > which is a statement about WHO CHOOSES THE BYTES and not a claim that the
//! > reads are harmless: a compromised, misconfigured or MITM'd IdP endpoint
//! > reaches the same allocation.
//!
//! ## The four auth files entered scope in phase 116, closing D-113-V
//!
//! `D-113-V` (`.planning/phases/113-.../deferred-items.md`) recorded 31
//! reviewed-unbounded whole-body reads across exactly those four files, and
//! recorded that the SCANNER ALREADY FOUND THEM — only the scope fence kept them
//! unreported. Phase 116 bounded every one of those reads (plans 116-06, 116-07,
//! 116-12) and then widened the fence here, in that order: widening first would
//! have left `make quality-gate` red for several waves. The fence is now the thing
//! that keeps them bounded, so a NEW unbounded read added to an auth file fails
//! this suite by name instead of passing unnoticed.
//!
//! This file is the *enumeration* half. The O(n) clause is a separate artifact.
//!
//! # Why it exists
//!
//! "Memory-bounded" started life as a derived success criterion with no closure
//! condition, and it reopened three times. Each round bounded exactly the sites
//! that round's review happened to name; the next review found one it had not.
//! The fixes were real, but "a reviewer must notice" is not a closure condition.
//! This file replaces it with "the suite fails by name".
//!
//! # What each check actually proves — read this before trusting it
//!
//! The two checks have deliberately different strengths, and this file does not
//! claim more than it verifies:
//!
//! * The **whole-body-read check is a structural property check**. It asserts
//!   that every whole-body read in scope is bounded *in its own statement*. It
//!   therefore fails both when a new unbounded read appears AND when an existing
//!   read loses its bound — a site count that never moves cannot hide it.
//! * The **accumulation check is a change detector**, not a proof of
//!   boundedness. Whether appending to a growable buffer is bounded depends on
//!   the drain downstream of it, which no lexical scan can see. So each site is
//!   enumerated with a written justification naming the mechanism that bounds
//!   it, and the check fails when the population changes in either direction.
//!
//! Both checks scan **stripped** source: comments and string/char literal
//! contents are removed before matching, so a doc comment that merely mentions
//! `collect()` in prose is not counted as a site. That stripping is itself unit
//! tested below, because a scanner that over-strips would make every check pass
//! vacuously.
//!
//! # When this file fails
//!
//! Bound the new site, or justify it in the allowlist and have that
//! justification reviewed. Raising a number to match reality is the failure mode
//! this file exists to prevent.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Scope discovery
// ---------------------------------------------------------------------------

/// The directory walked at runtime, so a NEW file cannot escape the scan by
/// nobody remembering to add it here. Losing coverage by omission is exactly
/// how this requirement reopened three times.
const SHARED_DIR: &str = "src/shared";

/// The server-side auth surface, walked for the SAME reason `src/shared/` is.
///
/// Phase 116 first enumerated `providers/generic_oidc.rs` and `providers/cognito.rs`
/// individually, which reproduced by hand exactly the omission this file's other
/// owner had already learned not to risk — and it cost coverage immediately: every
/// file under here holds a `reqwest::Client` and parses a body an identity provider
/// controls, but `jwt.rs` and `jwt_validator.rs` were never scanned, so their
/// unbounded JWKS reads went unreported through the whole of D-113-V.
///
/// Deriving the scope is also CHEAPER than enumerating it here: the accumulation
/// needle count across every other file under this directory is zero, so widening
/// from two named providers to the whole tree adds no `ALLOWLIST` entries at all.
const SERVER_AUTH_DIR: &str = "src/server/auth";

/// The individually-named files this file's two owners put in scope beyond the two
/// walked directories.
///
/// The first two are HTTP-09's. The last two are AUTH-03 / D-15's: they are named
/// rather than walked because `src/client/` as a whole is not the auth surface.
const EXTRA_SCOPE: &[&str] = &[
    "src/client/subscriptions.rs",
    "src/server/streamable_http_server.rs",
    "src/client/auth.rs",
    "src/client/oauth.rs",
];

/// Files whose absence from the discovered scope means discovery is broken.
///
/// Without this, a `read_dir` that silently returned nothing would make every
/// check in this file pass over an empty set.
///
/// **These are FULL RELATIVE PATHS, matched against `rel()`, not base names.**
/// A base name is not a safe key here: nine tracked files in this repo share the
/// base name of `src/client/auth.rs`, and two of them live under `src/` (the
/// other being `src/types/auth.rs`), so a bare base-name entry could be satisfied
/// by the WRONG file entering scope and would report a green guard over coverage
/// that was never established. The pre-existing five entries were converted to
/// full paths in the same edit so the constant has one consistent form.
///
/// A reviewer can check the form mechanically — this returns nothing:
/// `grep -nE '"(auth|oauth|generic_oidc|cognito)\.rs"' tests/v2_bounded_reads_tripwire.rs`
const REQUIRED_FILES: &[&str] = &[
    "src/shared/http.rs",
    "src/shared/sse_parser.rs",
    "src/shared/streamable_http.rs",
    "src/server/streamable_http_server.rs",
    "src/client/subscriptions.rs",
    "src/client/auth.rs",
    "src/client/oauth.rs",
    "src/server/auth/providers/generic_oidc.rs",
    "src/server/auth/providers/cognito.rs",
    "src/server/auth/jwt.rs",
    "src/server/auth/jwt_validator.rs",
];

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

/// Every file HTTP-09 puts in scope, discovered at runtime.
fn scope_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files = Vec::new();
    collect_rs_files(&root.join(SHARED_DIR), &mut files);
    collect_rs_files(&root.join(SERVER_AUTH_DIR), &mut files);
    for extra in EXTRA_SCOPE {
        let path = root.join(extra);
        assert!(path.is_file(), "scope file {extra} no longer exists");
        files.push(path);
    }
    files.sort();
    files.dedup();
    assert!(
        !files.is_empty(),
        "scope discovery returned nothing — every check in this file would pass vacuously"
    );
    // Project once, then look up: `rel()` allocates twice per call, and the nested
    // form ran it once per (file x required) pair.
    let discovered: BTreeSet<String> = files.iter().map(|p| rel(p)).collect();
    for required in REQUIRED_FILES {
        assert!(
            discovered.contains(*required),
            "scope discovery lost {required}; discovered: {discovered:?}\n    \
             REQUIRED_FILES holds FULL RELATIVE PATHS. If this fired after an EXTRA_SCOPE edit, a \
             path was dropped or mistyped and its file is no longer scanned — the silent \
             coverage loss this guard exists to catch."
        );
    }
    files
}

// ---------------------------------------------------------------------------
// Source stripping (comments and literal contents removed, line map retained)
// ---------------------------------------------------------------------------

/// Source rendered with whitespace collapsed away, comments removed and every
/// string/char literal's CONTENT removed (delimiters kept, so a call still
/// reads as a call).
///
/// `lines[i]` is the 1-based source line of `text`'s i-th byte, so every match
/// can be reported as `path:line`.
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

/// Source line of the byte at `index` in the stripped text.
fn line_of(stripped: &Stripped, index: usize) -> u32 {
    stripped.lines.get(index).copied().unwrap_or(0)
}

/// A lexical construct whose interior must not be scanned.
struct Construct {
    /// Index one past the construct's last character.
    end: usize,
    /// What to emit in its place (delimiters only, or nothing for comments).
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

/// End of a double-quoted literal, honouring backslash escapes.
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

/// End of an `r"..."` / `r#"..."#` raw string starting at `from` (the `r`).
///
/// `None` when this `r` begins an identifier or a raw identifier (`r#type`)
/// rather than a raw string.
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

/// End of a char literal starting at `from`.
///
/// `None` when the tick opens a LIFETIME (`'a`, `'static`, `'_`) rather than a
/// literal — getting this wrong would swallow the rest of the file.
fn end_of_char_literal(chars: &[char], from: usize) -> Option<usize> {
    let c1 = *chars.get(from + 1)?;
    if c1 == '\\' {
        // The escaped character occupies at least one position, so the closing
        // tick cannot be earlier than `from + 3` (an escaped tick is the tight
        // case).
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

/// Classify the construct beginning at `i`, if any.
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
        // `br#"..."#` — the byte-raw-string prefix, whose `r` is preceded by an
        // identifier character and so would otherwise be missed.
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
/// One pass: comments vanish, literal contents vanish (delimiters survive),
/// whitespace vanishes. Whitespace removal is what lets a rustfmt-broken method
/// chain be matched as a single needle.
fn strip(source: &str) -> Stripped {
    let chars: Vec<char> = source.chars().collect();
    let lines = line_numbers(&chars);
    let mut out = Stripped::default();
    let mut i: usize = 0;
    let mut prev_ident = false;
    while i < chars.len() {
        if let Some(construct) = skip_construct(&chars, i, prev_ident) {
            out.push_delims(construct.delims, lines[i]);
            i = construct.end.max(i + 1);
            prev_ident = false;
            continue;
        }
        let ch = chars[i];
        if ch.is_whitespace() {
            prev_ident = false;
        } else {
            out.push_char(ch, lines[i]);
            prev_ident = is_ident_char(ch);
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// `cfg`-test region exclusion
// ---------------------------------------------------------------------------

/// Index of the delimiter closing the group opened at `open`.
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

/// Split a `cfg` predicate list on top-level commas.
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
///
/// True for `test` itself and for any `all(...)` carrying `test` as a conjunct —
/// which is what makes the streamable HTTP transport's
/// `all(test, not(target_arch = "wasm32"), feature = "streamable-http")` module
/// an excluded region even though a bare `#[cfg(test)]` grep finds nothing in
/// that 108 KB file.
///
/// False when `test` appears only inside an `any(...)`: such an item COMPILES
/// WITHOUT `test` (the `any(feature = "fuzzing", test)` fuzz seam is on in every
/// build that enables that feature), so it ships and must stay in scope.
///
/// Matching is by whole-conjunct equality, so `latest`, `testing` and
/// `test_utils` do not trigger it.
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

/// Span of the item that follows an attribute ending at `from`.
///
/// Skips any further attributes, jumps over balanced groups so a comma inside a
/// parameter list is not mistaken for the end of the item, and ends at either
/// the item's brace-matched body or the `;` / `,` terminating a body-less item.
///
/// Brace matching runs over the ALREADY-STRIPPED text, so a brace inside a
/// string or a comment cannot unbalance it.
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
/// Deliberately per-item rather than "truncate at the first marker": the
/// streamable HTTP server has two `cfg(test)` FUNCTIONS ahead of its test
/// module, and truncating at the first would drop thousands of lines of
/// production code from the scan — the miss-by-omission failure this file
/// exists to prevent.
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

// ---------------------------------------------------------------------------
// Match enumeration
// ---------------------------------------------------------------------------

/// Byte offsets of every occurrence of `needle` in `text`.
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

/// The last `max` characters of `text`, for a failure message that shows the
/// offending statement without dumping a whole file.
fn tail(text: &str, max: usize) -> String {
    let mut chars: Vec<char> = text.chars().rev().take(max).collect();
    chars.reverse();
    chars.into_iter().collect()
}

/// One scanned file: its path, its stripped text and its `cfg(test)`-only
/// regions.
struct ScannedFile {
    path: PathBuf,
    stripped: Stripped,
    excluded: Vec<Range<usize>>,
}

impl ScannedFile {
    fn load(path: PathBuf) -> Self {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let stripped = strip(&source);
        let excluded = cfg_test_spans(&stripped);
        Self {
            path,
            stripped,
            excluded,
        }
    }

    /// Offsets of `needle` outside every `cfg(test)`-only region — that is, the
    /// occurrences that exist in a shipped build.
    fn shipped_hits(&self, needle: &str) -> Vec<usize> {
        occurrences(&self.stripped.text, needle)
            .into_iter()
            .filter(|at| !is_excluded(&self.excluded, *at))
            .collect()
    }

    fn line(&self, index: usize) -> u32 {
        line_of(&self.stripped, index)
    }
}

fn scanned_scope() -> Vec<ScannedFile> {
    scope_files().into_iter().map(ScannedFile::load).collect()
}

// ---------------------------------------------------------------------------
// Whole-body reads — a STRUCTURAL check
// ---------------------------------------------------------------------------

/// The reads that turn a peer-chosen body length into a single allocation.
///
/// * `.collect().await` is `http_body_util::BodyExt::collect`. It is
///   discriminating precisely BECAUSE an iterator `collect` is never awaited, so
///   ordinary iterator code cannot trip it.
/// * `read_to_end` / `read_to_string` are the std and tokio unbounded reads.
/// * `body::to_bytes(` is the axum/hyper one-shot helper. Its limit is a
///   REQUIRED argument, so it is bounded by construction — the only way to
///   unbound it is to pass the maximum, which is what its rule checks for.
/// * `.text().await` / `.bytes().await` / `.json().await` / `.json::<` are
///   `reqwest`'s response-consuming reads. They take NO limit argument, so there
///   is no bounded form of them to recognise and every occurrence needs a
///   reviewed exemption. They are here because the four families above cover
///   hyper, axum, std and tokio but not the HTTP client one shared transport
///   actually uses — the exact "the needles named this round" gap this file
///   exists to close.
const WHOLE_BODY_NEEDLES: &[&str] = &[
    ".collect().await",
    "read_to_end",
    "read_to_string",
    "body::to_bytes(",
    ".text().await",
    ".bytes().await",
    ".json().await",
    ".json::<",
];

/// A reviewed exemption from the whole-body-read rule.
struct Allowed {
    path: &'static str,
    needle: &'static str,
    why: &'static str,
}

/// Whole-body reads that are not bounded in their own statement.
///
/// An entry is a REVIEWED, WRITTEN exemption, never a silent one: `why` must say
/// either what bounds the read or, plainly, that it is unbounded and who owns
/// the fix. Enumerating a gap is the point — an unnamed gap is what reopened
/// this requirement three times.
///
/// This list should shrink, never grow. It is now EMPTY, which is its floor:
/// the last entry — the `reqwest` whole-body read in
/// `OptimizedSseTransport::connect_sse`, which this tripwire itself found — was
/// BOUNDED in plan 113.1-03 rather than re-exempted, and removed by hand.
const WHOLE_BODY_ALLOWLIST: &[Allowed] = &[];

/// The statement a match sits in: everything back to the nearest `;`, `{` or
/// `}`.
///
/// This is the window the bound must appear in. Whitespace is already collapsed,
/// so a rustfmt-broken method chain is one statement here.
fn statement_scope(text: &str, match_start: usize) -> &str {
    let start = text[..match_start]
        .rfind([';', '{', '}'])
        .map_or(0, |at| at + 1);
    &text[start..match_start]
}

/// Whether the statement around a whole-body read bounds it.
///
/// This is a STRUCTURAL check, and that is the whole point: it fails when a NEW
/// unbounded site is added AND when an EXISTING site loses its bound. A
/// count-based check can only see the first, which is how a review that
/// enumerated "the sites this round found" kept missing the next one.
fn bound_in_scope(scope: &str, needle: &str) -> bool {
    match needle {
        ".collect().await" => scope.contains("Limited::new("),
        "read_to_end" | "read_to_string" => scope.contains(".take("),
        "body::to_bytes(" => !scope.contains("usize::MAX"),
        _ => false,
    }
}

fn allowlisted<'a>(list: &'a [Allowed], path: &str, needle: &str) -> Option<&'a Allowed> {
    list.iter()
        .find(|entry| entry.path == path && entry.needle == needle)
}

#[test]
fn no_unbounded_whole_body_read_over_peer_supplied_bytes() {
    let mut violations = String::new();
    for file in scanned_scope() {
        let path = rel(&file.path);
        for needle in WHOLE_BODY_NEEDLES {
            if allowlisted(WHOLE_BODY_ALLOWLIST, &path, needle).is_some() {
                continue;
            }
            for at in file.shipped_hits(needle) {
                let scope = statement_scope(&file.stripped.text, at);
                if bound_in_scope(scope, needle) {
                    continue;
                }
                let _ = writeln!(
                    violations,
                    "\n  {}:{} — unbounded `{}`\n    statement: ...{}",
                    path,
                    file.line(at),
                    needle,
                    tail(scope, 200)
                );
            }
        }
    }
    assert!(
        violations.is_empty(),
        "HTTP-09 / AUTH-03: unbounded whole-body read(s) over peer-supplied bytes:{violations}\n\
         Required action depends on which client the site uses, and the two shapes are NOT \
         interchangeable:\n\
         \u{20} * hyper / axum (src/shared/, src/client/subscriptions.rs, \
         src/server/streamable_http_server.rs): wrap the read in `http_body_util::Limited` with \
         the transport's configured cap, exactly as `collect_body_within_cap` does in \
         src/shared/http.rs and src/shared/streamable_http.rs.\n\
         \u{20} * reqwest (src/client/auth.rs, src/client/oauth.rs, \
         src/server/auth/providers/generic_oidc.rs, src/server/auth/providers/cognito.rs): \
         `Limited` does NOT apply — reqwest exposes no body-limit API and every one of its \
         whole-body reads (`.text()`, `.bytes()`, `.json()`) is unbounded with no bounded form to \
         recognise. Use the `Response::chunk()`-accumulate shape instead: \
         `collect_reqwest_body_within_cap` in src/shared/http_body_cap.rs (116-06), which checks \
         the running total BEFORE each append so an over-cap body is never held whole, then \
         `serde_json::from_slice`. `collect_sse_text_within_cap` in src/shared/sse_optimized.rs \
         is the same shape for text.\n\
         If this site genuinely cannot be bounded, add a WHOLE_BODY_ALLOWLIST entry with a \
         written justification and get it reviewed — that list is EMPTY and that is its floor, so \
         an entry is a decision a human makes on the record, not a way to close a phase. \
         Deleting the needle is not a fix."
    );
}

/// Anti-vacuity guard for the check above.
///
/// If `strip` ever over-strips, or scope discovery regresses, the structural
/// test would pass over an EMPTY set of sites and report success. This test
/// fails in that case: it pins the two known reads and asserts both are
/// classified bounded.
#[test]
fn the_two_known_capped_whole_body_reads_are_found_and_classified_bounded() {
    let mut found: Vec<(String, u32)> = Vec::new();
    for file in scanned_scope() {
        let path = rel(&file.path);
        for at in file.shipped_hits(".collect().await") {
            let scope = statement_scope(&file.stripped.text, at);
            assert!(
                scope.contains("Limited::new("),
                "{}:{} is a shipped whole-body read that is NOT Limited-wrapped",
                path,
                file.line(at)
            );
            found.push((path.clone(), file.line(at)));
        }
    }
    assert_eq!(
        found.len(),
        2,
        "expected exactly the two capped collect_body_within_cap reads in scope, found {found:?} \
         — if this fell to zero the structural check above is passing vacuously; if it rose, a new \
         whole-body read was added and needs review"
    );
    let paths: Vec<&str> = found.iter().map(|(p, _)| p.as_str()).collect();
    assert!(
        paths.contains(&"src/shared/http.rs") && paths.contains(&"src/shared/streamable_http.rs"),
        "the two capped reads should be the two transports' collect_body_within_cap helpers, \
         found {found:?}"
    );
}

/// The exemption mechanism is only worth keeping if an entry must say something.
#[test]
fn every_whole_body_exemption_carries_a_substantive_justification() {
    for entry in WHOLE_BODY_ALLOWLIST {
        assert!(
            entry.why.trim().len() >= MIN_JUSTIFICATION_CHARS,
            "WHOLE_BODY_ALLOWLIST entry {}/{} needs a real justification naming why this read \
             cannot be bounded, not {:?}",
            entry.path,
            entry.needle,
            entry.why
        );
    }
    assert_eq!(
        WHOLE_BODY_ALLOWLIST.len(),
        0,
        "the whole-body exemption list should SHRINK, never grow, and it is now EMPTY — its \
         floor. At the time this file landed it held exactly one entry: the reqwest whole-body \
         read in OptimizedSseTransport that this tripwire itself found. Plan 113.1-03 BOUNDED \
         that read (collect_sse_text_within_cap, a running total against \
         DEFAULT_HTTP_SSE_BUFFERED_BYTES) and deleted the entry, closing HTTP-09 on the merits. \
         ANY entry at all now means a new unbounded read was exempted rather than fixed, and \
         that is a decision a human has to make on the record."
    );
}

/// An exemption shorter than this is a label, not a justification.
const MIN_JUSTIFICATION_CHARS: usize = 40;

// ---------------------------------------------------------------------------
// Accumulations — a CHANGE DETECTOR over a justified allowlist
// ---------------------------------------------------------------------------

/// Ways bytes get appended to a growable buffer.
///
/// `.extend(` does not match inside `extend_from_slice(` — the character after
/// `extend` there is `_`, not `(` — so the two needles never double-count.
const ACCUMULATION_NEEDLES: &[&str] = &["extend_from_slice(", "push_str(", ".extend(", ".append("];

/// One reviewed accumulation site population.
///
/// Keyed by file + needle + exact count rather than by line number on purpose:
/// line numbers churn on every unrelated edit, which would make this a nuisance
/// rather than a gate, while a COUNT change is exactly the event that needs a
/// human to look.
struct Accumulation {
    path: &'static str,
    needle: &'static str,
    count: usize,
    why: &'static str,
}

/// Every accumulation site in scope, with the mechanism that bounds it.
///
/// Whether appending to a growable buffer is bounded depends on the drain
/// downstream of it, which no lexical scan can see — so this list is where the
/// reasoning lives, and the test below only detects that the population moved.
///
/// Counts were MEASURED by running the check with an empty list, not copied from
/// a plan. Lines are recorded in the plan summary; the key deliberately is not a
/// line number, because those churn on every unrelated edit.
const ALLOWLIST: &[Accumulation] = &[
    Accumulation {
        path: "src/client/auth.rs",
        needle: "push_str(",
        count: 2,
        why: "rendered_source_chain walks a &dyn Error source chain, appending \" <- \" and one \
              cause's Display per LINK. The loop runs once per link, not once per byte: the chain \
              is the finite structure reqwest built (its own error, then hyper/io or this crate's \
              redirect refusal), and a reqwest error's Display names the request URL, the error \
              kind and the status — it never carries the response body, which is the only thing \
              in this path whose length an identity provider chooses. Sole production call site \
              is request_failure. The bound is the chain's DEPTH, not a drain downstream; nothing \
              streams into this String.",
    },
    Accumulation {
        path: "src/client/oauth.rs",
        needle: "push_str(",
        count: 1,
        why: "extract_base_url appends a decimal port AFTER Url::parse has already accepted the \
              MCP server URL, so the appended text is a colon plus at most five digits — \
              Url::port() is an Option<u16>, and the `if let Some(port)` guard runs at most once \
              per call with no loop and no incremental reader behind it. This is the same \
              TYPE-bounded shape as normalize_server_key's entry below, reached from a different \
              direction: it formats a parsed authority into a discovery base URL rather than into \
              a credential cache key.",
    },
    Accumulation {
        path: "src/client/subscriptions.rs",
        needle: "extend_from_slice(",
        count: 2,
        why: "The listen stream's reader and its fuzz seam each append ONE hyper frame to a byte \
              buffer and then fully drain it with take_utf8_prefix on the same iteration, so the \
              residual is the incomplete-character tail of at most three bytes. What the decoded \
              text then feeds is bounded by SseParser::feed's unconditional pre-check over \
              retained state PLUS this chunk (113-17), under the 256 KiB MAX_LISTEN_LINE_BYTES \
              listen ceiling.",
    },
    Accumulation {
        path: "src/client/subscriptions.rs",
        needle: ".extend(",
        count: 2,
        why: "These collect the payloads drain_sse_payloads has just COMPLETED from one chunk, \
              not bytes retained across chunks: the stream yields each pending payload and \
              removes it before the next poll, so the population is bounded by what a single \
              hyper frame can complete, and each completed payload was itself admitted under the \
              parser's 256 KiB retained-state bound.",
    },
    Accumulation {
        path: "src/client/subscriptions.rs",
        needle: "push_str(",
        count: 1,
        why: "truncate_frame copies at most MAX_ECHOED_FRAME characters into a String pre-sized \
              to exactly that boundary. It IS a bound rather than a consumer of one: it exists to \
              keep untrusted peer frame bytes out of a client's logs, and it is the only writer \
              of that String.",
    },
    Accumulation {
        path: "src/server/auth/providers/cognito.rs",
        needle: "push_str(",
        count: 5,
        why: "Two mechanisms, both bounded by STRUCTURE rather than by a drain. (a) Three appends \
              in authorization_url build a query string out of AuthorizationParams — a struct the \
              calling server supplies, not bytes read off a socket. The nonce and PKCE-challenge \
              appends each sit under a single `if let` that fires at most once, and the `extra` \
              loop appends exactly once per HashMap entry the caller put there; every value goes \
              through urlencoding::encode, so a value cannot break out of the parameter it is in. \
              If an operator ever forwards a remote client's query map into `extra` verbatim, the \
              bound becomes that map's size, and that is a decision the operator makes. (b) Two \
              appends in rendered_source_chain run once per source() link of a reqwest error \
              whose Display carries the URL, kind and status and never a response body.",
    },
    Accumulation {
        path: "src/server/auth/providers/generic_oidc.rs",
        needle: "push_str(",
        count: 5,
        why: "The same two mechanisms as the Cognito provider, in this file's own code: three \
              appends in authorization_url and two in rendered_source_chain. What differs here is \
              where the URL's PREFIX comes from — this provider starts from the \
              authorization_endpoint of a discovery document, which is IdP-chosen text, and that \
              document is read through collect_reqwest_body_within_cap under \
              DEFAULT_AUTH_RESPONSE_BYTES (116-06), so even the peer-chosen part arrived under a \
              1 MiB ceiling before it was ever appended to. The two caller-supplied appends are \
              single-shot `if let` guards, the `extra` loop is one append per map entry, all \
              urlencoding::encode'd, and the error-chain appends run once per finite source() \
              link. Neither mechanism retains state across calls.",
    },
    Accumulation {
        path: "src/server/streamable_http_server.rs",
        needle: "push_str(",
        count: 2,
        why: "write_sse_frame appends one SSE frame into the caller's String — the frame's \
              serialized payload and its terminator — and render_v2_multi_frame_body is its only \
              caller: it assembles the v2 multi-frame SSE POST response body (CONF-07 / D-16) \
              with one frame per queued progress notification, then exactly one for the terminal \
              result. The loop's length is the length of the vector \
              V2ProgressQueue::drain returned, and THAT is bounded by the queue's own capacity — \
              V2_PROGRESS_QUEUE_CAPACITY (64), a BOUNDED tokio mpsc whose synchronous producer \
              uses try_send and drops on a full queue rather than blocking or growing \
              (T-118.1-12-01). So the bound is the channel's capacity, applied BEFORE these bytes \
              exist, not a drain downstream of them. The per-frame size is bounded independently: \
              a progress notification is a ProgressToken plus two f64s and an Option<String>, all \
              chosen by the SERVER's own handler, never read off a socket — the only \
              peer-supplied component is the progress token, which arrived inside the request \
              body under the transport's max_request_bytes cap. The second append runs exactly \
              once per response and carries the same JSONRPCResponse the JSON path would have \
              serialized whole, so it adds no byte the unchanged path did not already emit. \
              Nothing streams into this String and it is built to completion before axum sees it.",
    },
    Accumulation {
        path: "src/shared/credential_store.rs",
        needle: "push_str(",
        count: 1,
        why: "normalize_server_key appends a decimal port to a key it has just built from an \
              ALREADY-PARSED authority, not from a peer byte stream. Url::port() is an \
              Option<u16>, so the appended text is a colon plus at most five digits, and the \
              branch is a single `if let Some(port)` rather than a loop, so it runs at most once \
              per call. The bound here is the TYPE, not a drain downstream — this String is a \
              cache key, and nothing streams into it.",
    },
    Accumulation {
        path: "src/shared/http.rs",
        needle: "extend_from_slice(",
        count: 1,
        why: "connect_sse's reader appends one hyper frame to `undecoded` and take_utf8_prefix \
              drains it in the same loop iteration, leaving only the incomplete-character tail of \
              at most three bytes. The parser downstream is bounded by 113-17's unconditional \
              retained-plus-chunk pre-check under this transport's configurable \
              DEFAULT_HTTP_SSE_BUFFERED_BYTES ceiling (16 MiB, movable via \
              with_sse_buffered_bytes).",
    },
    Accumulation {
        path: "src/shared/http_body_cap.rs",
        needle: "extend_from_slice(",
        count: 1,
        why: "collect_reqwest_body_within_cap IS a bound rather than a consumer of one. It \
              evaluates `chunk.len() > max_bytes - accumulated.len()` and returns Err BEFORE this \
              append, so an over-cap body is never held whole, and `accumulated.len() <= \
              max_bytes` is the loop invariant that keeps that subtraction from underflowing. \
              All three production call sites (src/client/auth.rs discovery, token and \
              registration reads) pass DEFAULT_AUTH_RESPONSE_BYTES, a 1 MiB ceiling. A declared \
              Content-Length over the cap short-circuits earlier, but that header is ADVISORY \
              only — the delivered-byte total is the authority.",
    },
    Accumulation {
        path: "src/shared/simd_parsing.rs",
        needle: "extend_from_slice(",
        count: 2,
        why: "SimdSseParser::parse_chunk drains every COMPLETE event out of its buffer before \
              returning, so only an unterminated event tail is retained, and current_data \
              accumulates the data lines of that one event. It carries no ceiling of its own, and \
              the honest statement is that nothing feeds it a peer byte stream today: it is an \
              exported standalone utility with zero in-crate callers outside its own test module. \
              Wiring a transport to it requires giving it the same unconditional pre-check \
              SseParser::feed carries.",
    },
    Accumulation {
        path: "src/shared/sse_optimized.rs",
        needle: "extend_from_slice(",
        count: 1,
        why: "collect_sse_text_within_cap appends one reqwest chunk at a time into `accumulated`, \
              and checks the running total against its max_bytes parameter BEFORE each append — \
              `chunk.len() > max_bytes - accumulated.len()` returns Err rather than appending, so \
              an over-cap body is never held whole. The subtraction cannot underflow because \
              `accumulated.len() <= max_bytes` is the loop invariant. The single production call \
              site passes DEFAULT_HTTP_SSE_BUFFERED_BYTES (16 MiB), the same ceiling the sibling \
              reader in src/shared/http.rs uses. A declared Content-Length over the cap is \
              refused before any body byte is read, but that header is an OPTIMISATION only — \
              the delivered-byte total is the authority (T-113-93).",
    },
    Accumulation {
        path: "src/shared/sse_optimized.rs",
        needle: "push_str(",
        count: 1,
        why: "parse_sse_event builds one event out of the BytesMut slice that split_to() has \
              already cut at the event boundary, so this push is bounded by that single event. \
              The function carries allow(dead_code) and has no caller. The LIVE path in this file \
              is connect_sse's response read, which since plan 113.1-03 is bounded by \
              collect_sse_text_within_cap (its own entry above) rather than being an unbounded \
              reqwest whole-body read enumerated in WHOLE_BODY_ALLOWLIST — that list is now EMPTY.",
    },
    Accumulation {
        path: "src/shared/sse_parser.rs",
        needle: "push_str(",
        count: 4,
        why: "take_utf8_prefix's output is bounded by the caller's buffer length — each byte is \
              appended exactly once and the buffer is drained at a single exit point, which is \
              also what makes it linear rather than quadratic. The two parser buffers are bounded \
              by feed's UNCONDITIONAL pre-check over retained state plus this chunk (113-17, \
              T-113-86), and feed is now the parser's ONLY entry point: the unbounded \
              complete-body sibling that used to state its byte cap as a precondition on the \
              caller was DELETED in 118.2-03. Both of the whole-body collects it served — the GET \
              session stream (118.2-01) and the POST response (118.2-03) in streamable_http.rs — \
              are incremental readers under that same pre-check now, so this file carries no \
              bound-bypassing path at all rather than one discharged by a caller's cap.",
    },
    Accumulation {
        path: "src/shared/streamable_http.rs",
        needle: "extend_from_slice(",
        count: 2,
        why: "TWO sites since 118.2-04, and the SECOND is the fuzz seam for the FIRST. Site one: \
              the SSE reader appends ONE hyper frame to a byte buffer and then \
              fully drains it with take_utf8_prefix on the SAME iteration, so the residual is the \
              incomplete-character tail of at most three bytes. ONE site, TWO consumers since \
              118.2-03: the GET session stream and the POST response that answers \
              text/event-stream both go through SseReadState + read_next_sse_frame, so this count \
              did not move when the second whole-body collect became incremental. What the \
              decoded text then feeds \
              is bounded by SseParser::feed's unconditional pre-check over retained state PLUS \
              this chunk (113-17, T-113-86), under the transport's own max_collected_body_bytes \
              ceiling — DEFAULT_MAX_COLLECTED_BODY_BYTES, 16 MiB, user-overridable through \
              with_max_collected_body_bytes. That reuse is Phase 118.2 D-02: the collected-body \
              cap BECOMES the parser bound rather than a second knob, because a new field on the \
              externally-constructible StreamableHttpTransportConfig would be a MAJOR semver \
              event. Overflow ends the stream with a named error; it never keeps parsing. Site \
              two: decode_sse_chunks_for_fuzz replays that EXACT sequence over a caller-supplied \
              chunk list, under a caller-supplied bound, on the same SseParser and the same \
              take_utf8_prefix. It is #[cfg(any(feature = \"fuzzing\", test))] and so absent from \
              default and full builds, and its whole purpose is that a campaign ASSERTS the bound \
              this justification claims — fuzz/fuzz_targets/streamable_sse_frames.rs checks \
              buffered_bytes() <= max_buffer_size and an undecoded tail of at most 3 bytes after \
              EVERY chunk. A second append that MEASURES the first one's bound is the one kind of \
              new accumulation site that makes this entry stronger rather than weaker.",
    },
    Accumulation {
        path: "src/shared/streamable_http.rs",
        needle: ".extend(",
        count: 2,
        why: "TWO sites since 118.2-04, and the SECOND is the fuzz seam for the FIRST. Site one \
              collects the events drain_sse_events has just COMPLETED from one chunk, not \
              bytes retained across chunks: the reader task pops every pending event and delivers \
              it before it polls the body again, so the population is bounded by what a single \
              hyper frame can complete, and each completed event was itself admitted under the \
              parser's retained-state bound. Same mechanism as the src/client/subscriptions.rs \
              entry above, on this transport's two SSE streams — the GET session stream and the \
              POST response — rather than on subscriptions/listen. One drain site, two \
              consumers. Site two is decode_sse_chunks_for_fuzz collecting the SAME \
              drain_sse_events output into its outcome vector, bounded per chunk by the same \
              parser and unreachable outside a fuzzing or test build (its cfg is \
              any(feature = \"fuzzing\", test), so neither default nor full compiles it).",
    },
    Accumulation {
        path: "src/shared/streamable_http.rs",
        needle: "push_str(",
        count: 1,
        why: "truncate_sse_frame copies at most MAX_ECHOED_SSE_FRAME (200) characters into a \
              String pre-sized to exactly that boundary. It IS a bound rather than a consumer of \
              one: it exists so a hostile server cannot push an unbounded frame into a client's \
              logs through an error Display (ASVS V7), and it is the only writer of that String. \
              The twin of src/client/subscriptions.rs's truncate entry above.",
    },
    Accumulation {
        path: "src/shared/uri_template.rs",
        needle: "push_str(",
        count: 11,
        why: "URI-template expansion and regex-pattern construction over a SERVER-AUTHORED \
              template plus a bounded variable map. This is not a peer-supplied byte stream and \
              has no incremental reader, so the accurate statement is that its input is not \
              peer-chosen — not that it carries a ceiling. It is in scope only because it lives \
              under src/shared/.",
    },
    Accumulation {
        path: "src/shared/wasm_http.rs",
        needle: "push_str(",
        count: 1,
        why: "The wasm transport has no incremental reader: the browser fetch API hands it one \
              already-materialised response String, and this loop reassembles the data lines of \
              the FIRST event out of that existing allocation, stopping at the blank line. It \
              introduces no growth beyond the response that already exists in memory.",
    },
];

/// Observed `(path, needle) -> source lines` across the whole scope.
fn observed_accumulations() -> BTreeMap<(String, &'static str), Vec<u32>> {
    let mut observed = BTreeMap::new();
    for file in scanned_scope() {
        let path = rel(&file.path);
        for needle in ACCUMULATION_NEEDLES {
            let lines: Vec<u32> = file
                .shipped_hits(needle)
                .into_iter()
                .map(|at| file.line(at))
                .collect();
            if !lines.is_empty() {
                observed.insert((path.clone(), *needle), lines);
            }
        }
    }
    observed
}

fn allowlisted_accumulation(path: &str, needle: &str) -> Option<&'static Accumulation> {
    ALLOWLIST
        .iter()
        .find(|entry| entry.path == path && entry.needle == needle)
}

/// Report an observed population the allowlist does not cover, or covers with a
/// smaller count.
fn report_unreviewed(out: &mut String, path: &str, needle: &str, lines: &[u32]) {
    let Some(entry) = allowlisted_accumulation(path, needle) else {
        let _ = writeln!(
            out,
            "\n  NEW accumulation site(s): {path} `{needle}` at line(s) {lines:?}\n    \
             Bound it, or add an ALLOWLIST entry naming the mechanism that bounds it."
        );
        return;
    };
    if lines.len() > entry.count {
        let _ = writeln!(
            out,
            "\n  COUNT ROSE: {path} `{needle}` — allowlisted {}, observed {} at line(s) {lines:?}\n    \
             One of those lines is new. Raising the number to match reality WITHOUT a justification \
             is the failure mode this test exists to prevent.",
            entry.count,
            lines.len()
        );
    }
}

/// Report an allowlist entry that no longer describes anything.
///
/// This is the anti-rot half: a stale entry is how the next round quietly loses
/// coverage, because a real new site can hide under a number that was set for a
/// site since deleted.
fn report_dead_entry(
    out: &mut String,
    entry: &Accumulation,
    observed: &BTreeMap<(String, &'static str), Vec<u32>>,
) {
    let seen = observed
        .get(&(entry.path.to_string(), entry.needle))
        .map_or(0, Vec::len);
    if seen < entry.count {
        let _ = writeln!(
            out,
            "\n  DEAD allowlist entry: {} `{}` — allowlisted {}, observed {}.\n    \
             Delete the entry (or lower its count). A site was removed and its justification was \
             not.",
            entry.path, entry.needle, entry.count, seen
        );
    }
}

#[test]
fn every_peer_byte_accumulation_is_reviewed() {
    let observed = observed_accumulations();
    let mut failures = String::new();
    for ((path, needle), lines) in &observed {
        report_unreviewed(&mut failures, path, needle, lines);
    }
    for entry in ALLOWLIST {
        report_dead_entry(&mut failures, entry, &observed);
    }
    assert!(
        failures.is_empty(),
        "HTTP-09: the reviewed accumulation population changed:{failures}\n\
         This check is a CHANGE DETECTOR, not a proof of boundedness — it exists so that a new \
         append over peer-supplied bytes cannot enter this scope without somebody writing down \
         what bounds it."
    );
}

#[test]
fn every_allowlist_justification_is_substantive() {
    let mut seen: Vec<&str> = Vec::new();
    for entry in ALLOWLIST {
        let why = entry.why.trim();
        assert!(
            why.len() >= MIN_JUSTIFICATION_CHARS,
            "ALLOWLIST entry {} `{}` needs a real justification naming the mechanism that bounds \
             it, not {why:?}",
            entry.path,
            entry.needle
        );
        assert!(
            !seen.contains(&why),
            "ALLOWLIST entry {} `{}` reuses another entry's justification verbatim; a \
             copy-pasted reason is not a reason",
            entry.path,
            entry.needle
        );
        seen.push(why);
    }
}

// ---------------------------------------------------------------------------
// Tests for the scanner itself — without these, every check built on top of it
// could pass vacuously and nobody would know.
// ---------------------------------------------------------------------------

mod scanner {
    use super::{
        cfg_requires_test, cfg_test_spans, is_excluded, line_of, occurrences, scope_files, strip,
    };

    /// Line of the first stripped occurrence of `needle`, if any.
    fn find_one(source: &str, needle: &str) -> Option<u32> {
        let stripped = strip(source);
        let at = stripped.text.find(needle)?;
        Some(line_of(&stripped, at))
    }

    #[test]
    fn line_comments_doc_comments_and_block_comments_are_not_scanned() {
        let source = "fn f() {\n    // let x = body.collect().await;\n    let y = 1;\n}\n";
        assert!(find_one(source, ".collect().await").is_none());

        let doc = "/// See `body.collect().await` for the unbounded shape.\nfn f() {}\n";
        assert!(find_one(doc, ".collect().await").is_none());

        let inner = "//! `body.collect().await` is the unbounded form.\nfn f() {}\n";
        assert!(find_one(inner, ".collect().await").is_none());

        let block =
            "fn f() {\n/* body.collect().await /* nested */ still comment */\nlet y = 1;\n}\n";
        assert!(find_one(block, ".collect().await").is_none());
        assert!(
            find_one(block, "lety=1").is_some(),
            "a nested block comment must end where Rust says it ends"
        );
    }

    #[test]
    fn string_and_raw_string_contents_are_not_scanned() {
        let source = "fn f() {\n    let msg = \"body.collect().await\";\n}\n";
        assert!(find_one(source, ".collect().await").is_none());
        assert!(
            find_one(source, "letmsg=\"\";").is_some(),
            "delimiters must survive so a call still reads as a call"
        );

        let raw = "fn f() {\n    let msg = r#\"body.collect().await\"#;\n}\n";
        assert!(find_one(raw, ".collect().await").is_none());

        let byte_raw = "fn f() {\n    let msg = br#\"body.collect().await\"#;\n}\n";
        assert!(find_one(byte_raw, ".collect().await").is_none());
    }

    #[test]
    fn lifetimes_and_char_literals_do_not_derail_the_scan() {
        let source = "fn f<'a>(s: &'a str) -> char {\n    let c = '\\'';\n    let d = 'x';\n    \
                      let _ = (c, d, s);\n    body.collect().await\n}\n";
        assert_eq!(
            find_one(source, ".collect().await"),
            Some(5),
            "tick handling swallowed the rest of the file"
        );
    }

    #[test]
    fn a_rustfmt_broken_chain_is_matched_and_reports_its_first_line() {
        let source = "fn f() {\n    let b = body\n        .collect()\n        .await;\n}\n";
        assert_eq!(
            find_one(source, ".collect().await"),
            Some(3),
            "a chain broken across lines must match, at the line of its FIRST character"
        );
    }

    #[test]
    fn cfg_requires_test_classifies_the_documented_predicate_shapes() {
        assert!(cfg_requires_test("test"));
        assert!(cfg_requires_test(
            "all(test, not(target_arch = \"wasm32\"), feature = \"streamable-http\")"
        ));
        assert!(
            !cfg_requires_test("any(feature = \"fuzzing\", test)"),
            "an any(...) item compiles WITHOUT test, so it ships and stays in scope"
        );
        assert!(!cfg_requires_test("feature = \"testing\""));
        assert!(!cfg_requires_test("all(test_utils, latest)"));
    }

    #[test]
    fn a_cfg_test_fn_excludes_only_its_own_body() {
        let source = "#[cfg(test)]\nfn helper(a: u8, b: u8) -> u8 {\n    \
                      let _ = body.collect().await;\n    a + b\n}\n\n\
                      fn shipped() {\n    body.collect().await\n}\n";
        let stripped = strip(source);
        let spans = cfg_test_spans(&stripped);
        let hits = occurrences(&stripped.text, ".collect().await");
        assert_eq!(
            hits.len(),
            2,
            "both occurrences should be lexically present"
        );
        assert!(
            is_excluded(&spans, hits[0]),
            "the cfg(test) helper body must be excluded"
        );
        assert!(
            !is_excluded(&spans, hits[1]),
            "production code AFTER a cfg(test) item must still be scanned — truncating at the \
             first marker is the under-scan this file exists to prevent"
        );
    }

    #[test]
    fn a_body_less_cfg_test_item_does_not_swallow_the_file() {
        let source = "#[cfg(test)]\nuse super::*;\n\nfn shipped() {\n    body.collect().await\n}\n";
        let stripped = strip(source);
        let spans = cfg_test_spans(&stripped);
        let hits = occurrences(&stripped.text, ".collect().await");
        assert_eq!(hits.len(), 1);
        assert!(!is_excluded(&spans, hits[0]));
    }

    #[test]
    fn scope_discovery_finds_the_named_files_at_runtime() {
        // `scope_files()` already asserts every REQUIRED_FILES entry by FULL PATH and
        // panics with a better message, so a base-name restatement here would be a
        // strictly weaker fourth copy of the scope list — in the very form this file's
        // doc calls unsafe (nine files in the tree are named `auth.rs`). What is left
        // is the one thing that check cannot cover: that the walk itself found a
        // plausible population rather than a handful of files.
        let files = scope_files();
        assert!(
            files.len() > 20,
            "src/shared/ carries about thirty files; discovering {} suggests the walk is broken",
            files.len()
        );
    }
}
