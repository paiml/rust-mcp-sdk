//! SMPL-01 severability drift gate: `full` vs `full-v2`.
//!
//! # What this file protects
//!
//! Phase 117 makes "v1 is severable" a *compile-time fact* rather than a
//! convention. The mechanism is a default-on, dependency-free `v1-compat`
//! marker feature plus a parallel `full-v2` list that is `full` minus exactly
//! `v1-compat`. The severance proof is then a real build:
//!
//! ```text
//! RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2
//! ```
//!
//! `--no-default-features` ALONE would prove nothing, because
//! `default = ["logging"]`: it would strip `http`/`streamable-http` too and
//! "prove" severability by never compiling the transport at all. Hence the
//! parallel positive list.
//!
//! # The hazard this file exists for
//!
//! `full` and `full-v2` are two ENUMERATED lists, and enumerated lists drift.
//! A feature added to `full` and forgotten in `full-v2` silently SHRINKS the
//! severance proof: the build still passes, but it now proves severability of
//! a smaller crate than the one that ships. Nothing about that failure is
//! visible — no error, no warning, just a weaker guarantee.
//!
//! # Why the scope is DERIVED, not enumerated
//!
//! Every list in this file is parsed out of `Cargo.toml` at test time. Phase
//! 116-14 proved the opposite approach wrong: an enumerated tripwire scope hid
//! two real defects, because the enumeration itself was the thing that went
//! stale. A tripwire whose scope can rot is a tripwire that reports green while
//! covering nothing.
//!
//! For the same reason the manifest is PARSED (`toml::from_str`) and never
//! string-matched line by line — see the "manifests are NEVER read as text"
//! rule recorded in `tests/v2_schema_tripwires.rs`. `[features]` values are
//! literal arrays with no rename or inheritance mechanism, so a parse is exact.
//!
//! `toml` is already a plain runtime dependency of `pmcp`, so this costs zero
//! new dependencies.

use std::collections::BTreeSet;

/// The manifest every check in this file derives its scope from.
///
/// Resolved through `CARGO_MANIFEST_DIR` so the test is independent of the
/// working directory `cargo test` happens to be invoked from.
const MANIFEST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");

/// The marker feature whose presence/absence IS the severability boundary.
const V1_COMPAT: &str = "v1-compat";

/// Floor on the parsed `full` entry count.
///
/// This exists so a broken reader cannot make the difference assertion below
/// pass over an empty set: `{} - {}` is `{}`, which would compare unequal to
/// `["v1-compat"]` — but a reader that returned a *partial* list could still
/// produce a difference that looks right for the wrong reason. `full` holds 16
/// entries today (15 pre-Phase-117 plus `v1-compat`); the floor sits at 15 so
/// legitimate additions do not need to touch it.
///
/// If this fires, the remedy is to FIX THE READER. Never lower the floor.
const MIN_FULL_ENTRIES: usize = 15;

/// Floor on the parsed `full-v2` entry count, for the same reason as
/// [`MIN_FULL_ENTRIES`]. `full-v2` holds 15 entries today; the floor sits at 14.
///
/// If this fires, the remedy is to FIX THE READER. Never lower the floor.
const MIN_FULL_V2_ENTRIES: usize = 14;

/// Parse the real `Cargo.toml`.
fn manifest() -> toml::Value {
    let text =
        std::fs::read_to_string(MANIFEST).unwrap_or_else(|e| panic!("cannot read {MANIFEST}: {e}"));
    toml::from_str(&text).unwrap_or_else(|e| panic!("cannot parse {MANIFEST} as TOML: {e}"))
}

/// Read one `[features]` entry as a set.
///
/// Panics — naming the feature — when the key is absent or is not an array of
/// strings. A missing `full-v2` must be a loud failure, not an empty set that
/// every downstream assertion then passes over vacuously.
fn feature_list(manifest: &toml::Value, name: &str) -> BTreeSet<String> {
    let features = manifest.get("features").unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: {MANIFEST} has no `[features]` table, so feature `{name}` cannot be \
             read and every check in this file would pass over an empty set.\n\
             WHAT TO DO: fix the reader or restore the table; do not weaken the assertions."
        )
    });
    let entry = features.get(name).unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: feature `{name}` is MISSING from `[features]` in {MANIFEST}.\n\
             `full`, `full-v2` and `default` are all load-bearing for the SMPL-01 severance \
             proof: `full-v2` IS the proof set, and `{V1_COMPAT}` in `default` is what keeps \
             every existing consumer working.\n\
             WHAT TO DO: restore `{name}` in Cargo.toml `[features]`; do not delete this check."
        )
    });
    let array = entry.as_array().unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: feature `{name}` in {MANIFEST} `[features]` is not an array \
             (found {entry:?}), so its entries cannot be compared.\n\
             WHAT TO DO: fix the reader, not the assertion."
        )
    });
    array
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| {
                    panic!(
                        "FAILURE MODE: feature `{name}` in {MANIFEST} `[features]` holds a \
                         non-string entry {value:?}.\n\
                         WHAT TO DO: fix the reader, not the assertion."
                    )
                })
                .to_string()
        })
        .collect()
}

/// Assert a derived feature list is large enough to be believable.
///
/// Separated out so the guard reads identically at every call site and so the
/// message always blames the READER, which is the actual cause, rather than the
/// invariant being checked.
fn assert_not_vacuous(name: &str, list: &BTreeSet<String>, floor: usize) {
    assert!(
        list.len() >= floor,
        "FAILURE MODE: derived `{name}` has only {} entr(y|ies), at or below the {floor} floor. \
         A reader that silently returns a partial or empty list makes every other check in this \
         file pass over nothing.\n\
         WHAT TO DO: fix the reader, not the assertion. Never lower the floor.",
        list.len()
    );
}

/// `full` minus `full-v2` must be EXACTLY `{v1-compat}`, in both directions.
#[test]
fn full_and_full_v2_differ_by_exactly_v1_compat() {
    let manifest = manifest();
    let full = feature_list(&manifest, "full");
    let full_v2 = feature_list(&manifest, "full-v2");

    assert_not_vacuous("full", &full, MIN_FULL_ENTRIES);
    assert_not_vacuous("full-v2", &full_v2, MIN_FULL_V2_ENTRIES);

    let only_in_full: Vec<String> = full.difference(&full_v2).cloned().collect();
    let only_in_v2: Vec<String> = full_v2.difference(&full).cloned().collect();

    assert_eq!(
        only_in_full,
        vec![V1_COMPAT.to_string()],
        "`full` minus `full-v2` must be EXACTLY [{V1_COMPAT}], but it is {only_in_full:?}.\n\
         CONSEQUENCE: a feature added to `full` and forgotten in `full-v2` silently shrinks the \
         severance proof — `cargo build -p pmcp --no-default-features --features full-v2` keeps \
         passing, but it now proves severability of a SMALLER crate than the one that ships.\n\
         WHAT TO DO: mirror the new feature into `full-v2` in Cargo.toml (everything `full` has \
         except `{V1_COMPAT}`)."
    );
    assert!(
        only_in_v2.is_empty(),
        "`full-v2` has entries `full` does not: {only_in_v2:?}.\n\
         CONSEQUENCE: `full-v2` must be a strict SUBSET of `full`, or the severance build is \
         compiling a configuration no consumer can actually get.\n\
         WHAT TO DO: remove the stray entries from `full-v2`, or add them to `full` too."
    );
}

/// `v1-compat` must stay default-on, and must stay inside `full`.
#[test]
fn v1_compat_is_in_default_and_full() {
    let manifest = manifest();
    let default = feature_list(&manifest, "default");
    let full = feature_list(&manifest, "full");

    assert!(
        default.contains(V1_COMPAT),
        "`{V1_COMPAT}` is missing from `default` (found {default:?}).\n\
         CONSEQUENCE: dropping `{V1_COMPAT}` from `default` silently breaks every existing user \
         — the MCP 2025-11-25 session/resumability layer would vanish from an ordinary \
         `pmcp = \"2\"` dependency with no error and no warning.\n\
         WHAT TO DO: restore `{V1_COMPAT}` in `default`. Removing it is SMPL-F1 / pmcp 3.0, \
         gated on public client adoption of v2 — see docs/v1-sunset-policy.md."
    );
    assert_not_vacuous("full", &full, MIN_FULL_ENTRIES);
    assert!(
        full.contains(V1_COMPAT),
        "`{V1_COMPAT}` is missing from `full` (found {full:?}).\n\
         CONSEQUENCE: `full` and `full-v2` would become identical, so the severance build would \
         prove nothing at all.\n\
         WHAT TO DO: restore `{V1_COMPAT}` in `full`."
    );
}

/// The reader itself is not vacuous — checked independently of what it is read for.
#[test]
fn the_feature_list_reader_is_not_vacuous() {
    let manifest = manifest();

    let full = feature_list(&manifest, "full");
    let full_v2 = feature_list(&manifest, "full-v2");
    let default = feature_list(&manifest, "default");

    assert_not_vacuous("full", &full, MIN_FULL_ENTRIES);
    assert_not_vacuous("full-v2", &full_v2, MIN_FULL_V2_ENTRIES);
    assert!(
        !default.is_empty(),
        "FAILURE MODE: derived `default` is empty, so the `{V1_COMPAT}`-is-default-on check \
         would pass over nothing.\n\
         WHAT TO DO: fix the reader, not the assertion."
    );

    // `full-v2` must contain the transport, or the severance build compiles no
    // transport at all and is a false green (RESEARCH Q3.5 pitfall 1).
    assert!(
        full_v2.contains("streamable-http"),
        "FAILURE MODE: `full-v2` does not contain `streamable-http`, which is where the v1 \
         session and SSE-resumability machinery lives.\n\
         CONSEQUENCE: the severance build would compile no transport and pass vacuously — it \
         would 'prove' v1 is severable by never compiling the code being severed.\n\
         WHAT TO DO: restore `streamable-http` in `full-v2`."
    );
}

// ===========================================================================
// SMPL-02: the v1 null twin, asserted at the SOURCE level.
//
// Everything above this line protects the FEATURE LISTS. Everything below
// protects the CLAIM those lists are made for: that a `full-v2` build contains
// no MCP 2025-11-25 session lifecycle and no SSE resumability.
//
// A build cannot check that claim. `cargo build --no-default-features
// --features full-v2` proves only that the null twin COMPILES; a twin that
// quietly re-implemented a session map would compile just as well. So the claim
// is checked where it lives: in the source of `v1_session_off.rs`.
// ===========================================================================

/// The `v1-compat` half of the paired module: the real v1 state.
const V1_REAL: &str = "src/server/streamable_http_server/v1_session.rs";

/// The `full-v2` half: the null twin whose emptiness IS the SMPL-02 claim.
const V1_OFF: &str = "src/server/streamable_http_server/v1_session_off.rs";

/// The file that declares the pair with two `cfg_attr` path attributes.
const TRANSPORT: &str = "src/server/streamable_http_server.rs";

// ---------------------------------------------------------------------------
// DO NOT ADD A SUBSTRING BLACKLIST HERE.
//
// An earlier draft of this gate forbade the bare substrings `sessions`,
// `event_store`, `EventStore` and `sse_streams` in the null twin. FOUR of them
// are mechanically unsatisfiable, because plans 117-09 / 117-12 / 117-13 require
// the twin to carry signatures that are TEXTUALLY IDENTICAL to their real
// counterparts. Measured against `src/server/streamable_http_server.rs`:
//
// * `sessions` is a substring of `sessions_active_for`, `sessions_active` and
//   the `sessions_on: bool` parameter of `apply_session_header`;
// * `event_store` is a substring of the parameter names `cfg_has_event_store`
//   (`resumability_active_for`) and `event_store`
//   (`replay_sse_events_from_header`, `sse_event_for_message`);
// * `EventStore` is a substring of `EventStoreHandle`, which appears in the
//   return type `Option<&EventStoreHandle>` and in two parameter types;
// * `sse_streams` is reached by the surviving `build_response` routing seam,
//   which must work through the pair on BOTH feature sets.
//
// A tripwire that rejects those identifiers would make Wave 3 unlandable behind
// a Wave 2 gate. The invariant SMPL-02 actually needs is not "these words are
// absent" — it is "no state is held and no state or header is touched". That is
// what the checks below assert, from the DERIVED declaration sets of the two
// halves rather than from a hand-written list that rots.
// ---------------------------------------------------------------------------

/// Type tokens that would mean the null twin is HOLDING v1 state.
///
/// Each entry names a container or a concrete v1 type. Their presence in the
/// twin means the `full-v2` build allocates session/SSE state after all, which
/// is the exact claim this file exists to make false.
///
/// `EventStoreHandle` is deliberately ABSENT from this list: carrying one in a
/// mirrored SIGNATURE is required by 117-09/117-12. What is forbidden is HOLDING
/// one, which `Arc<dyn EventStore` catches.
///
/// WHAT TO DO when one of these fires: MOVE the item into `v1_session.rs`, where
/// v1 state belongs. Never shorten this list to make a failure go away.
const FORBIDDEN_STATE_TYPES: &[&str] = &[
    "HashMap",
    "BTreeMap",
    "RwLock",
    "Mutex",
    "DashMap",
    "SessionInfo",
    "InMemoryEventStore",
    "Arc<dyn EventStore",
];

/// Operation tokens that would mean the null twin is TOUCHING state or headers.
///
/// A constant-answer twin needs none of them: it neither reads nor writes a
/// lock, neither inserts into nor removes from a map, spawns nothing, and never
/// looks at a request or response header. A twin that needs one of these is a
/// FINDING — the v2 answer stopped being a constant.
///
/// WHAT TO DO when one of these fires: MOVE the operation into `v1_session.rs`.
/// Never shorten this list to make a failure go away.
const FORBIDDEN_OPERATIONS: &[&str] = &[
    ".read()",
    ".write()",
    ".lock()",
    ".insert(",
    ".remove(",
    ".entry(",
    ".get_mut(",
    ".contains_key(",
    "tokio::spawn",
    "LAST_EVENT_ID",
    "MCP_SESSION_ID",
    ".headers()",
    "headers.get",
];

/// Floor on the twin's stripped byte count.
///
/// Every check below is an ABSENCE check, and absence checks pass trivially over
/// an empty string. A file that failed to read, was truncated, or was emptied by
/// a bad merge would therefore report green while proving nothing.
///
/// # Why the floor is not a token value
///
/// It was 200 bytes, an order of magnitude below the twin's actual stripped
/// size — so a twin gutted to a TENTH of itself still cleared it, and the floor
/// only caught a file that was empty rather than one that had been hollowed out.
/// It now sits at roughly 70% of the twin's current stripped length, which is
/// the level at which "several functions silently disappeared" fails rather than
/// merely "the file is gone" (WR-09).
///
/// WHAT TO DO when this fires: restore the file. If the twin legitimately shrank
/// because the REAL half shrank too, lower this in the SAME commit as that
/// deletion and say so in the message — never on its own.
const MIN_STRIPPED_BYTES: usize = 2400;

/// [`declaration_name`] recognises every item kind, including the ones a twin
/// could once have grown invisibly.
///
/// Self-test with COUNTER-EXAMPLES, in the spirit of the rest of this file: a
/// reader is only trustworthy if it is shown to answer `None` where it should.
/// Without the negative rows a `declaration_name` that returned `Some` for every
/// line would pass the positive rows and make
/// `the_v1_null_twin_declares_nothing_the_real_module_does_not` compare noise.
#[test]
fn the_declaration_reader_recognises_every_item_kind() {
    let positive = [
        ("pub(crate) const fn is_x(a: u8) -> bool {", "is_x"),
        ("pub(crate) async fn handle(a: u8) {", "handle"),
        ("    unsafe fn raw() {", "raw"),
        ("pub(in crate::server) fn scoped() {", "scoped"),
        ("pub struct V1State {", "V1State"),
        ("enum SessionState {", "SessionState"),
        ("pub(crate) union Bits {", "Bits"),
        ("trait EventStore {", "EventStore"),
        (
            "pub(crate) type EventStoreHandle = Arc<dyn EventStore>;",
            "EventStoreHandle",
        ),
        ("const MAX: usize = 4;", "MAX"),
        (
            "static SESSIONS: OnceLock<Map> = OnceLock::new();",
            "SESSIONS",
        ),
        ("pub(crate) mod inner {", "inner"),
        ("impl V1State {", "V1State"),
        ("macro_rules! session {", "session"),
    ];
    for (line, expected) in positive {
        assert_eq!(
            declaration_name(line).as_deref(),
            Some(expected),
            "FAILURE MODE: `declaration_name` did not read `{expected}` out of `{line}`.\n\
             CONSEQUENCE: an item kind the reader cannot see is an item kind the twin can grow \
             INVISIBLY — `the_v1_null_twin_declares_nothing_the_real_module_does_not` would go \
             green over a twin holding `static SESSIONS: OnceLock<…>`.\n\
             WHAT TO DO: add the keyword to ITEM_KEYWORDS, or fix the modifier stripping."
        );
    }

    for line in [
        "    if is_initialize_request(msg) {",
        "        Ok(None)",
        "let x = fn_like_name();",
        "",
        "// fn commented_out() {",
    ] {
        assert_eq!(
            declaration_name(line).as_deref(),
            None,
            "FAILURE MODE: `declaration_name` invented a declaration in `{line}`.\n\
             CONSEQUENCE: a reader that answers `Some` for ordinary code compares NOISE, so the \
             inclusion check it feeds proves nothing.\n\
             WHAT TO DO: tighten the keyword match; never loosen the counter-examples."
        );
    }
}

/// The crate root, so no check in this file names an absolute path.
fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Path relative to the crate root, for failure messages a reader can act on.
fn rel(path: &std::path::Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

/// Read a repo-relative source file, naming it if the read fails.
fn source(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "FAILURE MODE: cannot read {}: {e}.\n\
             Every check over this file would then pass over an empty string.\n\
             WHAT TO DO: restore the file, or fix the path constant — do not delete the check.",
            rel(&path)
        )
    })
}

/// Advance past a `//`-style comment, keeping the terminating newline.
fn skip_line_comment(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && chars[i] != '\n' {
        i += 1;
    }
    i
}

/// Advance past a `/* … */` comment, honouring Rust's nesting and preserving
/// the newlines inside it so line-oriented scans downstream keep their shape.
fn skip_block_comment(chars: &[char], mut i: usize, out: &mut String) -> usize {
    let mut depth = 1usize;
    i += 2;
    while i < chars.len() && depth > 0 {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            depth += 1;
            i += 2;
        } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
            depth -= 1;
            i += 2;
        } else {
            if chars[i] == '\n' {
                out.push('\n');
            }
            i += 1;
        }
    }
    i
}

/// Copy a `"…"` string literal verbatim, honouring backslash escapes.
///
/// String CONTENTS are kept rather than blanked: nothing in the twin is allowed
/// to hide a forbidden token in a string either, and keeping them means the
/// stripper has one less way to be wrong.
fn copy_string_literal(chars: &[char], mut i: usize, out: &mut String) -> usize {
    out.push(chars[i]);
    i += 1;
    while i < chars.len() {
        let c = chars[i];
        out.push(c);
        i += 1;
        if c == '\\' {
            if let Some(next) = chars.get(i) {
                out.push(*next);
                i += 1;
            }
            continue;
        }
        if c == '"' {
            break;
        }
    }
    i
}

/// The `#` count of a raw-string opener starting at `i`, if `i` starts one.
///
/// Returns `None` unless `chars[i] == 'r'`, the character BEFORE it cannot be
/// part of an identifier (so `for`, `char` and `iter` are not mistaken for a
/// raw-string prefix), and the `r` is followed by zero or more `#` and then a
/// `"`.
fn raw_string_hashes(chars: &[char], i: usize) -> Option<usize> {
    if chars[i] != 'r' {
        return None;
    }
    if i > 0 {
        let prev = chars[i - 1];
        if prev.is_alphanumeric() || prev == '_' {
            return None;
        }
    }
    let mut j = i + 1;
    let mut hashes = 0usize;
    while chars.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    if chars.get(j) == Some(&'"') {
        Some(hashes)
    } else {
        None
    }
}

/// Copy an `r#"…"#` raw string verbatim, honouring its `#` count.
///
/// Raw strings have no escape processing, so the ONLY terminator is `"` followed
/// by exactly the opening number of `#`. Handling them is not cosmetic: the
/// `#[cfg_attr(feature = "v1-compat", doc = r#"…"#)]` doc payloads in
/// [`CLIENT_TRANSPORT`] contain `http://localhost:8080`, and a stripper that
/// fell out of string mode there would treat the `//` as a line comment and eat
/// the rest of the line — silently shrinking the very region the client scan
/// has to reason about.
fn copy_raw_string(chars: &[char], mut i: usize, hashes: usize, out: &mut String) -> usize {
    // Copy the `r`, the `#`s and the opening quote.
    let opener_len = 1 + hashes + 1;
    for k in 0..opener_len {
        out.push(chars[i + k]);
    }
    i += opener_len;
    while i < chars.len() {
        if chars[i] == '"' && (1..=hashes).all(|k| chars.get(i + k) == Some(&'#')) {
            for k in 0..=hashes {
                out.push(chars[i + k]);
            }
            return i + hashes + 1;
        }
        out.push(chars[i]);
        i += 1;
    }
    i
}

/// Rust source with `//`, `///`, `//!` and `/* … */` comments removed.
///
/// The scan below MUST run on stripped source. A doc comment that mentions a
/// session map in PROSE — and the twin's module doc says a great deal about what
/// it does not do — is documentation, not an implementation. Matching raw source
/// would turn every such sentence into a false failure and push the next author
/// toward deleting the explanation instead of the code.
///
/// The inverse hazard is worse and is unit-tested by
/// `the_stripper_does_not_over_strip`: a stripper that ate real code would make
/// every check in this section pass over nothing at all.
fn strip_comments(rust: &str) -> String {
    let chars: Vec<char> = rust.chars().collect();
    let mut out = String::with_capacity(rust.len());
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            i = skip_line_comment(&chars, i);
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i = skip_block_comment(&chars, i, &mut out);
        } else if let Some(hashes) = raw_string_hashes(&chars, i) {
            i = copy_raw_string(&chars, i, hashes, &mut out);
        } else if c == '"' {
            i = copy_string_literal(&chars, i, &mut out);
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Every item keyword `declaration_name` recognises.
///
/// # Why this list is longer than it looks like it needs to be
///
/// It used to hold five entries — `const fn`, `fn`, `struct`, `type`, `const` —
/// which meant a twin that added `static SESSIONS: OnceLock<…>`, `enum
/// SessionState`, a `trait`, a `mod`, a `union`, an `impl` block or a
/// `macro_rules!` was INVISIBLE to
/// `the_v1_null_twin_declares_nothing_the_real_module_does_not`. That test bills
/// itself as "the derived replacement for an enumerated blacklist … it catches
/// invented machinery without needing a list that goes stale", and it was
/// resting on exactly such a list — in the file whose own prose condemns the
/// pattern. Phase 117's code review caught it (WR-09).
///
/// It is still an enumerated list, because Rust's item grammar is finite and a
/// parser is the wrong tool here. What changed is that it is now the WHOLE
/// grammar rather than the subset the pair happened to use: every item kind that
/// can introduce state or machinery is named, so a twin cannot grow one silently.
///
/// ORDER MATTERS. Longest-prefix first within a family, so `const fn ` resolves
/// as a function rather than as a constant named `fn`.
const ITEM_KEYWORDS: &[&str] = &[
    "const fn ",
    "async fn ",
    "unsafe fn ",
    "fn ",
    "struct ",
    "enum ",
    "union ",
    "trait ",
    "type ",
    "const ",
    "static ",
    "mod ",
    "impl ",
    "macro_rules!",
];

/// The item NAME a stripped line declares, for every kind in [`ITEM_KEYWORDS`].
///
/// Line-oriented on purpose: both halves of the pair are small, hand-written
/// files whose declarations start their own line. Visibility and the modifiers
/// that may precede the keyword are stripped first, and `const fn` is resolved
/// as a function rather than as a constant.
///
/// `pub(in path)` is handled as well as bare `pub(crate)`/`pub(super)`/`pub`:
/// before Phase 117's fix pass a `pub(in crate::server) fn` yielded `None` and
/// slipped past every name-based check.
fn declaration_name(line: &str) -> Option<String> {
    let mut rest = line.trim();
    // `pub(in some::path)` is variable-length, so it is matched structurally
    // rather than by a fixed prefix list.
    if let Some(after) = rest.strip_prefix("pub(") {
        if let Some(close) = after.find(')') {
            rest = after[close + 1..].trim_start();
        }
    } else if let Some(stripped) = rest.strip_prefix("pub ") {
        rest = stripped.trim_start();
    }
    while let Some(stripped) = ["async ", "unsafe ", "extern ", "default "]
        .iter()
        .find_map(|m| rest.strip_prefix(m))
    {
        rest = stripped.trim_start();
    }
    // `trim_start`: `macro_rules!` carries no trailing space in the keyword table
    // (the name may follow it immediately or after whitespace), and a leading
    // space would make the `take_while` below yield an empty name.
    let tail = ITEM_KEYWORDS
        .iter()
        .find_map(|kw| rest.strip_prefix(kw))?
        .trim_start();
    let name: String = tail
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Every declaration name in a stripped source file.
fn declaration_names(stripped: &str) -> BTreeSet<String> {
    stripped.lines().filter_map(declaration_name).collect()
}

/// The null twin holds NO state: a unit `V1State`, and no state-bearing type.
#[test]
fn the_v1_null_twin_holds_no_state() {
    let stripped = strip_comments(&source(V1_OFF));

    assert!(
        stripped.contains("struct V1State;"),
        "FAILURE MODE: {V1_OFF} does not declare `V1State` as a UNIT struct.\n\
         CONSEQUENCE: the zero-sized twin is what makes 'no session map is allocated on a \
         `full-v2` build' a property of the TYPE rather than of a runtime branch someone can \
         forget to take.\n\
         WHAT TO DO: keep the declaration a unit struct and move any field into {V1_REAL}."
    );
    assert!(
        !stripped.contains("struct V1State {"),
        "FAILURE MODE: {V1_OFF} gave `V1State` a field block, so the null twin now carries data.\n\
         CONSEQUENCE: severance stops being structural — the `full-v2` build allocates v1 state \
         again, and nothing else in the test suite would notice.\n\
         WHAT TO DO: move the field into {V1_REAL}; the twin answers with a constant, not a value."
    );

    for token in FORBIDDEN_STATE_TYPES {
        assert!(
            !stripped.contains(token),
            "FAILURE MODE: state-bearing type `{token}` appears in {V1_OFF}.\n\
             CONSEQUENCE: the null twin is holding v1 state, which is precisely what a `full-v2` \
             build is supposed to prove it does not do.\n\
             WHAT TO DO: MOVE it into {V1_REAL}. Do not remove `{token}` from \
             FORBIDDEN_STATE_TYPES to silence this."
        );
    }
}

/// The null twin PERFORMS no state or header operation.
#[test]
fn the_v1_null_twin_performs_no_state_or_header_operation() {
    let stripped = strip_comments(&source(V1_OFF));

    for token in FORBIDDEN_OPERATIONS {
        assert!(
            !stripped.contains(token),
            "FAILURE MODE: operation `{token}` appears in {V1_OFF}.\n\
             CONSEQUENCE: the v2 answer stopped being a constant — the twin now reads or mutates \
             state, or looks at a session/resumability header, on a build whose whole claim is \
             that neither exists.\n\
             WHAT TO DO: MOVE the operation into {V1_REAL} and leave a constant here. Do not \
             remove `{token}` from FORBIDDEN_OPERATIONS to silence this."
        );
    }
}

/// The twin declares NOTHING the real module does not.
///
/// This is the derived replacement for an enumerated blacklist: it catches
/// invented machinery without needing a list that goes stale, and it cannot
/// reject an identifier the real module legitimately carries.
#[test]
fn the_v1_null_twin_declares_nothing_the_real_module_does_not() {
    let twin = declaration_names(&strip_comments(&source(V1_OFF)));
    let real = declaration_names(&strip_comments(&source(V1_REAL)));

    assert!(
        !twin.is_empty(),
        "FAILURE MODE: no declaration was extracted from {V1_OFF}, so this check would pass over \
         an empty set.\n\
         WHAT TO DO: fix `declaration_name`, not the assertion."
    );
    assert!(
        !real.is_empty(),
        "FAILURE MODE: no declaration was extracted from {V1_REAL}, so every twin declaration \
         would look like an addition.\n\
         WHAT TO DO: fix `declaration_name`, not the assertion."
    );

    let extra: Vec<&String> = twin.difference(&real).collect();
    for name in &extra {
        eprintln!("extra declaration in {V1_OFF}, absent from {V1_REAL}: {name}");
    }
    assert!(
        extra.is_empty(),
        "FAILURE MODE: {V1_OFF} declares {extra:?}, which {V1_REAL} does not.\n\
         CONSEQUENCE: severance grew machinery of its own. The twin's only job is to answer the \
         questions v1 asks with a constant; anything it declares alone is code that exists ONLY \
         on the build that is supposed to contain less.\n\
         WHAT TO DO: declare the item in {V1_REAL} too (signature identity is what lets the \
         transport name `v1::…` unconditionally), or delete it from the twin."
    );
}

/// The absence checks above cannot pass over an empty or truncated file.
#[test]
fn the_null_twin_check_is_not_vacuous() {
    let stripped = strip_comments(&source(V1_OFF));

    assert!(
        stripped.len() >= MIN_STRIPPED_BYTES,
        "FAILURE MODE: {V1_OFF} strips to {} byte(s), below the {MIN_STRIPPED_BYTES} floor.\n\
         CONSEQUENCE: every check in this section is an ABSENCE check, and absence checks pass \
         trivially over an empty string — a truncated or unreadable file would report green while \
         proving nothing.\n\
         WHAT TO DO: restore the file. Never lower the floor.",
        stripped.len()
    );
    assert!(
        stripped.contains("V1State"),
        "FAILURE MODE: {V1_OFF} strips to something that does not mention `V1State`.\n\
         CONSEQUENCE: the reader is looking at the wrong content, so the absence checks above are \
         vacuous.\n\
         WHAT TO DO: fix the reader or restore the file."
    );
}

/// The stripper removes comments and ONLY comments.
///
/// Both directions matter. Under-stripping turns prose into false failures;
/// over-stripping makes every check in this section pass over nothing.
#[test]
fn the_stripper_does_not_over_strip() {
    let fixture = "let sessions = 1; // sessions in a line comment\n\
                   //! sessions in a module doc\n\
                   /// sessions in an item doc\n\
                   /* sessions in a block comment */\n\
                   let kept = 2;\n";
    let stripped = strip_comments(fixture);

    assert!(
        stripped.contains("let sessions = 1;"),
        "FAILURE MODE: the stripper ate real code. Observed: {stripped:?}\n\
         CONSEQUENCE: every absence check in this section would pass over a blank file.\n\
         WHAT TO DO: fix `strip_comments`, not the assertion."
    );
    assert!(
        stripped.contains("let kept = 2;"),
        "FAILURE MODE: the stripper ate code that follows a block comment. Observed: \
         {stripped:?}\n\
         WHAT TO DO: fix `strip_comments`, not the assertion."
    );
    for prose in [
        "in a line comment",
        "in a module doc",
        "in an item doc",
        "in a block comment",
    ] {
        assert!(
            !stripped.contains(prose),
            "FAILURE MODE: the stripper left `{prose}` behind. Observed: {stripped:?}\n\
             CONSEQUENCE: a doc comment that DESCRIBES what the twin does not do would be \
             matched as if it were an implementation, and the honest remedy — deleting the \
             explanation — makes the file worse.\n\
             WHAT TO DO: fix `strip_comments`, not the assertion."
        );
    }
}

/// Both halves exist, and the transport still selects between them.
///
/// Deleting one half breaks the build on ONE feature set only, which a
/// single-configuration CI job can miss entirely. This turns that into a test
/// failure on every configuration.
#[test]
fn both_paired_module_files_exist() {
    for half in [V1_REAL, V1_OFF] {
        let path = repo_root().join(half);
        assert!(
            path.is_file(),
            "FAILURE MODE: {half} is missing.\n\
             CONSEQUENCE: the paired module has one half, so `cargo build` fails on exactly one \
             feature set — the configuration a single-config CI job does not run.\n\
             WHAT TO DO: restore the file. Deleting BOTH halves is SMPL-F1 / pmcp 3.0 and is a \
             semver-major change; see docs/v1-sunset-policy.md."
        );
    }

    let transport = strip_comments(&source(TRANSPORT));
    for attribute in [
        "cfg_attr(feature = \"v1-compat\", path",
        "cfg_attr(not(feature = \"v1-compat\"), path",
    ] {
        let hits = transport.matches(attribute).count();
        assert_eq!(
            hits, 1,
            "FAILURE MODE: {TRANSPORT} contains {hits} occurrence(s) of `{attribute}`, expected \
             exactly 1.\n\
             CONSEQUENCE: the pair is selected by exactly two attributes on one `mod v1;`. Zero \
             means the seam is gone; more than one means two declarations can disagree.\n\
             WHAT TO DO: restore the single declaration. Note the `#[rustfmt::skip]` above it is \
             load-bearing — rustfmt explodes the `not(...)` form across four lines and this match \
             is single-line."
        );
    }
}

// ===========================================================================
// SMPL-01/SMPL-02, CLIENT half: the transport's session lifecycle and SSE
// resumability (plan 117-14).
//
// Everything above this line is about the SERVER: the feature lists, and the
// paired module that severs the server's session map and event store. The
// CLIENT transport carries the mirror image of all of it — a stored session id,
// a `Mcp-Session-Id` capture, a DELETE teardown, and a `Last-Event-ID` writer —
// and a `full-v2` build that severed only the server proves half of a
// requirement whose wording ("initialize/session lifecycle, SSE resumability")
// carries no server-only qualifier.
//
// Why the scope is DERIVED here too: the checks below SCAN the client transport
// for three tokens and answer "is this occurrence inside a `v1-compat` region"
// STRUCTURALLY, from brace depth and the innermost enclosing attribute. They do
// not consult a line list. 116-14's lesson is that an enumerated scope hides
// exactly the items it forgot to list, and pre-cut line numbers in a plan go
// stale the moment the first edit lands.
// ===========================================================================

/// The client transport whose v1 surface this section scans.
const CLIENT_TRANSPORT: &str = "src/shared/streamable_http.rs";

/// The deliberately UNGATED constants module, gated PER-CONST.
const HTTP_CONSTANTS: &str = "src/shared/http_constants.rs";

/// The tokens whose ungated presence in [`CLIENT_TRANSPORT`] would falsify
/// SMPL-02 on the client side.
///
/// `session_id` is the stored identity, its accessors and its capture;
/// `resumption_token` is the SSE replay cursor and its callback; and
/// `LAST_EVENT_ID` is the wire header the cursor is written into.
///
/// WHAT TO DO when one of these fires: GATE the item. Never shorten this list
/// and never narrow the scan — that is the 116-14 failure mode this section
/// exists to avoid.
const CLIENT_V1_TOKENS: &[&str] = &["session_id", "resumption_token", "LAST_EVENT_ID"];

/// Floor on the number of tracked-token occurrences the scan must FIND.
///
/// Every client check below is an ABSENCE check over the ungated remainder, and
/// absence checks pass trivially when the scan matched nothing. The transport
/// holds comfortably more than this today.
///
/// WHAT TO DO when this fires: fix the scanner or the path constant. Never
/// lower the floor.
const MIN_CLIENT_V1_OCCURRENCES: usize = 20;

/// Floor on [`CLIENT_TRANSPORT`]'s stripped byte count, for the same reason.
const MIN_CLIENT_STRIPPED_BYTES: usize = 20_000;

/// Whether one line lies inside a `v1-compat` region and/or a `#[cfg(test)]`
/// region.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct LineGate {
    /// Inside a region governed by `#[cfg(feature = "v1-compat")]` (or by a
    /// `#[cfg_attr(feature = "v1-compat", …)]` attribute's own extent).
    v1: bool,
    /// Inside a `#[cfg(test)]` region.
    test: bool,
}

/// A block opened by a line that carried a gate.
struct GateFrame {
    /// Brace depth BEFORE the opening line; the frame pops when depth returns.
    close_depth: i64,
    v1: bool,
    test: bool,
}

/// What one complete attribute says about the code around it.
struct AttrClass {
    v1: bool,
    test: bool,
    /// `#[cfg(…)]` governs the item that FOLLOWS it. `#[cfg_attr(…, doc = …)]`
    /// does NOT: it only makes its own payload conditional, so a `cfg_attr`
    /// carrying a v1-only doctest must not mark the struct beneath it as v1.
    governs_next_item: bool,
}

/// Collapse every run of whitespace to a single space.
///
/// The predicate matches below are `contains` over fixed strings with exactly
/// one space around each `=`, so a `rustfmt`-wrapped or hand-written
/// `#[cfg(not(feature =\n    "v1-compat"))]` classified as UNGATED (IN-02). The
/// failure direction was safe — a false POSITIVE finding rather than a missed
/// one — but it is a sharp edge on a file whose whole job is to be believed, and
/// one normalising pass removes it.
fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Classify one complete attribute's text.
fn classify_attribute(text: &str) -> AttrClass {
    // Whitespace-normalised FIRST, so a line-wrapped predicate reads the same as
    // a single-line one.
    let flat = normalize_whitespace(text);
    // A `not(feature = "v1-compat")` mention is the NULL-TWIN half. Remove those
    // first so only a POSITIVE mention counts as a gate — otherwise the twin,
    // whose whole point is that it carries no v1 surface, would be treated as a
    // place v1 surface is allowed to live.
    let positive = flat.replace("not(feature = \"v1-compat\")", "");
    let trimmed = text.trim_start();
    AttrClass {
        v1: positive.contains("feature = \"v1-compat\""),
        test: flat.contains("cfg(test)") || flat.contains("cfg(all(test"),
        governs_next_item: trimmed.starts_with("#[cfg(") || trimmed.starts_with("#![cfg("),
    }
}

/// A LINE-WRAPPED feature predicate classifies the same as a single-line one.
///
/// The counter-example is the point: before Phase 117's fix pass the two spellings
/// below disagreed, so a `rustfmt` pass over a long `#[cfg]` could silently flip a
/// gated region to ungated in the reader's eyes and produce a bogus finding.
#[test]
fn the_attribute_reader_is_insensitive_to_line_wrapping() {
    let single = classify_attribute("#[cfg(feature = \"v1-compat\")]");
    let wrapped = classify_attribute("#[cfg(feature =\n    \"v1-compat\")]");
    assert!(
        single.v1 && wrapped.v1,
        "FAILURE MODE: a line-wrapped `#[cfg(feature = \"v1-compat\")]` did not classify as \
         gated (single={}, wrapped={}).\n\
         CONSEQUENCE: `rustfmt` wrapping a long predicate would make the reader report a gated \
         region as ungated — a finding with no defect behind it, in the file a reader trusts to \
         tell them the difference.\n\
         WHAT TO DO: fix `normalize_whitespace`; never special-case the spelling.",
        single.v1,
        wrapped.v1
    );

    let negated = classify_attribute("#[cfg(not(feature =\n    \"v1-compat\"))]");
    assert!(
        !negated.v1,
        "FAILURE MODE: a line-wrapped `not(feature = \"v1-compat\")` classified as GATED.\n\
         CONSEQUENCE: the null-twin half would be treated as a place v1 surface may live, which \
         is the exact inversion this classifier exists to prevent.\n\
         WHAT TO DO: the negative strip must run over the NORMALISED text, not the raw text."
    );

    let wrapped_test = classify_attribute("#[cfg(all(test,\n    feature = \"v1-compat\"))]");
    assert!(
        wrapped_test.test && wrapped_test.v1,
        "FAILURE MODE: a wrapped `#[cfg(all(test, feature = \"v1-compat\"))]` lost its `test` or \
         `v1` classification.\n\
         WHAT TO DO: fix `normalize_whitespace`; the `cfg(all(test` match must read the \
         normalised text."
    );
}

/// Net `{` minus `}` on a line whose string and char literals are blanked.
fn brace_delta(skeleton_line: &str) -> i64 {
    counted(skeleton_line, '{') - counted(skeleton_line, '}')
}

/// How many times `needle` appears on a line, as a signed count.
///
/// `i64::try_from` rather than `as i64`: a lossy cast on a line long enough to
/// overflow would silently invert a depth calculation, and every frame after it
/// would close in the wrong place.
fn counted(line: &str, needle: char) -> i64 {
    i64::try_from(line.matches(needle).count()).unwrap_or(i64::MAX)
}

/// Net `[` minus `]`, for walking a multi-line attribute to its end.
fn bracket_delta(skeleton_line: &str) -> i64 {
    counted(skeleton_line, '[') - counted(skeleton_line, ']')
}

/// Net `(` minus `)`, for keeping a gate attached across a WRAPPED signature.
///
/// `rustfmt` splits a long `fn` signature across several lines, so the `{` that
/// opens the body is not on the line the `#[cfg]` governs. Without this the gate
/// would be consumed by the signature's first line and the whole body would read
/// as ungated — which is exactly the false positive this scanner reported the
/// first time it ran against the real transport.
fn paren_delta(skeleton_line: &str) -> i64 {
    counted(skeleton_line, '(') - counted(skeleton_line, ')')
}

/// The same source with the INTERIOR of every string and char literal replaced
/// by spaces, so structural counting cannot be fooled by punctuation in a
/// literal.
///
/// Line count and column count are preserved exactly, so this maps 1:1 onto the
/// stripped source it is derived from. The braces in a `doc = r#"…"#` payload —
/// which the transport's `cfg_attr` doctests are full of — must not move the
/// brace depth, or every subsequent frame would close in the wrong place.
fn structural_skeleton(stripped: &str) -> String {
    let chars: Vec<char> = stripped.chars().collect();
    let mut out = String::with_capacity(stripped.len());
    let mut i = 0usize;
    while i < chars.len() {
        if let Some(hashes) = raw_string_hashes(&chars, i) {
            let mut buf = String::new();
            let end = copy_raw_string(&chars, i, hashes, &mut buf);
            blank_literal(&buf, &mut out);
            i = end;
            continue;
        }
        if chars[i] == '"' {
            let mut buf = String::new();
            let end = copy_string_literal(&chars, i, &mut buf);
            blank_literal(&buf, &mut out);
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Push `literal` with every non-newline character replaced by a space.
fn blank_literal(literal: &str, out: &mut String) {
    for c in literal.chars() {
        out.push(if c == '\n' { '\n' } else { ' ' });
    }
}

/// Per-line answer to "is this line inside a `v1-compat` region, and/or inside
/// a `#[cfg(test)]` region".
///
/// A line-oriented state machine over the STRIPPED source, using a skeleton with
/// blanked literals for all structural counting:
///
/// * an attribute (possibly spanning lines) is walked to its closing bracket;
/// * a `#[cfg(…)]` sets a PENDING gate that applies to the next construct, while
///   a `#[cfg_attr(…)]` marks only its own lines;
/// * a construct line that opens more braces than it closes pushes a frame
///   carrying its gate, and the frame pops when brace depth returns;
/// * a construct whose parenthesis depth is still open CARRIES its gate to the
///   next line, so a `rustfmt`-wrapped `fn` signature keeps the gate until the
///   `{` that actually opens the body;
/// * an attribute with code AFTER its closing `]` (`#[cfg(…)] param: T,`)
///   governs THAT line and nothing further.
///
/// Answering structurally rather than by proximity is the whole point:
/// `the_gate_region_scanner_distinguishes_gated_from_ungated` asserts BOTH
/// directions on a fixture, because an always-true scanner would make every
/// assertion in this section vacuous — which is exactly what 116-14 shipped.
fn gate_map(stripped: &str) -> Vec<LineGate> {
    let skeleton = structural_skeleton(stripped);
    let lines: Vec<&str> = stripped.lines().collect();
    let skel: Vec<&str> = skeleton.lines().collect();
    let mut out = vec![LineGate::default(); lines.len()];
    let mut stack: Vec<GateFrame> = Vec::new();
    let mut depth: i64 = 0;
    let mut paren: i64 = 0;
    let mut carry: Option<LineGate> = None;
    let mut pending = LineGate::default();
    let mut attr_depth: i64 = 0;
    let mut attr_text = String::new();
    let mut attr_first = 0usize;

    for i in 0..lines.len() {
        let sk = skel.get(i).copied().unwrap_or("");
        let t = sk.trim();
        let inherited = LineGate {
            v1: stack.iter().any(|f| f.v1),
            test: stack.iter().any(|f| f.test),
        };

        // Still walking a multi-line attribute.
        if attr_depth > 0 {
            attr_text.push(' ');
            attr_text.push_str(lines[i]);
            attr_depth += bracket_delta(sk);
            out[i] = inherited;
            if attr_depth <= 0 {
                let class = classify_attribute(&attr_text);
                for line in out.iter_mut().take(i + 1).skip(attr_first) {
                    line.v1 |= class.v1;
                    line.test |= class.test;
                }
                if class.governs_next_item {
                    pending.v1 |= class.v1;
                    pending.test |= class.test;
                }
                attr_text.clear();
                attr_depth = 0;
            }
            continue;
        }

        // An attribute starts here.
        if t.starts_with("#[") || t.starts_with("#![") {
            let delta = bracket_delta(sk);
            if delta > 0 {
                attr_first = i;
                attr_text = lines[i].to_string();
                attr_depth = delta;
                out[i] = inherited;
                continue;
            }
            let class = classify_attribute(lines[i]);
            out[i] = LineGate {
                v1: inherited.v1 || pending.v1 || class.v1,
                test: inherited.test || pending.test || class.test,
            };
            let close = sk.rfind(']').map_or(sk.len(), |p| p + 1);
            if sk[close..].trim().is_empty() {
                if class.governs_next_item {
                    pending.v1 |= class.v1;
                    pending.test |= class.test;
                }
                continue;
            }
            // `#[cfg(…)] param: T,` — the attribute governs THIS line only.
            let before = depth;
            depth += brace_delta(sk);
            paren += paren_delta(sk);
            if depth > before {
                stack.push(GateFrame {
                    close_depth: before,
                    v1: out[i].v1,
                    test: out[i].test,
                });
                carry = None;
                paren = 0;
            } else if paren <= 0 {
                carry = None;
                paren = 0;
            }
            pending = LineGate::default();
            pop_closed_frames(&mut stack, depth);
            continue;
        }

        // A blank line keeps a pending gate alive (a stripped doc comment leaves
        // one behind between the attribute and the item it governs).
        if t.is_empty() {
            out[i] = inherited;
            continue;
        }

        let base = carry.unwrap_or(pending);
        let here = LineGate {
            v1: inherited.v1 || base.v1,
            test: inherited.test || base.test,
        };
        out[i] = here;
        let before = depth;
        depth += brace_delta(sk);
        paren += paren_delta(sk);
        if depth > before {
            stack.push(GateFrame {
                close_depth: before,
                v1: here.v1,
                test: here.test,
            });
            carry = None;
            paren = 0;
        } else if paren > 0 {
            // A wrapped signature or call: the gate stays attached until the
            // parentheses close and the body's `{` arrives.
            carry = Some(base);
        } else {
            carry = None;
            paren = 0;
        }
        pending = LineGate::default();
        pop_closed_frames(&mut stack, depth);
    }
    out
}

/// Pop every frame whose block has closed at `depth`.
fn pop_closed_frames(stack: &mut Vec<GateFrame>, depth: i64) {
    while let Some(frame) = stack.last() {
        if depth <= frame.close_depth {
            stack.pop();
        } else {
            break;
        }
    }
}

/// One tracked-token occurrence the scan found.
struct Occurrence {
    line_number: usize,
    token: &'static str,
    text: String,
}

/// Every tracked-token occurrence in `relative`, split into gated and ungated.
///
/// `#[cfg(test)]` regions are excluded entirely: the transport's own unit tests
/// legitimately construct v1 configurations to prove the v1 path still works,
/// and they never ship in a `full-v2` LIBRARY build.
fn client_v1_occurrences(relative: &str) -> (Vec<Occurrence>, Vec<Occurrence>) {
    let stripped = strip_comments(&source(relative));
    let gates = gate_map(&stripped);
    let mut inside_gate = Vec::new();
    let mut outside_gate = Vec::new();
    for (index, line) in stripped.lines().enumerate() {
        let gate = gates.get(index).copied().unwrap_or_default();
        if gate.test {
            continue;
        }
        for token in CLIENT_V1_TOKENS {
            for _ in line.matches(token) {
                let occurrence = Occurrence {
                    line_number: index + 1,
                    token,
                    text: line.trim().to_string(),
                };
                if gate.v1 {
                    inside_gate.push(occurrence);
                } else {
                    outside_gate.push(occurrence);
                }
            }
        }
    }
    (inside_gate, outside_gate)
}

/// The client transport holds NO ungated session or resumability surface.
#[test]
fn the_client_transport_carries_no_ungated_session_state() {
    let (_gated, ungated) = client_v1_occurrences(CLIENT_TRANSPORT);

    for occurrence in &ungated {
        eprintln!(
            "ungated `{}` at {CLIENT_TRANSPORT}:{} — {}",
            occurrence.token, occurrence.line_number, occurrence.text
        );
    }
    assert!(
        ungated.is_empty(),
        "FAILURE MODE: {CLIENT_TRANSPORT} names v1 session/resumability surface OUTSIDE any \
         `#[cfg(feature = \"v1-compat\")]` region — {} occurrence(s), first at line {} (`{}`): {}\n\
         CONSEQUENCE: a `full-v2` build still compiles the client half of the MCP 2025-11-25 \
         session lifecycle, so SMPL-01/SMPL-02 are proven for the server only while the \
         requirement's wording covers both.\n\
         WHAT TO DO: GATE the item, or route the read through a paired accessor whose `full-v2` \
         twin answers a constant. NEVER narrow the scan or shorten CLIENT_V1_TOKENS — an \
         enumerated scope hides exactly what it forgot to list (116-14).",
        ungated.len(),
        ungated[0].line_number,
        ungated[0].token,
        ungated[0].text
    );
}

/// The client scan cannot pass over an empty, truncated or unmatched file.
#[test]
fn the_client_inventory_check_is_not_vacuous() {
    let stripped = strip_comments(&source(CLIENT_TRANSPORT));
    assert!(
        stripped.len() >= MIN_CLIENT_STRIPPED_BYTES,
        "FAILURE MODE: {CLIENT_TRANSPORT} strips to {} byte(s), below the \
         {MIN_CLIENT_STRIPPED_BYTES} floor.\n\
         CONSEQUENCE: the client checks are ABSENCE checks and absence checks pass trivially over \
         an empty string, so a truncated or unreadable file would report green while proving \
         nothing.\n\
         WHAT TO DO: restore the file or fix the path constant. Never lower the floor.",
        stripped.len()
    );

    let (gated, ungated) = client_v1_occurrences(CLIENT_TRANSPORT);
    let found = gated.len() + ungated.len();
    assert!(
        found >= MIN_CLIENT_V1_OCCURRENCES,
        "FAILURE MODE: the client scan matched {found} tracked-token occurrence(s), below the \
         {MIN_CLIENT_V1_OCCURRENCES} floor.\n\
         CONSEQUENCE: `the_client_transport_carries_no_ungated_session_state` would then be \
         vacuous — it passes by matching nothing rather than by finding everything gated.\n\
         WHAT TO DO: fix `gate_map` / `strip_comments` / CLIENT_V1_TOKENS. Never lower the floor."
    );
    assert!(
        !gated.is_empty(),
        "FAILURE MODE: the client scan found tracked tokens but classified NONE of them as gated.\n\
         CONSEQUENCE: `gate_map` is returning `false` unconditionally, which would make the \
         ungated-set assertion fire on everything — or, with the opposite bug, pass on nothing.\n\
         WHAT TO DO: fix `gate_map`, not the assertion."
    );
}

/// The line index of the first stripped line whose trimmed form starts with
/// `needle`, together with the file's gate map.
fn declaration_gate(relative: &str, needle: &str) -> LineGate {
    let stripped = strip_comments(&source(relative));
    let gates = gate_map(&stripped);
    for (index, line) in stripped.lines().enumerate() {
        if line.trim_start().starts_with(needle) {
            return gates.get(index).copied().unwrap_or_default();
        }
    }
    panic!(
        "FAILURE MODE: no line in {relative} starts with `{needle}`.\n\
         CONSEQUENCE: the check that depends on it would have nothing to assert about.\n\
         WHAT TO DO: fix the needle or restore the declaration — do not delete the check."
    );
}

/// `LAST_EVENT_ID` and its ONE client reader carry the SAME gate.
///
/// Gating the const without its reader — or the reader without the const — is a
/// compile break on exactly one feature set, which is the configuration a
/// single-config CI job does not run.
#[test]
fn the_last_event_id_const_and_its_reader_are_co_gated() {
    let declaration = declaration_gate(HTTP_CONSTANTS, "pub const LAST_EVENT_ID");
    assert!(
        declaration.v1,
        "FAILURE MODE: `LAST_EVENT_ID` in {HTTP_CONSTANTS} is NOT governed by \
         `#[cfg(feature = \"v1-compat\")]`.\n\
         CONSEQUENCE: the SSE replay cursor's header name survives into a build whose whole claim \
         is that it never writes an attacker-influenced cursor onto the wire (T-117-53).\n\
         WHAT TO DO: gate the const AND its reader in {CLIENT_TRANSPORT} in ONE edit — gating \
         either alone does not compile."
    );

    let ungated_readers: Vec<Occurrence> = client_v1_occurrences(CLIENT_TRANSPORT)
        .1
        .into_iter()
        .filter(|occurrence| occurrence.token == "LAST_EVENT_ID")
        .collect();
    assert!(
        ungated_readers.is_empty(),
        "FAILURE MODE: {CLIENT_TRANSPORT} reads `LAST_EVENT_ID` outside any `v1-compat` region \
         (first at line {}).\n\
         CONSEQUENCE: the const is gated but its reader is not, so `--no-default-features \
         --features full-v2` fails to compile — and `--features streamable-http` without \
         `v1-compat` fails too (T-117-54).\n\
         WHAT TO DO: gate the reader. Do not ungate the const to make this pass.",
        ungated_readers
            .first()
            .map_or(0, |occurrence| occurrence.line_number)
    );
}

/// A `full-v2` build names NO resumption cursor at the reconnect call site
/// (Phase 118.2, plan 04, D-03 / T-118.2-04-03).
///
/// # Why this assertion lives HERE and not in `tests/client_sse_stream.rs`
///
/// That file's `#![cfg]` header REQUIRES `v1-compat`, so it cannot observe a
/// `full-v2` build at all — a fence written there asserting "the reconnect GET
/// carries no `Last-Event-ID`" would simply not compile on the build whose
/// behaviour it claims to measure. The property is therefore asserted on the
/// SOURCE, in the file that already owns every other severance-by-construction
/// check, where `--features full` and `--features full-v2` see the same bytes.
///
/// Two things are checked, and they are the two halves of the pattern:
///
/// 1. `reconnect_cursor` is a `#[cfg]` PAIR — one half inside a `v1-compat`
///    region, one half outside it. A single ungated definition reading the
///    config would not compile on `full-v2` (the field does not exist); a single
///    GATED definition would leave the reconnect call site needing its own
///    `#[cfg]`, which is exactly what (2) forbids.
/// 2. The number of `v1-compat` attributes governing a STATEMENT in
///    [`CLIENT_TRANSPORT`] is still exactly ONE, and it is still the
///    `apply_resumption_header` call. The file's own comment at that site says
///    it is the only call-site `#[cfg]` in the file and asks that a second not
///    accumulate; this is what turns that request into a gate.
#[test]
fn a_full_v2_build_names_no_resumption_cursor_on_reconnect() {
    let stripped = strip_comments(&source(CLIENT_TRANSPORT));
    let gates = gate_map(&stripped);

    let halves: Vec<LineGate> = stripped
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            trimmed.starts_with("fn reconnect_cursor")
                || trimmed.starts_with("const fn reconnect_cursor")
        })
        .map(|(index, _)| gates.get(index).copied().unwrap_or_default())
        .collect();

    assert_eq!(
        halves.len(),
        2,
        "FAILURE MODE: {CLIENT_TRANSPORT} declares {} `reconnect_cursor` half/halves, not 2.\n\
         CONSEQUENCE: the reconnect cursor is not a paired accessor. Either `full-v2` fails to \
         compile (an ungated half reading a field that build does not have), or the reconnect call \
         site needs its own `#[cfg]` — the second call-site gate this file exists to prevent.\n\
         WHAT TO DO: restore the pair, modelled on `resumption_callback`. Do not delete this test.",
        halves.len()
    );
    assert!(
        halves.iter().any(|gate| gate.v1),
        "FAILURE MODE: neither `reconnect_cursor` half sits inside a `v1-compat` region.\n\
         CONSEQUENCE: the half that READS the stored cursor is compiled into `full-v2`, so a \
         severed build still names a resumption cursor.\n\
         WHAT TO DO: gate the reading half."
    );
    assert!(
        halves.iter().any(|gate| !gate.v1),
        "FAILURE MODE: both `reconnect_cursor` halves are `v1-compat`-gated.\n\
         CONSEQUENCE: `full-v2` has no `reconnect_cursor` at all, so the reconnect call site must \
         carry its own `#[cfg]` — the accumulation the site's comment forbids.\n\
         WHAT TO DO: add the null twin returning the constant `None`."
    );

    let call_sites = v1_gated_statements(&stripped, &gates);
    assert_eq!(
        call_sites.len(),
        1,
        "FAILURE MODE: {CLIENT_TRANSPORT} has {} `v1-compat` attribute(s) governing a STATEMENT, \
         not 1. Observed: {call_sites:?}\n\
         CONSEQUENCE: the file's one unavoidable call-site gate has grown a sibling. Every such \
         gate is a place where the two feature sets execute DIFFERENT code paths rather than the \
         same path over a different constant, and each one has to be reasoned about separately.\n\
         WHAT TO DO: route the new v1 read through a paired accessor whose `full-v2` twin answers \
         a constant, exactly as `resumption_callback`, `outbound_session_from` and \
         `reconnect_cursor` do.",
        call_sites.len()
    );
    assert!(
        call_sites[0].contains("apply_resumption_header"),
        "FAILURE MODE: the single `v1-compat`-gated statement in {CLIENT_TRANSPORT} is \
         `{}`, not the `apply_resumption_header` call.\n\
         CONSEQUENCE: either the one sanctioned call site moved without this gate being updated, \
         or the classifier below stopped recognising it — in which case the count above is \
         measuring something other than what it claims.\n\
         WHAT TO DO: check the site, then this classifier. Never relax the assertion.",
        call_sites[0]
    );
}

/// The statements — not items, not struct-expression fields — governed by a
/// `v1-compat` attribute in `stripped`.
///
/// The classification is deliberately narrow, because the four OTHER `v1-compat`
/// attributes inside function bodies in [`CLIENT_TRANSPORT`] are
/// struct-EXPRESSION fields (`session_id: None,` in the builder's `new` and
/// `build`). A field ends with `,`; a statement ends with `;`. Item declarations
/// that also end with `;` — `use`, `const`, `static`, `type`, `mod` — are named
/// and excluded, so what remains is exactly "a gated line of executable code".
fn v1_gated_statements(stripped: &str, gates: &[LineGate]) -> Vec<String> {
    const ITEM_STARTS: &[&str] = &[
        "use ",
        "pub use ",
        "const ",
        "pub const ",
        "static ",
        "pub static ",
        "type ",
        "pub type ",
        "mod ",
        "pub mod ",
        "extern ",
    ];
    let lines: Vec<&str> = stripped.lines().collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.trim() != "#[cfg(feature = \"v1-compat\")]" {
            continue;
        }
        // Attributes stack; the governed line is the next one that is not itself
        // an attribute and not blank.
        let Some(governed) = lines[index + 1..]
            .iter()
            .map(|next| next.trim())
            .find(|next| !next.is_empty() && !next.starts_with("#["))
        else {
            continue;
        };
        if !governed.ends_with(';') || ITEM_STARTS.iter().any(|kw| governed.starts_with(kw)) {
            continue;
        }
        if gates.get(index).copied().unwrap_or_default().test {
            continue;
        }
        found.push(governed.to_string());
    }
    found
}

/// The v2-REQUIRED constants are still ungated — the live counter-example.
///
/// Without this, a `gate_map` that returned `true` for every line would make
/// every assertion above pass while proving nothing. These three constants MUST
/// come back `false`, from the same scanner, on the same file.
///
/// `MCP_SESSION_ID` is in this list DELIBERATELY and by MEASUREMENT (plan 117-14,
/// assumption A4): its server-side readers sit on the shared v2 POST path, and
/// the v2 test surface names it precisely to assert its ABSENCE. What is severed
/// is the STORING and SENDING of a session id, not the header's name.
#[test]
fn the_v2_required_constants_are_still_ungated() {
    for needle in [
        "pub const MCP_METHOD",
        "pub const MCP_NAME",
        "pub const MCP_SESSION_ID",
    ] {
        let gate = declaration_gate(HTTP_CONSTANTS, needle);
        assert!(
            !gate.v1,
            "FAILURE MODE: `{needle}` in {HTTP_CONSTANTS} is governed by \
             `#[cfg(feature = \"v1-compat\")]`.\n\
             CONSEQUENCE: either a v2-REQUIRED constant (VERS-05) was severed and the v2 build is \
             now broken, or — if this fires while the code is correct — the gate scanner returns \
             `true` unconditionally and every check in this section is vacuous.\n\
             WHAT TO DO: ungate the constant. Severance here is PER-CONST; gating the module is \
             forbidden by that module's own doc."
        );
    }
}

/// The gate scanner distinguishes gated from ungated, in BOTH directions.
///
/// An always-true scanner passes every absence check; an always-false one fails
/// on correct code and pressures the next author to weaken the assertion. The
/// fixture therefore carries one gated occurrence, one ungated occurrence, one
/// null-twin (`not(...)`) occurrence and one `#[cfg(test)]` occurrence.
#[test]
fn the_gate_region_scanner_distinguishes_gated_from_ungated() {
    let fixture = r#"
pub struct Thing {
    pub url: String,
    #[cfg(feature = "v1-compat")]
    pub session_id: Option<String>,
    pub ungated_session_id: Option<String>,
}

#[cfg(feature = "v1-compat")]
fn gated_block(&self) -> Option<String> {
    self.config.read().session_id.clone()
}

#[cfg(feature = "v1-compat")]
fn gated_wrapped_signature(
    request: &mut Request,
    resumption_token: Option<&str>,
) -> Result<()> {
    request.insert(LAST_EVENT_ID, resumption_token);
}

#[cfg(not(feature = "v1-compat"))]
const fn twin(&self) -> Option<String> {
    None
}

fn ungated_block(&self) -> Option<String> {
    self.config.read().session_id.clone()
}

#[cfg(test)]
mod tests {
    fn helper(session_id: Option<&str>) {}
}
"#;
    let stripped = strip_comments(fixture);
    let gates = gate_map(&stripped);
    let lines: Vec<&str> = stripped.lines().collect();

    let gate_of = |needle: &str| -> LineGate {
        let index = lines
            .iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("fixture line containing `{needle}` not found"));
        gates[index]
    };

    // Direction 1: gated occurrences are ACCEPTED.
    assert!(
        gate_of("pub session_id: Option<String>").v1,
        "FAILURE MODE: a field carrying `#[cfg(feature = \"v1-compat\")]` was NOT recognised as \
         gated.\n\
         CONSEQUENCE: correct code fails the client scan, and the pressure is to weaken the \
         assertion rather than fix the scanner.\n\
         WHAT TO DO: fix `gate_map`."
    );
    assert!(
        gate_of("fn gated_block").v1,
        "FAILURE MODE: a `#[cfg(feature = \"v1-compat\")]` fn was not recognised as gated.\n\
         WHAT TO DO: fix `gate_map`'s frame handling."
    );
    assert!(
        gate_of("resumption_token: Option<&str>,").v1,
        "FAILURE MODE: a `rustfmt`-WRAPPED signature under `#[cfg(feature = \"v1-compat\")]` lost \
         its gate — the parameter lines read as ungated because the `{{` that opens the body is \
         on a later line.\n\
         CONSEQUENCE: correctly gated helpers such as `apply_resumption_header` report as ungated \
         v1 surface, and the pressure is to un-wrap real code to appease the scanner.\n\
         WHAT TO DO: fix `gate_map`'s paren carry, not the assertion or the source."
    );
    assert!(
        gate_of("request.insert(LAST_EVENT_ID").v1,
        "FAILURE MODE: the BODY of a wrapped-signature gated fn was not recognised as gated.\n\
         WHAT TO DO: fix `gate_map`'s paren carry."
    );

    // Direction 2: ungated occurrences are REJECTED.
    assert!(
        !gate_of("pub ungated_session_id").v1,
        "FAILURE MODE: an UNGATED field was reported as gated, so `gate_map` returns `true` too \
         readily.\n\
         CONSEQUENCE: every absence check in this section becomes vacuous — the 116-14 failure \
         mode exactly.\n\
         WHAT TO DO: fix `gate_map`."
    );
    assert!(
        !gate_of("fn ungated_block").v1,
        "FAILURE MODE: an ungated fn was reported as gated — the gate leaked past the end of the \
         preceding `#[cfg]` block.\n\
         WHAT TO DO: fix `gate_map`'s frame popping."
    );

    // The null twin is NOT a place v1 surface may live.
    assert!(
        !gate_of("const fn twin").v1,
        "FAILURE MODE: `#[cfg(not(feature = \"v1-compat\"))]` was treated as a v1-compat gate.\n\
         CONSEQUENCE: the null twin — the half that exists ONLY on `full-v2` — would become a \
         legal home for session state, inverting the whole point of the pair.\n\
         WHAT TO DO: fix `classify_attribute`."
    );

    // `#[cfg(test)]` is recognised, so test fixtures are excluded rather than
    // silently counted as production surface.
    assert!(
        gate_of("fn helper(session_id").test,
        "FAILURE MODE: a `#[cfg(test)]` region was not recognised.\n\
         CONSEQUENCE: the transport's own v1 unit tests would be reported as ungated production \
         surface, and the honest remedy — deleting them — makes the suite worse.\n\
         WHAT TO DO: fix `gate_map`."
    );
}

/// The stripper handles raw strings, so a `//` inside one is not a comment.
///
/// [`CLIENT_TRANSPORT`] carries `#[cfg_attr(feature = "v1-compat", doc = r#"…"#)]`
/// payloads containing `http://localhost:8080`. A stripper that fell out of
/// string mode there would eat the rest of that line — and, worse, desynchronise
/// so the attribute's closing `)]` disappeared, leaving `gate_map` inside an
/// unterminated attribute and marking everything after it as gated.
#[test]
fn the_stripper_handles_raw_strings() {
    let fixture = "let a = r#\"see http://localhost:8080 for more\"#;\nlet kept = 2; // gone\n";
    let stripped = strip_comments(fixture);

    assert!(
        stripped.contains("http://localhost:8080"),
        "FAILURE MODE: the stripper treated `//` INSIDE a raw string as a line comment. \
         Observed: {stripped:?}\n\
         WHAT TO DO: fix `raw_string_hashes` / `copy_raw_string`, not the assertion."
    );
    assert!(
        stripped.contains("let kept = 2;"),
        "FAILURE MODE: the stripper desynchronised after a raw string. Observed: {stripped:?}\n\
         WHAT TO DO: fix `copy_raw_string`."
    );
    assert!(
        !stripped.contains("gone"),
        "FAILURE MODE: a real line comment survived. Observed: {stripped:?}\n\
         WHAT TO DO: fix `strip_comments`."
    );
}
