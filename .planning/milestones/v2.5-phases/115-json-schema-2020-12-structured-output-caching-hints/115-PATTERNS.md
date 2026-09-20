# Phase 115: JSON Schema 2020-12 + Structured Output + Caching Hints — Pattern Map

**Mapped:** 2026-07-31
**Files analyzed:** 20 (7 new, 13 modified)
**Analogs found:** 19 / 20 (1 partial — the D-03 manifest scanner half)

> Every excerpt below was read from the working tree in this session. Line numbers are as-of
> 2026-07-31 on branch `fix/mcp-publisher-oidc-audience`. Re-grep before quoting a line number
> into a plan action; do NOT re-derive the *pattern* — that work is done here.

---

## File Classification

### New files

| New file | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `schema/vendored/core-2026-07-28/schema.ts` + `schema.json` | reference artifact (read-only) | file-I/O | `schema/vendored/ext-tasks/schema.ts` + `schema.json` | **exact** |
| `schema/vendored/core-2026-07-28/PROVENANCE.md` | config / attestation | file-I/O | `schema/vendored/ext-tasks/PROVENANCE.md` | **exact** |
| `tests/v1_lists_golden.rs` | test (golden byte fixture) | request-response | `tests/v1_tasks_golden.rs` | **exact** |
| `tests/v2_schema_tripwires.rs` | test (source tripwire) | batch / static scan | `tests/v2_tasks_tripwires.rs` (source half) | **exact** for `.rs` half; **partial** for the `Cargo.toml` half (see § No Analog Found) |
| `tests/v2_caching_hints.rs` | test (integration) | request-response | `tests/structured_tool_output.rs` (twin-dispatcher shape) + `tests/common/v2.rs` (HTTP harness) | **role-match** |
| `examples/sNN_v2_caching_hints.rs` | example | request-response | `examples/s47_v2_stateless_mrtr.rs` | **exact** |
| `CacheScope` enum (new type, home TBD) | model (wire enum) | CRUD | `TaskStatus`, `src/types/tasks.rs:11-26` | **exact** |

### Modified files

| Modified file | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `Cargo.toml:135` (jsonschema bump) | config | — | `Cargo.toml:135` itself (in-place edit); `Cargo.toml:605-607` for `[[example]]` registration | **exact** |
| `crates/pmcp-agent/Cargo.toml:27`, `crates/pmcp-server-toolkit/Cargo.toml:54` | config | — | same line, same shape | **exact** |
| `src/server/output_validation.rs` | service (validation) | transform | itself (existing `cached_validator`) + `src/server/core.rs:605-616` (era-capture-before-move) | **role-match** — **first era branch in this module** |
| `src/types/tools.rs` (`ListToolsResult` fields; `structured_value` ctor) | model | CRUD | `CallToolResult::structured_with_text` (`tools.rs:670`) for the sibling ctor; `src/types/tasks.rs:726-742` for the field rustdoc/serde shape | **exact** |
| `src/types/resources.rs` (3 result types) | model | CRUD | `ReadResourceResult._meta` (`resources.rs:369-388`) — the *additive field on a `#[non_exhaustive]` result* precedent | **exact** |
| `src/types/prompts.rs` (`ListPromptsResult`) | model | CRUD | same as above | **exact** |
| `src/types/protocol/mod.rs` (`ServerDiscoverResult`, the **sixth** type) | model | CRUD | same as above | **exact** |
| `src/server/core.rs:1561` (`inject_v2_result_envelope`) | middleware (egress projection) | request-response | itself — it already IS the D-12 chokepoint | **exact** |
| `src/server/core.rs:3186-3247` (cacheable capture) | controller (dispatch) | request-response | `src/server/core.rs:605-616` (`CreateTrigger::resolve` captured before the move) | **exact** |
| `src/server/mod.rs:1614/1705/2211` (twin sites) | controller (dispatch) | request-response | `src/server/mod.rs:2001` (`create_path_era`) + `:1705` | **exact** |
| `src/types/protocol/version.rs:42` (`Era` derives) | model | — | itself — needs `Hash` added; see § Structural Findings F1 | **exact** |
| `tests/vendored_schema_provenance.rs` (generalize to `schema/vendored/*`) | test (tripwire) | file-I/O | itself — `discover_vendored_files` already recurses | **exact** |
| `tests/structured_tool_output.rs` (extend, SCHM-02) | test (integration) | request-response | itself | **exact** |
| `tests/property_tests.rs` (extend, ALWAYS) | test (property) | transform | `tests/property_tests.rs:21-48` `proptest!` block; `src/types/tasks.rs:1474-1500` for in-module proptests | **exact** |

---

## Pattern Assignments

### `schema/vendored/core-2026-07-28/PROVENANCE.md` (config, file-I/O) — D-14

**Analog:** `schema/vendored/ext-tasks/PROVENANCE.md` (156 lines — copy its *section skeleton verbatim*)

Required sections, in this order (the provenance test at
`tests/vendored_schema_provenance.rs` reads only the digests and the 40-hex SHA, but the
narrative sections are what a reviewer reads):

```markdown
# Vendored schema provenance — `modelcontextprotocol/modelcontextprotocol`
**Produced by:** Phase 115 plan `115-NN`, Task N
**Fetch date (UTC):** ...
## What these files are
## THESE FILES ARE A READ-ONLY REFERENCE ARTIFACT      <- copy the 4 bullets verbatim
## Source                                              <- table w/ **Pinned commit (full 40 chars)**
## Vendored files                                      <- table: Local path | Upstream path | Bytes | Lines | **SHA256**
### Independent corroboration — git blob SHA-1         <- table: local `git hash-object` vs GitHub contents API
## Reproducing this fetch                              <- runnable bash block
## Why these are pre-final values / RE-VERIFICATION OBLIGATION
## Change protocol                                     <- 5 numbered steps ending in the nextest command
```

**The `exclude`-list note (copy this reasoning, `ext-tasks/PROVENANCE.md:38-42`):**

```markdown
`schema/` is deliberately **not** added to `Cargo.toml`'s `[package] exclude` list. The total is
56,324 bytes, which is immaterial against the crates.io limit, and excluding it would break
`tests/vendored_schema_provenance.rs` for anyone running `cargo test` on the published crate —
the same failure mode that forced `tests/team_contracts_conformance.rs` out of the package when
`contracts/` was excluded (see the comment at `Cargo.toml:41-45`).
```

⚠ The new tree is ~280 KB (`schema.ts` 98,426 B + `schema.json` 181,474 B) vs the existing
56,324 B. Restate the size arithmetic in the new record rather than copying "56,324".

**Reproduce-block pattern** (`ext-tasks/PROVENANCE.md:93-109`) — retarget repo/path/SHA:

```bash
gh api repos/<owner>/<repo>/commits/<40-char-sha> \
  --jq '{sha:.sha,date:.commit.author.date,subject:(.commit.message|split("\n")[0])}'
BASE=https://raw.githubusercontent.com/<owner>/<repo>/<40-char-sha>/schema/2026-07-28
curl -sSf -o /tmp/schema.ts   "$BASE/schema.ts"
curl -sSf -o /tmp/schema.json "$BASE/schema.json"
shasum -a 256 /tmp/schema.ts /tmp/schema.json
diff /tmp/schema.ts   schema/vendored/core-2026-07-28/schema.ts
```

---

### `tests/vendored_schema_provenance.rs` (test/tripwire, file-I/O) — Pitfall 8

**Analog:** itself. The generalization is three constants and one loop.

**What is hardcoded today (lines 62, 66, 74):**

```rust
/// The vendored artifact directory, relative to the crate root.
const VENDORED_DIR: &str = "schema/vendored/ext-tasks";

/// The attribution record. Excluded from digest computation — it is the thing
/// the digests are recorded *in*, so digesting it would be circular.
const PROVENANCE_FILE: &str = "PROVENANCE.md";

/// A floor on how many files the scan must find, so a passing run can never mean
/// "the directory was empty".
const MINIMUM_VENDORED_FILES: usize = 2;
```

**Reusable as-is (do NOT rewrite):** `discover_vendored_files` (:101-120, already recurses),
`sha256_of` (:128-142, in-process `sha2`, no subprocess, no skip path),
`maximal_hex_runs` (:155-169 — the maximal-run trick that stops a 64-hex digest from
satisfying a 40-hex commit-SHA search), `recorded_digests` (:196-201).

**The four tests to re-shape into a per-directory loop** (:207, :238, :256, :294, :338). The
anti-vacuity guard becomes a floor on the number of **directories**:

```rust
// new: a floor on subdirectories, so adding a tree cannot leave it unchecked
const MINIMUM_VENDORED_TREES: usize = 2;   // ext-tasks + core-2026-07-28
```

Preserve the failure-message voice verbatim — it is the file's load-bearing property
(`FAILURE MODE 1/2/3` headers + a `WHAT TO DO:` block per assertion, e.g. :268-286).

---

### `tests/v1_lists_golden.rs` (test/golden, request-response) — D-13, **wave 1, before any field lands**

**Analog:** `tests/v1_tasks_golden.rs` (1012 lines). Copy the whole instrument.

**Module-doc contract to restate** (`v1_tasks_golden.rs:1-48`) — the four headings:
`# Read this before you change a literal in this file`, `# Why a RAW-STRING comparison, and what
the ONLY permitted normalization is`, and the both-paths coverage note.

**Feature gate + harness imports** (:49-69):

```rust
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{post, spawn_stateless_config, v1_body, Resp};
```

**The width-preserving normalization** (:82-211) — `DynamicField { key, token, shape,
shape_description }`, `width_preserving`, `substitute_one`. For the five/six list+read
results there are likely **no dynamic values at all**, so the correct import is
`NO_DYNAMICS`-style:

```rust
/// The router-backed path returns values the test router chose, so NOTHING is
/// normalized there — those goldens are pinned verbatim, byte for byte.
const NO_DYNAMICS: &[DynamicField] = &[];
```

**The assertion helper's four-step contract** (:275-292 rustdoc, :292 signature) — copy the
ordering and the doc:

```rust
/// 1. **Width invariant.** A same-width substitution must leave the length
///    unchanged and every dynamic key's occurrence count unchanged. …
/// 2. **RAW-STRING comparison** against the canonical golden — the load-bearing
///    assertion, and the only one that sees key order, spacing and
///    omission-versus-null.
/// 3. **Structural comparison** of the parsed frame. …
/// 4. **v2 leak guards**: neither `resultType` nor `serverInfo` may appear on a
///    v1 wire, plus the `_meta` rule.
fn assert_v1_bytes(raw: &str, golden: &V1Golden<'_>) {
```

⚠ Extend step 4's leak guard for this phase: add `ttlMs` and `cacheScope` to the
must-not-appear set on the v1 wire (D-11).

**Golden literal shape** (:584-590) — raw strings, one per fixture, full JSON-RPC frame:

```rust
const STORE_LIST: &str = r#"{"jsonrpc":"2.0","id":4,"result":{"tasks":[{"taskId":"<TASK-ID>",…}]}}"#;
```

**Test-name prefix rule (Pitfall 4):** every test function starts with the file stem
(`v1_tasks_golden_list_store_backed`, :806) so `test(/v1_lists_golden/)` selects correctly.
Name yours `v1_lists_golden_*`.

**Server/spawn/shutdown trio** (:508-551):

```rust
async fn spawn(server: Server) -> (SocketAddr, JoinHandle<()>) {
    spawn_stateless_config(server).await
}
async fn shutdown(handle: JoinHandle<()>) {
    common::v2::teardown(handle, ()).await;   // D-113-T teardown order
}
fn tasks_body(id: i64, method: &str, params: Value) -> String { v1_body(method, json!(id), params) }
```

**Test body shape** (:628-651) — spawn → post → shutdown → `assert_eq!(status, 200)` →
`assert_v1_bytes`.

---

### `tests/v2_schema_tripwires.rs` (test/tripwire, static scan) — D-03 / SEP-2106

**Analog:** `tests/v2_tasks_tripwires.rs` (2083 lines, 25 tests). Take the *scanner primitives*
and the *justified-allowlist discipline*; the `Cargo.toml` half is new (§ No Analog Found).

**Restate-don't-share doctrine** (`v2_tasks_tripwires.rs:23-32`) — copy this rationale into the
new file's module doc:

```rust
//! # The scanner primitives are DELIBERATELY duplicated
//!
//! A Rust integration test is its own crate, so this file cannot import
//! `tests/v2_prohibited_error_codes.rs`'s scanner and that file cannot import
//! this one. The primitives below are therefore RESTATED rather than shared, and
//! the idiom is kept identical on purpose so the repository has one
//! source-scanning shape rather than three.
```

**Primitives to copy verbatim** (:82-152):

```rust
/// A justification shorter than this is a label, not a decision.
const MIN_JUSTIFICATION_CHARS: usize = 40;

fn repo_root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }
fn rel(path: &Path) -> String { … }
fn read(path: &str) -> String { … }
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) { … }   // recursive

/// Every `.rs` file under `src/`, discovered at RUNTIME with `read_dir` so a NEW
/// file cannot escape the scan by nobody remembering to add it.
fn src_files() -> Vec<PathBuf> {
    …
    assert!(
        files.len() > 50,
        "src/ carries well over fifty files; discovering {} means the walk is broken and every \
         check in this file would pass vacuously",
        files.len()
    );
    files
}
```

Also available and worth reusing if the SEP-2106 scan needs to ignore string literals /
comments: `Stripped` + `line_of` + `strip_keeping_literals` (:154-330).

**The two-kind justified-entry model** (:1063-1104) — this is the shape D-03 asks for:

```rust
/// Why an `INTERNAL_ERROR` in the tasks dispatch is not a v2 `NotFound`.
enum Disposition {
    V1Only { guard: &'static str, guard_site: &'static str },
    NotFoundRoutedElsewhere { guard: &'static str, guard_site: &'static str },
    /// **RECORDED GAP, not a clean bill of health.** See the entries' `why`.
    RouterLeg,
}

struct InternalErrorEntry {
    function: &'static str,
    hits: usize,                 // ← the MEASURED count; a second hit in an
    disposition: Disposition,    //   already-allowlisted function is the shape
    why: &'static str,           //   a regression takes
}

const INTERNAL_ERROR_SITES: &[InternalErrorEntry] = &[ … ];
```

For SCHM-01, the two kinds map onto: `PinnedByPolicy` (the `output_validation.rs` v2 site) and
`OutOfScopeAllowlisted` (the `pmcp-agent` `decide.rs:218` `validator_for`, per Assumption A4).
Give each a `hits: usize`.

**The allowlist-discipline helper** (:1821-1842) — copy verbatim:

```rust
/// Every entry in a justified allowlist carries a real, distinct reason.
///
/// Length alone is trivially defeated by padding; pairwise distinctness alone is
/// defeated by five one-word labels. Both together mean a copy-pasted or empty
/// justification fails.
fn assert_justifications(label: &str, entries: &[(&str, &str)]) {
    let mut seen: Vec<&str> = Vec::new();
    for (name, why) in entries {
        let why = why.trim();
        assert!(why.len() >= MIN_JUSTIFICATION_CHARS, "…");
        assert!(!seen.contains(&why), "… a copy-pasted reason is not a reason");
        seen.push(why);
    }
    assert!(!entries.is_empty(), "{label} is EMPTY, so every check keyed on it passes over nothing");
}
```

**Anti-vacuity test pattern** (:1854-1869) — a dedicated `#[test]` asserting the exclusions
themselves are load-bearing. Mirror it: assert the scan finds ≥3 `jsonschema` manifest lines and
≥1 `draft202012` call site, so a broken scanner fails instead of passing green.

**Reading a vendored file at RUNTIME, not `include_str!`** (:95-102) — reuse this reasoning if
the tripwire compares against the newly vendored core schema:

```rust
/// Deliberately `read_to_string` rather than `include_str!`: … `include_str!` bakes the bytes in
/// at COMPILE time. Reading it at runtime is what makes a re-vendoring … move this test without
/// anyone remembering to touch it.
const VENDORED_SCHEMA: &str = "schema/vendored/ext-tasks/schema.ts";
```

*(Counter-precedent, both are acceptable: `src/types/tasks.rs:1515` uses `include_str!` for the
in-module serde locks. Runtime read for integration tests; `include_str!` for in-module unit locks.)*

---

### `src/server/output_validation.rs` (service/validation, transform) — SCHM-01, D-01/D-02

**Analog:** itself (161 lines — read in full) plus the era-capture idiom from `core.rs:605-616`.

**Module doc that must be amended** (:1-13) — the sentence D-01 invalidates is on line 8:

```rust
//! The module compiles unconditionally so dispatcher call sites stay plain
//! one-liners; the `validation` feature gate lives INSIDE
//! [`warn_on_schema_mismatch`], which is a no-op without the feature.
//! Compiled validators are cached per schema (keyed by the schema's canonical
//! JSON text), so steady-state cost per call is one lookup plus a short-circuit
//! `is_valid` check — the error-message pass runs only on actual mismatch.
```

**House style to preserve — warn-only** (:5-6, `never an error result`) and the crate-internal
`pub(crate)` / clippy tug-of-war allow (:15-19):

```rust
// Why: same tug-of-war as `task_dispatch` — rustc's `unreachable_pub` demands
// pub(crate) on items in a crate-internal module, while clippy's
// `redundant_pub_crate` flags that as redundant inside a pub(crate) module.
// rustc wins; silence the clippy side.
#![allow(clippy::redundant_pub_crate)]
```

**The three functions the era must thread through** (signatures as-of today):

```rust
pub(crate) fn warn_on_schema_mismatch(tool: &str, schema: &Value, value: &Value)   // :26 — compiles on ALL targets
#[cfg(feature = "validation")]
pub(crate) fn schema_mismatch(schema: &Value, value: &Value) -> Option<String>     // :50
#[cfg(feature = "validation")]
fn cached_validator(schema: &Value) -> Result<Arc<jsonschema::Validator>, Arc<str>> // :76
```

⚠ `warn_on_schema_mismatch` compiles unconditionally (including wasm32). Any era parameter added
to it must be a type available on wasm — `Era` (`src/types/protocol/version.rs:43`) is, since
`version.rs` carries no `cfg`. Preserve the existing no-feature arm:

```rust
    #[cfg(not(feature = "validation"))]
    let _ = (tool, schema, value);
```

**The cache to widen** (:79-99) — the shape to keep (Arc-cached, error-cached, poison-recovering):

```rust
    type Cache = Mutex<HashMap<String, Result<Arc<jsonschema::Validator>, Arc<str>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();

    let key = schema.to_string();
    let cache = CACHE.get_or_init(Cache::default);
    // Why: a poisoned mutex here only means another thread panicked while
    // inserting; the map itself is still usable — recover rather than
    // propagate a panic out of a warn-only diagnostics path.
    let mut map = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.entry(key)
        .or_insert_with(|| {
            jsonschema::validator_for(schema)
                .map(Arc::new)
                .map_err(|e| Arc::from(e.to_string().as_str()))
        })
        .clone()
```

**Era-branch placement pattern** — do NOT drag `ProtocolContext` in. The repo's answer is
"capture the `Option<Era>` at the dispatcher and pass it down", already proven twice:

```rust
// src/server/core.rs:605-616 — the 114-12 precedent, verbatim
        // Capture the ERA-AWARE create trigger BEFORE `protocol_context` is moved
        // into `extra` below (plan 114-12). `CreateTrigger::resolve` is the ONE
        // place the era picks a trigger …
        #[cfg(not(target_arch = "wasm32"))]
        let create_trigger = crate::server::task_dispatch::CreateTrigger::resolve(
            protocol_context.as_ref().map(|ctx| ctx.era),
            req.task.is_some(),
            protocol_context.as_ref(),
        );
```

```rust
// src/server/mod.rs:2001 — the twin, and the era value is STILL LIVE at the
// validation call site (:2211) because Option<Era> is Copy
        let create_path_era = protocol_context.as_ref().map(|ctx| ctx.era);
```

**The two production call sites to change:**

```rust
// src/server/core.rs:811-816
        // A declared outputSchema means structuredContent is emitted below
        // (via widget enrichment or the schema bridge) — validate the value
        // against it regardless of which branch does the emitting.
        if let Some(schema) = tool_info.and_then(|i| i.output_schema.as_ref()) {
            crate::server::output_validation::warn_on_schema_mismatch(&req.name, schema, &value);
        }
```

```rust
// src/server/mod.rs:2207-2212
        if let Some(info) = self.tool_infos.get(&req.name) {
            if let Some(schema) = &info.output_schema {
                output_validation::warn_on_schema_mismatch(&req.name, schema, &result);
            }
```

**Existing in-module test block to extend** (:102-161) — gate and helper shape:

```rust
#[cfg(all(test, feature = "validation"))]
mod tests {
    use super::*;
    use serde_json::json;

    fn person_schema() -> Value { json!({ "type": "object", … "required": ["name"] }) }

    #[test]
    fn invalid_schema_yields_message() {
        let bad_schema = json!({ "type": 42 });
        let mismatch = schema_mismatch(&bad_schema, &json!({}))
            .expect("an uncompilable schema must be reported, not ignored");
        assert!(mismatch.contains("outputSchema"), "…: {mismatch}");
    }
```

The Finding-1 fence belongs here: a test whose schema literally declares
`"$schema": "http://json-schema.org/draft-07/schema#"` and must still reject a violating instance.

---

### `src/types/tools.rs` — SCHM-02 sibling constructor (D-06)

**Analog:** `CallToolResult::structured_with_text`, `tools.rs:652-672` — the *existing* sibling
of `structured`, which is exactly the precedent D-06 invokes.

```rust
    /// Create a structured success result with a distinct human-readable voice.
    ///
    /// Like [`structured`](Self::structured), but `content` carries `text`
    /// instead of the raw JSON serialization — mirroring the two-voice
    /// separation [`rejected`](Self::rejected) has on the error side.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::CallToolResult;
    /// use serde_json::json;
    ///
    /// let result = CallToolResult::structured_with_text(
    ///     json!({ "matches": 42 }),
    ///     "Found 42 matches.",
    /// );
    /// assert_eq!(result.structured_content, Some(json!({ "matches": 42 })));
    /// ```
    pub fn structured_with_text(value: Value, text: impl Into<String>) -> Self {
        Self::new(vec![Content::text(text.into())]).with_structured_content(value)
    }
```

And the one it must NOT change (`tools.rs:647-650`):

```rust
    pub fn structured(value: Value) -> Self {
        let text = value.to_string();
        Self::new(vec![Content::text(text)]).with_structured_content(value)
    }
```

**Field already permissive** (`tools.rs:564-565`) — confirms Finding 6, no guard to remove:

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
```

Every new public fn needs a rustdoc **doctest** (house rule; `structured`/`structured_with_text`
both carry one, and `make quality-gate` runs `cargo test --doc`).

---

### The six `CacheableResult` types (model, CRUD) — SCHM-03, D-07

**Analog for the additive-field-on-a-`#[non_exhaustive]`-result move:** `ReadResourceResult._meta`,
`src/types/resources.rs:369-388`. This is the Phase-113 precedent that already answered the semver
question in-tree:

```rust
    /// Optional per-result metadata (`_meta`).
    ///
    /// The explicit `rename` defeats the struct-level `rename_all = "camelCase"`
    /// (which would emit `meta`, not the MCP spelling — the D-113-A defect);
    /// `skip_serializing_if` keeps an absent value byte-identical to the
    /// pre-Phase-113 wire, so a v1 `resources/read` response is unchanged.
    ///
    /// … Adding it is additive rather than a major bump because this struct is
    /// `#[non_exhaustive]` — `cargo semver-checks`' `constructible_struct_adds_field`
    /// only fires on externally-constructible structs (contrast D-113-D, where the five
    /// list-request structs were NOT `#[non_exhaustive]` and the same edit was a 3.0).
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none", default)]
    #[allow(clippy::pub_underscore_fields)]
    pub _meta: Option<serde_json::Value>,
```

**The five identical target shapes** (all `#[non_exhaustive] + Default + rename_all = "camelCase"`,
all with a `skip_serializing_if` `next_cursor`, all with a `new()` + `with_next_cursor()` impl):

| Type | Struct | `impl` |
|---|---|---|
| `ListToolsResult` | `src/types/tools.rs:428-437` | `:439-453` |
| `ListResourcesResult` | `src/types/resources.rs:131-140` | `:142-156` |
| `ListResourceTemplatesResult` | `src/types/resources.rs:297-306` | `:308-322` |
| `ReadResourceResult` | `src/types/resources.rs:354-389` | `:391-…` |
| `ListPromptsResult` | `src/types/prompts.rs:244-253` | `:255-269` |
| **`ServerDiscoverResult`** (the sixth, Finding 5) | `src/types/protocol/mod.rs:618-630` | — (built by `discover_result_from_capabilities`, `core.rs:1267-1272`) |

Canonical shape to extend (`tools.rs:428-453`):

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult {
    /// Available tools
    pub tools: Vec<ToolInfo>,
    /// Pagination cursor for next page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
}

impl ListToolsResult {
    pub fn new(tools: Vec<ToolInfo>) -> Self { Self { tools, next_cursor: None } }
    /// Set the pagination cursor for the next page.
    pub fn with_next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }
}
```

`with_next_cursor` is the exact template for the discretionary `.with_ttl_ms(..)` /
`.with_cache_scope(..)` builders.

⚠ `ServerDiscoverResult` is the odd one out: `#[non_exhaustive]` but **no `Default`** and **no
`new()`** (`protocol/mod.rs:618-630`). Its only producer is
`discover_result_from_capabilities` (`core.rs:1267-1272`).

**Field-rustdoc + serde treatment analog:** `TaskV2::ttl_ms` / `poll_interval_ms`,
`src/types/tasks.rs:726-742` — the required-and-nullable vs genuinely-optional distinction
spelled out, with a schema line citation:

```rust
    /// Time-to-live from creation in integer milliseconds, `null` for unlimited.
    ///
    /// **RENAMED from the v1 `ttl`** — inventory row 8. It is **required AND
    /// nullable** (`schema.ts:79-84`, `$defs.Task.required[4]`), so it is
    /// deliberately modelled WITHOUT `skip_serializing_if`: `None` must
    /// serialize as `"ttlMs":null` (present), never be omitted. …
    pub ttl_ms: Option<u64>,
    /// Suggested polling interval in integer milliseconds.
    ///
    /// **RENAMED from the v1 `pollInterval`** — inventory row 9. Genuinely
    /// OPTIONAL: it is absent from every per-variant `required` array
    /// (`schema.ts:86-91`), so it carries `skip_serializing_if` and a `None`
    /// omits the key entirely.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval_ms: Option<u64>,
```

For SCHM-03 the correct treatment is the **`skip_serializing_if` (second) shape** — `None` means
"handler expressed no preference"; the v2 chokepoint injects the default. See Research § Finding 9.

---

### `CacheScope` enum (model, CRUD) — SCHM-03, D-09

**Analog:** `TaskStatus`, `src/types/tasks.rs:11-26`:

```rust
/// Task status (5-value enum).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is actively being worked on
    #[default]
    Working,
    /// Task requires user input to continue
    InputRequired,
    …
}

impl std::fmt::Display for TaskStatus { … }   // :28-…
```

Two deltas for `CacheScope`: `rename_all = "lowercase"` (not `snake_case` — the values are
`"public"`/`"private"`, single words, so both happen to agree, but state the intent), and
`#[default]` on `Private` rather than a hand-written `impl Default` (the `TaskStatus` precedent
uses the derive + attribute, which is the house form).

**Serde-lock test analog:** `src/types/tasks.rs:1503-1608`, module `v2_projection_tests` — the
114-03 pattern in full:

```rust
/// Unit locks for the v2 projection types (114-11, TASK-04).
///
/// The `required` key sets are read from the VENDORED schema at compile time
/// rather than restated, so a re-vendoring at the D-18 gate moves these
/// assertions automatically instead of leaving them asserting yesterday's
/// contract.
#[cfg(test)]
mod v2_projection_tests {
    use super::*;
    use serde_json::{json, Value};

    /// The vendored tasks-extension JSON Schema, embedded at compile time.
    const EXT_TASKS_SCHEMA_JSON: &str = include_str!("../../schema/vendored/ext-tasks/schema.json");

    /// The `required` array of a `$defs` entry, as a sorted `Vec<String>`.
    fn schema_required(def: &str) -> Vec<String> { … }

    #[test]
    fn v2_projection_uses_the_renamed_ttl_ms_and_poll_interval_ms_keys() {
        let mut task = minimal(TaskStatus::Working);
        task.poll_interval_ms = Some(2500);
        let raw = serde_json::to_string(&task).expect("serializes");
        assert!(raw.contains("\"ttlMs\":60000"), "the v2 projection must spell `ttlMs`, got {raw}");
        assert!(raw.contains("\"pollIntervalMs\":2500"), "…");
        // The v1 spellings must NOT appear: these are RENAMES, not additions.
        assert!(!raw.contains("\"ttl\":"), "v1 `ttl` leaked into v2: {raw}");
    }
```

For SCHM-03: `include_str!("../../schema/vendored/core-2026-07-28/schema.json")`, read
`$defs.CacheableResult.required`, and assert it equals the sorted `["cacheScope", "ttlMs"]` the
Rust side emits. That makes the serde lock *derive from the vendored artifact*, which is exactly
what D-14 buys.

---

### `src/server/core.rs:1561` — the D-12 chokepoint (middleware, request-response)

**Analog:** itself. It already has every property D-11/D-12 need.

```rust
/// Inject the v2-only response envelope (`resultType` + `serverInfo`) at the
/// era-gated serialization boundary (Phase 112, VERS-07 / D-07 / D-08).
///
/// This is the ONE shared implementation BOTH native dispatch sites
/// (`core.rs` and `server/mod.rs`) call — not a per-site copy. The envelope
/// model is pinned (Codex HIGH #5):
///
/// - era != V2 (or no resolved context) → response left BYTE-IDENTICAL to
///   today (the v1 promise — no key added, golden-fixtured).
/// - error responses / notifications (no `result`) → NO injection.
/// - `result` is a JSON object → the SERVER-OWNED reserved fields are asserted
///   over it by [`own_reserved_result_fields`] …
/// - `result` is scalar/array/null → left unchanged …
pub(crate) fn inject_v2_result_envelope(
    response: &mut JSONRPCResponse,
    protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    server_info: &Implementation,
    disposition: ResponseDisposition,
    owner: ReservedFieldOwner,
) {
    // v2-only: a v1 (or non-opted-in) response is left byte-identical.
    if !matches!(protocol_context.map(|c| c.era), Some(crate::types::protocol::Era::V2)) {
        return;
    }
    // Only success results carry the envelope; errors / notifications do not.
    let crate::types::jsonrpc::ResponsePayload::Result(ref mut value) = response.payload else {
        return;
    };
    // A non-object result (scalar/array/null) cannot carry a key — leave it.
    if !value.is_object() {
        return;
    }
    own_reserved_result_fields(value, server_info, disposition, owner);
}
```

**The "named rule rather than argument order" idiom for a new parameter** (`core.rs:1495-1512`) —
if the `Cacheable` discriminator becomes a claim rather than a bare bool, this is the shape:

```rust
    /// Fold the MRTR egress's own claim over this dispatch claim.
    ///
    /// The egress wins when it made a claim at all, because it PHYSICALLY
    /// rewrote the result body … On every non-MRTR response the egress returns
    /// exactly [`Self::NONE`], so the common path is a pass-through.
    pub(crate) fn or_egress(self, disposition: ResponseDisposition, owner: ReservedFieldOwner) -> Self {
```

**Existing test home** (`core.rs:4593-4597`) — the envelope has its OWN `mod` with ~15 call
sites so `cargo test -- inject_v2_result_envelope` selects them. Add SCHM-03 unit cases there.

**The four production call sites** — all must be considered:

| Site | What flows through | Cacheable? |
|---|---|---|
| `core.rs:1794` (inside `build_discover_response`) | `ServerDiscoverResult` | **always** — no discrimination needed |
| `core.rs:3241` (`ServerCore` dispatch) | all five list/read results | needs the captured discriminator |
| `mod.rs:1530` | `tasks/update` | never |
| `mod.rs:1705` (`Server` twin dispatch) | all five list/read results | needs the captured discriminator |

`core.rs:1794` excerpt (the easy one — hardcode `Cacheable::Yes` here):

```rust
    let result = discover_result_from_capabilities(capabilities, info, negotiated_version);
    let mut response = ServerCore::success_response(id, serde_json::to_value(result).unwrap());
    // Parity: the v2 object result carries resultType + serverInfo via the SAME
    // shared envelope helper every other v2 result uses. `server/discover` mints
    // no reserved MRTR/tasks field, so it owns none of them.
    inject_v2_result_envelope(
        &mut response,
        protocol_context,
        info,
        ResponseDisposition::Complete,
        ReservedFieldOwner::None,
    );
```

---

### `src/server/core.rs:3186-3247` + `src/server/mod.rs:1614-1712` — capture-before-move (controller)

**Analog:** the era capture at `core.rs:605-616` / `mod.rs:2001` (quoted above under
`output_validation.rs`) — same idiom, different value.

**`core.rs`: `request` is still borrowed at :3187, moved at :3207.** Capture between them:

```rust
        #[cfg(feature = "streamable-http")]
        let (mrtr, protocol_context) = match MrtrRound::begin(
            &request,                                   // ← still a BORROW at :3187
            protocol_context,
            …
        ) { … };

        // ← capture `cacheable` HERE, before the move below

        let mut dispatch_claim = DispatchEnvelopeClaim::NONE;
        let mut response = self
            .handle_request_internal(
                id.clone(),
                request,                                 // ← MOVED at :3207
                auth_context,
                protocol_context.clone(),
                &mut dispatch_claim,
            )
            .await;
        …
        let claim = dispatch_claim.or_egress(disposition, reserved_field_owner);
        inject_v2_result_envelope(&mut response, protocol_context.as_ref(), &self.info,
                                  claim.disposition, claim.owner);          // :3241
```

**`mod.rs` twin: the same shape, with `ref boxed_req` on the first arm and a move on the second**
(`mod.rs:1614-1654`):

```rust
        let mut dispatch_claim = crate::server::core::DispatchEnvelopeClaim::NONE;
        let mut response = match request {
            Request::Client(ref boxed_req) if matches!(**boxed_req, ClientRequest::Initialize(_)) => { … },
            Request::Client(boxed_req) => {              // ← MOVED here
                Box::pin(self.handle_client_request(id, *boxed_req, auth_context,
                                                    protocol_context.clone(), &mut dispatch_claim)).await
            },
            …
        };
        …
        // Twin-site v2 envelope injection (VERS-07 / D-07 / D-08): the ONE shared
        // helper in `core.rs` — v2-only, object-results-only, collision-safe;
        // v1 / non-opted-in responses stay byte-identical. …
        let claim = dispatch_claim.or_egress(disposition, reserved_field_owner);
        crate::server::core::inject_v2_result_envelope(
            &mut response, protocol_context.as_ref(), &self.info, claim.disposition, claim.owner,
        );                                                                        // :1705
```

**Twin-site parity doctrine** (`core.rs:1804-1810`) — restate it in whatever helper the plan adds:

```rust
// ONE shared unit, called from BOTH native dispatch sites — `ServerCore` below
// and the high-level `Server` in `server/mod.rs`. That is the Phase-109/112
// twin-site parity rule: `mod.rs` CALLS these helpers, it never defines its own.
```

---

### `tests/v2_caching_hints.rs` (test/integration, request-response) — SCHM-03

**Analog A — twin-dispatcher test naming** (`tests/structured_tool_output.rs:136-220`). This is
the Pitfall-6 fence: every behavior gets a `server_*` and a `server_core_*` test.

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_auto_emits_structured_content_for_declared_output_schema() {
    let server = Server::builder().name("structured-output-server").version("1.0.0")
        .tool("propose_schema", propose_schema_tool()).build().expect("server builds");
    let result = call_via_server(server, "propose_schema", json!({ "corpus": "docs" })).await;
    assert_eq!(result.structured_content, Some(expected_proposed_schema()),
        "high-level Server bridges declared outputSchema to structuredContent");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_core_auto_emits_structured_content_for_declared_output_schema() {
    let core: Arc<dyn ProtocolHandler> = Arc::new(
        ServerCoreBuilder::new().name("structured-output-core").version("1.0.0")
            .tool("propose_schema", propose_schema_tool()).build().expect("core builds"));
    let result = call_via_core(core, "propose_schema", json!({ "corpus": "docs" })).await;
    assert_eq!(result.structured_content, Some(expected_proposed_schema()), "…");
}
```

Its harness import (`:25-33`):

```rust
#![cfg(all(not(target_arch = "wasm32"), feature = "schema-generation"))]

#[path = "common/duplex.rs"]
mod duplex;

use duplex::{call_via_core, call_via_server};
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
```

**Analog B — real-HTTP v2 harness** for the on-the-wire assertions (`tests/common/v2.rs`):
`spawn_stateless_config` (:389), `post` (:801), `v2_body` (:553), `v2_body_with_caps` (:569),
`v2_headers` (:688), `teardown` (:528), `Resp` (:717), `V2`/`V1` consts (:66/:72). Use
`v2_body` for the era-2 requests and `v1_body` for the byte-identity control.

---

### `examples/sNN_v2_caching_hints.rs` (example) — ALWAYS requirement

**Analog:** `examples/s47_v2_stateless_mrtr.rs` (module doc :1-57 — the house form for a v2
example) plus its `Cargo.toml` registration (`Cargo.toml:604-607`):

```rust
//! Example: STATELESS MCP 2026-07-28 server doing MULTI-ROUND-TRIP ELICITATION
//!
//! Run this server with:
//! ```bash
//! cargo run --example s47_v2_stateless_mrtr --features full
//! ```
//!
//! # What this demonstrates
//!
//! - **No `initialize` handshake.** …
//! - **A handler that asks for more input.** …
```

```toml
# Cargo.toml:604-607 — EVERY example needs this block; `required-features` is what
# keeps `cargo test --examples` on a default build from failing.
[[example]]
name = "s47_v2_stateless_mrtr"
path = "examples/s47_v2_stateless_mrtr.rs"
required-features = ["streamable-http", "testing"]
```

Numbering note recorded in-tree at `Cargo.toml:595-599`: example NAMES are unique, numeric
prefixes may collide; pick the next free `sNN_v2_*` name and say why in a comment if it collides.

---

### `tests/property_tests.rs` (test/property) — ALWAYS requirement

**Analog A — integration property tests** (`tests/property_tests.rs:1-48`):

```rust
//! Property-based tests for PMCP SDK
//! ALWAYS Requirement: Property tests for all new features

use pmcp::types::*;
use proptest::prelude::*;

#[cfg(test)]
mod protocol_invariants {
    use super::*;
    proptest! {
        /// Property: JSON-RPC serialization round-trip should preserve data
        #[test]
        fn property_jsonrpc_roundtrip(…) {
            …
            prop_assert_eq!(request.method, deserialized.method);
        }
    }
}
```

**Analog B — in-module proptests beside the type** (`src/types/tasks.rs:1474-1500`), which is
the better home for `CacheScope` serde round-trip and `$schema`-normalization idempotence:

```rust
    proptest::proptest! {
        /// For every TaskStatus and any poll_interval, poll_decision() returns
        /// exactly the mapped variant — the classifier never drifts.
        #[test]
        fn poll_decision_matches_expected_map(
            status_idx in 0usize..ALL_STATUSES.len(),
            poll_interval in proptest::option::of(proptest::prelude::any::<u64>()),
        ) {
            …
            proptest::prop_assert_eq!(task.poll_decision(), expected_decision(status, poll_interval));
        }
    }
```

---

## Shared Patterns

### 1. Era gating (applies to: `output_validation.rs`, all six result types, both dispatchers)

**Source:** `src/types/protocol/version.rs:35-71`
**Apply to:** every D-01 / D-11 / D-12 branch. Do not invent a second classifier.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Era {
    /// The 2024/2025 protocol generation (compatibility layer, current default).
    V1,
    /// The 2026-07-28 protocol generation (opt-in, stateless-first).
    V2,
}

/// Returns [`Era::V2`] **only** for [`PROTOCOL_VERSION_2026_07_28`] … This
/// conservative unknown-to-`V1` fallback guarantees that only an exact,
/// deliberate `2026-07-28` negotiation reaches v2 behavior.
pub fn protocol_era(version: &str) -> Era { … }
```

The gate expression used everywhere in the server:

```rust
if !matches!(protocol_context.map(|c| c.era), Some(crate::types::protocol::Era::V2)) {
    return;
}
```

Era capture idiom (`Option<Era>` is `Copy`, so capture once, use anywhere downstream):

```rust
let era = protocol_context.as_ref().map(|ctx| ctx.era);
```

### 2. Era-projected wire shape at ONE shared point (applies to: SCHM-03)

**Source:** `src/server/core.rs:1210-1231` (`project_capabilities_for_v1`) — the 114-05 pattern
D-12 names.
**Apply to:** the caching-hint projection.

```rust
pub(crate) fn project_capabilities_for_v1(capabilities: &ServerCapabilities) -> ServerCapabilities {
    let auto_advertised = crate::server::task_dispatch::tasks_extension_value();
    let mut projected = capabilities.clone();
    // ONE pass, ONE clone, mirroring [`project_capabilities_for_v2`] above. …
    …
    projected
}
```

Its rustdoc carries the anti-pattern warning worth restating for SCHM-03
(`core.rs:1243-1266`):

```rust
/// # The projection is PER-REQUEST-ERA and MUST NOT mutate stored capabilities
/// … A server's `capabilities` are per-SERVER while the projection is per-REQUEST-ERA: one
/// pmcp binary serves both eras, so mutating the stored struct here would make
/// the first v2 `server/discover` permanently change what every subsequent v1
/// `initialize` client sees. That is a cross-request state leak, not an
/// optimisation …
///
/// The anti-pattern this deliberately avoids: doing the suppression as a serde
/// change in `src/types/capabilities.rs`. That would alter the `initialize`
/// bytes of every existing tasks server on every era …
```

### 3. Justified-allowlist tripwire discipline (applies to: D-03, optional D-10 tripwire)

**Source:** `tests/v2_tasks_tripwires.rs:82` + `:1821-1842` + `:1854-1869`
**Apply to:** every new tripwire in this phase.

Three non-negotiables: `MIN_JUSTIFICATION_CHARS = 40`; pairwise-distinct `why` strings; and a
dedicated anti-vacuity test that fails if the scanner finds nothing.

### 4. Runtime discovery over hard-coded lists (applies to: all tripwires)

**Source:** `tests/v2_tasks_tripwires.rs:126-152`, `tests/vendored_schema_provenance.rs:101-120`
**Apply to:** the SEP-2106 manifest scan and the generalized provenance scan.

```rust
/// Every `.rs` file under `src/`, discovered at RUNTIME with `read_dir` so a NEW
/// file cannot escape the scan by nobody remembering to add it.
```

### 5. Failure messages that name the remedy (applies to: every new assertion)

**Source:** `tests/vendored_schema_provenance.rs:268-287`, `tests/v1_tasks_golden.rs:254-259`
**Apply to:** all Phase 115 tests.

```rust
fn wire_break_message(raw: &str) -> String {
    format!(
        "v1 tasks wire bytes changed. This is a V1 WIRE BREAK, not a stale fixture — \
         make the change v2-only instead of re-recording the golden. Raw response was: {raw}"
    )
}
```

The provenance file's `FAILURE MODE N` + `WHAT TO DO:` structure is the fuller form.

### 6. Feature-gated wasm cleanliness (applies to: the jsonschema bump)

**Source:** `Cargo.toml:135` + `:198`
**Apply to:** all three manifests.

```toml
# Validation (optional, feature-gated)
jsonschema = { version = "0.46", optional = true, default-features = false }
…
validation = ["dep:jsonschema", "dep:garde"]
```

Both `optional` and `default-features = false` are load-bearing — see Research § Finding 2.

### 7. Dependency-boundary gate via `cargo tree` (applies to: SEP-2106, secondary to the tripwire)

**Source:** `Makefile:505-534` (`purity-check`)
**Apply to:** an optional Makefile-level fence for `reqwest` under `--features validation`.

```make
	@cargo metadata --format-version 1 >/dev/null 2>&1 || { echo "purity-check FAILED: could not resolve Cargo.lock (failing closed)"; exit 1; }
	@set -euo pipefail; \
	BAN='umya|calamine|quick-xml|swc_|pmcp-code-mode'; \
	for crate in $(PURITY_CRATES); do \
	  for feat in "" "--no-default-features" "--all-features"; do \
	    status=0; tree=$$(cargo tree -p $$crate $$feat 2>&1) || status=$$?; \
	    if [ $$status -ne 0 ]; then echo "… failing closed"; exit 1; fi; \
	    if printf '%s\n' "$$tree" | grep -Ei "$$BAN"; then echo "… boundary is breached"; exit 1; fi; \
```

Note the fail-closed `cargo metadata` warm-up and the "unpinned tooling drift" caution — this
gate has bit-rotted before (see memory: *CI Purity Gate unpinned-tooling drift*).

---

## Structural Findings That Change Plan Shape

Discovered while mapping analogs. Each is a measured fact with a file:line.

**F1 — `Era` does not derive `Hash`.** `src/types/protocol/version.rs:42`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Era {
```

Research § Pattern 2 prescribes `HashMap<(Era, String), …>`. That does not compile today. The
plan must either add `Hash` to the derive (a public-API addition — additive, but a real edit to a
Phase-112 type) or key on something else (e.g. `(&'static str, String)` from a `match era`).
**Surface the choice; do not let it be discovered at build time.**

**F2 — the era is ALREADY in scope at both validation call sites, in exactly the right shape.**
`core.rs:585` takes `protocol_context: Option<ProtocolContext>` and captures
`protocol_context.as_ref().map(|ctx| ctx.era)` at `:615`; `mod.rs:2001` binds
`let create_path_era = protocol_context.as_ref().map(|ctx| ctx.era);` and it is still live at the
`:2211` validation call (`Option<Era>` is `Copy`; the only other use is `:2162`). D-01's "where
does the branch live without dragging `ProtocolContext` through" question is therefore already
answered by the tree: **thread `Option<Era>`, capture at the dispatcher.**

**F3 — ~24 in-crate struct-literal construction sites break when the six result types gain fields.**
None use `..Default::default()`. Measured sites:

| File | Lines |
|---|---|
| `src/server/core.rs` | 574, 860, 919, 993, 1272, 5600 |
| `src/server/core_tests.rs` | 134 |
| `src/server/mod.rs` | 1896, 2249, 2371, 2462, 5282, 5851 |
| `src/server/workflow/prompt_handler.rs` | 1952, 2086, 2222 |
| `src/server/simple_resources.rs` | 322, 347, 376 |
| `src/server/wasm_server.rs` | 141, 237, 260 |
| `src/server/wasm_server_tests.rs` | 225, 230 |
| `src/server/traits.rs` | 28 — ⚠ **ORPHAN**, no `mod traits` declaration; never compiled (recorded in project memory) |

Example (`core.rs:574`):

```rust
        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
```

Precedent for absorbing this: Phase 113 added `_meta` to `ReadResourceResult` and updated every
literal (`simple_resources.rs:322` now reads `ReadResourceResult { contents: …, _meta: None }`).
Budget the same. **`src/server/wasm_server.rs` is a THIRD dispatcher** beyond the `ServerCore`/`Server`
twins — it constructs three of the six result types (`:141`, `:237`, `:260`) but is *not* on the
`inject_v2_result_envelope` chokepoint. That is a fourth thing to decide, not covered by Pitfall 6.

**F4 — `ServerDiscoverResult` has no `Default` and no `new()`** (`protocol/mod.rs:618-630`), unlike
its five siblings. Its single producer is `discover_result_from_capabilities`
(`core.rs:1267-1272`). Adding fields there is one site, but the builder-method pattern
(`with_next_cursor`) has no home on this type.

**F5 — the `jsonschema` manifest population is exactly three lines** (measured with
`grep -rn jsonschema --include=Cargo.toml`), which fixes the D-03 anti-vacuity floor at `>= 3`:

```
./Cargo.toml:135:jsonschema = { version = "0.46", optional = true, default-features = false }
./crates/pmcp-server-toolkit/Cargo.toml:54:jsonschema = { version = "0.46", default-features = false, optional = true }
./crates/pmcp-agent/Cargo.toml:27:jsonschema = { version = "0.46", default-features = false }
```

Note `pmcp-server-toolkit` writes the keys in a **different order**
(`default-features` before `optional`) — a naive substring scan for the exact root spelling would
miss it. `crates/pmcp-agent/Cargo.toml:24` already carries a comment about literal pins ("the repo
has NO `[workspace.dependencies]` table"), which is context the tripwire's justification should cite.

**F6 — `tests/common/` is a shared harness with three entry shapes.** `tests/common/mod.rs` (297 B)
plus `duplex.rs` (in-process, `call_via_server`/`call_via_core`), `v2.rs` (real loopback HTTP,
36.5 KB), `mock_paginated.rs` (pagination mocks). Pick `duplex` for type-level round-trips and
`v2` for byte-level golden/wire assertions — `v1_tasks_golden.rs` uses `v2.rs`;
`structured_tool_output.rs` uses `duplex.rs`.

---

## No Analog Found

| File / concern | Role | Data Flow | Reason |
|---|---|---|---|
| The `Cargo.toml`-scanning half of `tests/v2_schema_tripwires.rs` (D-03, Finding 2) | test (tripwire) | file-I/O | **No test in the repo scans workspace manifests.** The three `Cargo.toml`-reading tests (`v2_conformance_pin.rs`, `v2_mrtr.rs`, `vendored_schema_provenance.rs`) reference it only in comments about the `exclude` list. The closest real dependency-boundary gate is `Makefile:505-534` `purity-check`, which shells `cargo tree` rather than parsing manifests. Reuse `v2_tasks_tripwires.rs`'s `repo_root()` / `rel()` / recursive-discovery primitives and write the manifest reader fresh — Research § Code Example 2 gives a working sketch, and Assumption A8 already books the "is this in the 114-16 instrument's spirit?" question. |
| The `$schema` normalization policy fn (`compile_2020_12`) | service | transform | Genuinely new — three lines of policy with no in-tree precedent (Research says so explicitly: *"one genuinely new three-line policy"*). Use Research § Pattern 1 verbatim; house it in `output_validation.rs` beside `cached_validator` so the policy is visible in our source, not the validator's. |

---

## Metadata

**Analog search scope:** `src/types/`, `src/server/`, `tests/`, `tests/common/`, `examples/`,
`schema/vendored/`, all workspace `Cargo.toml` files, `Makefile`

**Files read in full:** `src/server/output_validation.rs`, `src/types/protocol/version.rs`,
`schema/vendored/ext-tasks/PROVENANCE.md`, `tests/vendored_schema_provenance.rs`,
`tests/structured_tool_output.rs`

**Files read in targeted ranges:** `tests/v1_tasks_golden.rs`, `tests/v2_tasks_tripwires.rs`,
`src/server/core.rs`, `src/server/mod.rs`, `src/types/tools.rs`, `src/types/resources.rs`,
`src/types/prompts.rs`, `src/types/protocol/mod.rs`, `src/types/tasks.rs`,
`tests/property_tests.rs`, `examples/s47_v2_stateless_mrtr.rs`, `Cargo.toml`, `Makefile`

**Pattern extraction date:** 2026-07-31
