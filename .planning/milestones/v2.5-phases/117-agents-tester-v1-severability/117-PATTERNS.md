# Phase 117: Agents, Tester & v1 Severability - Pattern Map

> **⚠ SUPERSEDED NAMES — the plans are authoritative, not this document.** Two artifact names in this
> pattern map were drafted before the naming decisions were finalized:
>
> - `crates/mcp-tester/baselines/era-deltas.toml` → **`era-deltas.yaml`**. `crates/mcp-tester/Cargo.toml`
>   has **no** `toml` dependency but already carries `serde_yaml = "0.9"` (used by
>   `src/scenario.rs:234-249`); adding a dependency to a published 0.7.0 crate was rejected. Plan
>   117-08 ships YAML.
> - `examples/s49_v2_agent_client.rs` → **`examples/s53_v2_agent_client.rs`**. `s49` is occupied twice
>   and `s50`/`s51`/`s52` exist. Plan 117-10 ships `s53`, at the **repo root** so `make test-examples`
>   (which globs root `examples/*.rs` only) actually builds it.
>
> Every other reference to `.toml` or `s49` below should be read with these substitutions applied.



**Mapped:** 2026-08-07
**Files analyzed:** 14 new · 16 modified
**Analogs found:** 12 exact/role-match / 14 new files

> Every analog below was READ in this session. Line ranges are real. Where a claimed pattern does
> **not** exist in this repo, it is called out in § No Analog Found rather than invented — three of
> them are load-bearing for the plan's risk assessment.

---

## File Classification

### New files

| New file | Role | Data flow | Closest analog | Match |
|---|---|---|---|---|
| `tests/v1_severability_tripwire.rs` | test (source tripwire) | file-I/O + transform | `tests/v2_bounded_reads_tripwire.rs` | **exact** |
| `tests/v1_byte_identity_after_cut.rs` | test (golden, live socket) | request-response | `tests/v1_lists_golden.rs` | **exact** |
| `src/server/streamable_http_server/v1_session.rs` | module (transport internals) | request-response + session state | in-file `:405-564` gates (moving, not copying) | **exact (it is the source)** |
| `src/server/streamable_http_server/v1_session_off.rs` | module (null twin) | constant-answer | `src/server/mod.rs:224-234` (wasm ZST stub) | **partial — see § No Analog Found #2** |
| `docs/v1-sunset-policy.md` | docs (normative prose) | n/a | `docs/protocol-compatibility.md`, `docs/MIGRATION.md` | role-match |
| `crates/mcp-tester/baselines/era-deltas.toml` | data fixture | file-I/O | `contracts/binding.yaml` + `crates/pmcp-cfn-renderer/tests/goldens/*.json` | role-match |
| `crates/mcp-tester/tests/era_baseline.rs` | test (data-file schema tripwire) | file-I/O | `tests/phase115_contract_bindings.rs` | **exact** |
| `crates/mcp-tester/tests/report_compat.rs` | test (golden stdout) | transform | `tests/v1_lists_golden.rs` (normalization) + `report.rs:244` writer seam | **exact** |
| `crates/mcp-tester/tests/dual_run.rs` | test (live socket) | request-response | `crates/mcp-tester/tests/transport_conformance_integration.rs` | **exact** |
| `crates/mcp-tester/src/era_diff.rs` | module (report/domain) | transform + file-I/O | `crates/mcp-tester/src/post_deploy_report.rs` | **exact** |
| `crates/pmcp-agent/tests/agent_v2_e2e.rs` | test (live socket) | request-response | `crates/pmcp-agent/tests/http_sources_mock.rs` + `tests/common/v2.rs` | role-match — see § No Analog Found #4 |
| `examples/s49_v2_agent_client.rs` | example (runnable) | request-response | `examples/s48_v2_mrtr_client.rs` (+ `crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs`) | **exact** |
| `fuzz/fuzz_targets/<name>.rs` | fuzz target | transform | `fuzz/fuzz_targets/dcr_response_parser.rs` | **exact** |
| `.github/workflows/ci.yml` → job `v1-severance` | CI job | batch | `ci.yml:311-345` (`purity-check`) | **exact** |

### Modified files

| Modified file | Role | What changes | Pattern source |
|---|---|---|---|
| `Cargo.toml:203-205` | config | `+v1-compat` in `default`/`full`; new `full-v2` | `Cargo.toml:236-243` (`fuzzing` — a dependency-free marker feature with a written "NOT in default, NOT in full" rationale) |
| `.github/workflows/ci.yml:441-462` | config (CI) | **3 edits**: `needs:` `:443`, `env:` `:447-452`, `if` chain `:454-461` | shown verbatim below |
| `Makefile:426-430` | config | `+v1-compat` to `doc-check` feature list | shown below |
| `src/server/streamable_http_server.rs` | transport | state collapse + symbol move + verb split | shown below |
| `src/shared/mod.rs:33,:128-131` | module decl | whole-file gate on `event_store` | `src/shared/mod.rs:121-123` (`streamable_http` gate) |
| `src/shared/http_constants.rs:34` | constants | per-const gate on `LAST_EVENT_ID` | (module doc `:4-8` says "deliberately UNGATED" — gate the const, not the module) |
| `src/shared/streamable_http.rs:639` | client transport | the client-side `LAST_EVENT_ID` reader; must be gated in the SAME edit | — |
| `crates/pmcp-agent/src/invoker/factory.rs:125-146` | factory | two-attempt era-pinned construction | shown below |
| `crates/pmcp-agent/src/trace.rs:32-43,:163-182` | model + seam | additive era field + mismatch check | shown below |
| `crates/mcp-tester/src/conformance/mod.rs:66-134` | orchestrator | `run_dual` wraps `run` | shown below |
| `crates/mcp-tester/src/tester.rs:79-88` | client | **add builder**, do NOT widen `new` | — |
| `crates/mcp-tester/src/main.rs:125-136` | CLI | `--dual-run` flag on `Conformance` | shown below |
| `crates/mcp-tester/src/lib.rs` | barrel | export `era_diff` types | — |
| `crates/mcp-tester/Cargo.toml:20-40` | config | **`toml` dep is MISSING** — see § No Analog Found #3 | — |
| `Cargo.toml` `[dev-dependencies]` + `[[example]]` | config | `pmcp-agent` dev-dep + example block | `Cargo.toml:198-201`, `:627-635` |
| `fuzz/Cargo.toml` | config | new `[[bin]]` block | `fuzz/Cargo.toml:223-228` |

---

## Pattern Assignments

### `tests/v1_severability_tripwire.rs` (test / source tripwire, file-I/O)

**Analog:** `tests/v2_bounded_reads_tripwire.rs` (1,279 lines) — the canonical derived-scope tripwire.

**Repo-root + relative-path helpers** (`tests/v2_bounded_reads_tripwire.rs:144-168`) — copy verbatim;
every tripwire in this repo starts here, and `rel()` is what makes failure messages actionable:

```rust
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
```

**DERIVED scope + non-vacuity guard** (`:170-197`) — **this is the CONTEXT.md "scope must be derived,
never enumerated" rule in code.** Note the two-layer guard: `!files.is_empty()` AND a
`REQUIRED_FILES` membership check, because an empty `read_dir` and a *silently shrunk* `read_dir`
are different failures:

```rust
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
    let discovered: BTreeSet<String> = files.iter().map(|p| rel(p)).collect();
    for required in REQUIRED_FILES {
        assert!(
            discovered.contains(*required),
            "scope discovery lost {required}; discovered: {discovered:?}\n    \
             REQUIRED_FILES holds FULL RELATIVE PATHS. ..."
        );
    }
    files
}
```

**Why the scope constant carries its own rationale** (`:84-101`) — the 116-14 lesson is written INTO
the constant, not into a plan. Copy this discipline for `full`/`full-v2`:

```rust
/// The directory walked at runtime, so a NEW file cannot escape the scan by
/// nobody remembering to add it here. Losing coverage by omission is exactly
/// how this requirement reopened three times.
const SHARED_DIR: &str = "src/shared";
```

**Two-kind justified-allowlist entry model** (`:638-641`, `:803`, `:821-826`) — the 114-16
refinement CONTEXT.md names. `Allowed` is the *structural exemption*; `Accumulation` is the
*change detector* keyed on a COUNT, deliberately not a line number:

```rust
/// A reviewed exemption from the whole-body-read rule.
struct Allowed {
    path: &'static str,
    needle: &'static str,
    why: &'static str,
}

/// An exemption shorter than this is a label, not a justification.
const MIN_JUSTIFICATION_CHARS: usize = 40;

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
```

**The justification is itself tested** (`:778-790`):

```rust
#[test]
fn every_whole_body_exemption_carries_a_substantive_justification() {
    for entry in WHOLE_BODY_ALLOWLIST {
        assert!(
            entry.why.trim().len() >= MIN_JUSTIFICATION_CHARS,
            "WHOLE_BODY_ALLOWLIST entry {}/{} needs a real justification naming why this read \
             cannot be bounded, not {:?}",
            entry.path, entry.needle, entry.why
        );
    }
```

**⚠ MANIFEST-PARSING DISCIPLINE — read before writing the `full`/`full-v2` check.**
`tests/v2_schema_tripwires.rs:26-42` records this repo's hard-won rule, and RESEARCH § Q3.4's
sketch (which reads `Cargo.toml` as text via `toml::from_str`) sits on the *safe* side of it only
because `[features]` values are literal arrays with no rename/inheritance mechanism:

```rust
//! # Manifests are NEVER read as text
//!
//! The pre-review shape of this file scanned `Cargo.toml` dependency LINES with
//! string matching. That misses a table-style declaration, a multiline
//! declaration, a dependency renamed via `package = "jsonschema"`, and any
//! future `[workspace.dependencies]` inheritance. This file parses cargo's own
//! output instead, in two layers:
//!
//! 1. `cargo metadata --no-deps` -> every workspace package's DECLARED
//!    dependency, with `rename`, `optional`, `uses_default_features` and
//!    `features` as structured fields;
//! 2. `cargo metadata --features validation` -> the RESOLVED graph's
//!    `resolve.nodes[].features` ...
//!
//! This needs no new dependency: `std::process::Command` plus `serde_json`.
```

Planner call: `toml::from_str` on `[features]` is acceptable (it PARSES rather than string-matches),
and `toml = "1.0"` is already a root runtime dep at `Cargo.toml:76`. If a stronger form is wanted,
`cargo metadata --no-deps` reports `packages[].features` as a structured map — same zero-new-deps
cost, and it is the shape this repo has already ruled correct.

**For the v1-module source-content check**, the same file's stripping discipline applies
(`v2_bounded_reads_tripwire.rs:62-67`): scan **stripped** source so a doc comment mentioning
`sessions` in prose is not a hit, and unit-test the stripper so over-stripping cannot make the
check vacuous.

---

### `tests/v1_byte_identity_after_cut.rs` (test / golden, request-response)

**Analog:** `tests/v1_lists_golden.rs` (833 lines) — captured 2026-08-01 for exactly this purpose.
Its own header already says it "outlives Phase 115" and is "the severability precedent for
Phases 116-119".

**Feature gate + shared harness import** (`:61-71`):

```rust
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{post, spawn_stateless_config, v1_body, Resp};
```

**The width-preserving dynamic-value normalizer** (`:97-127`) — the ONLY permitted normalization,
and the instrument you will need for `Mcp-Session-Id` (minted per run) in the session-header fixture:

```rust
/// A response value that cannot be pinned because it is minted per run.
struct DynamicField {
    /// The JSON object key whose STRING value is dynamic.
    key: &'static str,
    /// The canonical placeholder written into the golden literal.
    token: &'static str,
    /// Shape predicate the raw value must satisfy — a normalization that
    /// accepted any string would let a reshaped value through unnoticed.
    shape: fn(&str) -> bool,
    /// Human-readable form of `shape`, for the failure message.
    shape_description: &'static str,
}
```

**The golden record + four-step assertion** (`:219-283`) — copy this whole shape:

```rust
/// One pinned v1 response.
struct V1Golden<'a> {
    id: i64,
    raw: &'a str,
    result: Value,
    dynamics: &'a [DynamicField],
    meta: MetaExpectation,
}

fn assert_v1_bytes(raw: &str, golden: &V1Golden<'_>) {
    let same_width = substitute(raw, golden.dynamics, true);
    assert_eq!(
        same_width.len(), raw.len(),
        "the placeholder substitution changed the response length; it must be \
         width-preserving so it cannot mask an added or removed byte: {raw}"
    );
    for field in golden.dynamics {
        assert_eq!(
            key_occurrences(&same_width, field.key),
            key_occurrences(raw, field.key),
            "the substitution changed how often `{}` appears; it must replace \
             VALUES only and never delete a key: {raw}", field.key
        );
    }
    let normalized = substitute(raw, golden.dynamics, false);
    assert_eq!(normalized, golden.raw, "{}", wire_break_message(raw));
    // ... structural compare + v1_leak_guard + assert_meta
}
```

**The failure message that names the correct remedy** (`:205-215`) — this is what stops a future
contributor re-recording the golden:

```rust
fn wire_break_message(raw: &str) -> String {
    format!(
        "v1 list/read wire bytes changed. This is a V1 WIRE BREAK, not a stale fixture — \
         make the change v2-only instead of re-recording the golden. Raw response was: {raw}"
    )
}
```

**Spawn / teardown / round-trip trio** (`:452-478`):

```rust
async fn spawn(server: Server) -> (SocketAddr, JoinHandle<()>) {
    spawn_stateless_config(server).await
}

async fn shutdown(handle: JoinHandle<()>) {
    common::v2::teardown(handle, ()).await;
}

async fn round_trip(id: i64, method: &str, params: Value) -> Resp {
    let (addr, handle) = spawn(pinned_server()).await;
    let response = post(addr, &[], &lists_body(id, method, params)).await;
    shutdown(handle).await;
    response
}
```

⚠ **For a SESSION golden, use `spawn_default_config` (`tests/common/v2.rs:383-386`), not
`spawn_stateless_config` (`:389-391`).** `stateless()` has no `session_id_generator`, so
`sessions_active_for(false, _) == false` and there is no `Mcp-Session-Id` to pin. `teardown`
(`tests/common/v2.rs:528-532`) is mandatory — the drop-sockets-then-abort-then-await ORDER is
D-113-T and a bare `abort()` produces intermittent nextest `LEAK` noise.

**GET/`Last-Event-ID` replay fixture**: `tests/common/v2.rs` already supplies `get(addr, extra)`
(`:835`) and `delete(addr, extra)` (`:843`) plus `header(name, value)` (`:703`).

---

### `src/server/streamable_http_server/v1_session.rs` + `v1_session_off.rs` (module pair)

**Analog for the CONTENT:** the symbols being moved, all read this session.

**The seven chokepoints to move** (`src/server/streamable_http_server.rs:416-474`, `:511-564`).
The `const fn` pure-rule / `fn` state-reader split must survive the move intact — the null twin
must keep the SAME signature including the ignored `era` parameter (RESEARCH anti-pattern:
"they take the `era` argument and ignore it, they do not drop it from the signature"):

```rust
const fn sessions_active_for(
    cfg_has_generator: bool,
    era: Option<crate::types::protocol::Era>,
) -> bool {
    !matches!(era, Some(crate::types::protocol::Era::V2)) && cfg_has_generator
}

/// Are sessions live for this request? THE single reader of
/// `config.session_id_generator`'s presence.
fn sessions_active(state: &ServerState, era: Option<crate::types::protocol::Era>) -> bool {
    sessions_active_for(state.config.session_id_generator.is_some(), era)
}

/// The second (and last) permitted reader of `config.session_id_generator`
fn active_session_generator(
    state: &ServerState,
    era: Option<crate::types::protocol::Era>,
) -> Option<&(dyn Fn() -> String + Send + Sync)> {
    if !sessions_active(state, era) { return None; }
    state.config.session_id_generator.as_deref()
}

/// The ONE place a `Mcp-Session-Id` response header is emitted.
fn apply_session_header(
    headers: &mut HeaderMap,
    response_session_id: Option<&String>,
    sessions_on: bool,
) { /* :460-474 */ }
```

**The file hands severance to this phase by name** (`:493-497`) — quote it in the plan; the
comment must be UPDATED (not deleted) when the move lands:

```rust
// SEVERABILITY (CONTEXT.md "Claude's Discretion", lighter option taken): the
// [EventStore] trait, [InMemoryEventStore], the LAST_EVENT_ID constant and
// the whole v1 replay path are left FULLY INTACT. Deleting them is a Phase-117 /
// SMPL-01 severability concern, not this phase's; removing them now would
// maximize v1 blast radius for zero v2 benefit.
```

**Step 1 — the `ServerState` collapse.** Current shape (`:271-291`) and its single constructor
(`:303-330`):

```rust
#[derive(Clone)]
pub(crate) struct ServerState {
    server: Arc<tokio::sync::Mutex<Server>>,
    config: Arc<StreamableHttpServerConfig>,
    allowed_origins: AllowedOrigins,
    sse_streams: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<TransportMessage>>>>,  // v1-ONLY
    sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,                                 // v1-ONLY
    event_store: Option<EventStoreHandle>,                                               // v1-ONLY
}

pub(crate) fn make_server_state(
    server: Arc<tokio::sync::Mutex<Server>>,
    config: StreamableHttpServerConfig,
) -> ServerState {
    let allowed_origins = config.allowed_origins.clone().unwrap_or_else(AllowedOrigins::localhost);
    let event_store: Option<EventStoreHandle> =
        config.event_store.clone().map(|store| store as EventStoreHandle);
    ServerState {
        server,
        config: Arc::new(config),
        allowed_origins,
        sse_streams: Arc::new(RwLock::new(HashMap::new())),
        sessions: Arc::new(RwLock::new(HashMap::new())),
        event_store,
    }
}
```

Three fields → one `v1: v1::V1State`; ONE construction site to edit.

**Step 3 — the verb split.** Both handlers already open with the v2 rejection
(`:4441-4447`, `:4505-4511`), so the "thin always-present head + `v1::…_body()`" cut is a
two-line edit at the top of each:

```rust
async fn handle_get_sse(State(state): State<ServerState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(rejection) = v2_verb_rejection(&state, &headers, "GET").await {
        return rejection;
    }
    ...
}

async fn handle_delete_session(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Some(rejection) = v2_verb_rejection(&state, &headers, "DELETE").await {
        return rejection;
    }
    ...
}
```

`build_mcp_router` (`:293-302`) is UNCHANGED — GET and DELETE stay routed, they just always 405.

**Whole-file gate pattern for `src/shared/event_store.rs`** — copy the existing conditional
module declaration at `src/shared/mod.rs:121-123`; note the ungated `pub mod event_store;` at
`:33` and the re-export at `:128-131` must BOTH be gated in the same edit:

```rust
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
/// Streamable HTTP transport implementation for MCP.
pub mod streamable_http;

// ... and the re-export that must move under the same cfg:
pub use event_store::{
    EventStore, EventStoreConfig, InMemoryEventStore, MessageDirection, ResumptionManager,
    ResumptionState, ResumptionToken, StoredEvent,
};
```

---

### `crates/mcp-tester/src/era_diff.rs` (module / report, transform + file-I/O)

**Analog:** `crates/mcp-tester/src/post_deploy_report.rs` — **the in-repo precedent for exactly the
A-D11 decision** ("new top-level struct, never a new field on `TestResult`"). Its header already
argues the case:

```rust
//! ## Contract stability
//!
//! `schema_version` is the wire-format guard. Phase 79 ships `"1"`. Future
//! breaking changes MUST bump this and downstream consumers MUST check it
//! before deserializing. Additive field changes (new optional fields with
//! `#[serde(default)]`) do NOT bump the version.
//!
//! ## Why a new struct (vs. extending `TestReport`)
//!
//! `mcp_tester::TestReport` (in `report.rs`) is the existing per-test-suite
//! report. `PostDeployReport` wraps it with metadata the verifier needs:
//! ...
//! Re-using `TestReport` directly would mix concerns; this wrapper keeps the
//! per-test-suite reporter (`TestReport`) and the per-subcommand verifier
//! contract (`PostDeployReport`) cleanly separated.
```

Forward-compatible field shape (`post_deploy_report.rs:60-70`):

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostDeployReport {
    pub command: TestCommand,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    pub outcome: TestOutcome,
    ...
}
```

**The types `DualRunReport` must NOT touch** (measured, hard compile breaks in `cargo-pmcp`):

```rust
// cargo-pmcp/src/commands/test/apps.rs:874-880 — struct literal, so a new field breaks it
    mcp_tester::TestResult {
        name: format!("[{uri}] read_resource"),
        category: mcp_tester::TestCategory::Apps,
        status: TestStatus::Failed,
        duration: Duration::from_secs(0),
        error: Some(format!("Could not read widget body: {reason}")),
        details: Some(...),
    }

// cargo-pmcp/src/commands/test/conformance.rs:276-289 — exhaustive, NO `_` arm,
// so a new TestCategory variant breaks it
fn category_to_kebab(category: &TestCategory) -> &'static str {
    match category {
        TestCategory::Core => "core",
        ...
        TestCategory::Protocol
        | TestCategory::Performance
        | TestCategory::Compatibility
        | TestCategory::Apps => "core",
    }
}
```

**Orchestrator to wrap, not fork** (`crates/mcp-tester/src/conformance/mod.rs:66-134`). `run_dual`
calls `self.run(...)` twice; `should_run` (`:136-138`) and `strict`/`domains` (`:67-70`) are reused
unchanged:

```rust
pub struct ConformanceRunner {
    strict: bool,
    domains: Option<Vec<ConformanceDomain>>,
}

impl ConformanceRunner {
    pub async fn run(&self, tester: &mut ServerTester) -> TestReport {
        let mut report = TestReport::new();
        let start = Instant::now();
        if self.should_run(ConformanceDomain::Core) {
            for result in core_domain::run_core_conformance(tester).await {
                report.add_test(result);
            }
        }
        if !report.has_failures() { /* Transport, Tools, Resources, Prompts, Tasks */ }
        if self.strict { report.apply_strict_mode(); }
        report.duration = start.elapsed();
        report
    }

    fn should_run(&self, domain: ConformanceDomain) -> bool {
        self.domains.as_ref().is_none_or(|d| d.contains(&domain))
    }
}
```

**The opt-in CLI flag** (`crates/mcp-tester/src/main.rs:125-136`, dispatch at `:324-340`) — copy
the `strict` flag's shape exactly; `--dual-run` is a sibling boolean:

```rust
    Conformance {
        /// Server URL
        url: String,

        /// Strict mode (promote warnings to failures)
        #[arg(long)]
        strict: bool,

        /// Run only specific domains (comma-separated: core,tools,resources,prompts,tasks)
        #[arg(long, value_delimiter = ',')]
        domain: Option<Vec<String>>,
    },
```

---

### `crates/mcp-tester/baselines/era-deltas.toml` + `crates/mcp-tester/tests/era_baseline.rs`

**Analog for the TEST:** `tests/phase115_contract_bindings.rs` — a checked-in-data-file schema
tripwire with the exact non-vacuity + FAILURE-MODE idiom this phase needs.

**File constants + a named floor** (`:87-90`, `:149`):

```rust
const BINDING_FILE: &str = "contracts/binding.yaml";

/// The contract file whose `equations:` map the bindings must reference.
const CONTRACT_FILE: &str = "contracts/mcp-protocol-sdk-v1.yaml";

const MINIMUM_BINDINGS: usize = 40;
```

**Non-vacuity guard with a message that names both the failure mode and the remedy**
(`:432-438` and `:701-712`) — note "fix the reader, not the assertion" / "do not lower the floor":

```rust
    assert!(
        names.len() >= 10,
        "FAILURE MODE: only {} equations parsed out of {CONTRACT_FILE} — the reader is broken and \
         the equation-existence check below would pass vacuously.\n\
         WHAT TO DO: fix the reader, not the assertion.",
        names.len()
    );

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
```

**Alternative directory-walk form** if the baseline is ever split into per-delta files:
`crates/pmcp-cfn-renderer/tests/semantic_golden.rs:35-50` — `read_dir` + extension filter +
`checked >= 1` counter.

⚠ **`crates/mcp-tester/Cargo.toml` has NO `toml` dependency** — see § No Analog Found #3.

---

### `crates/mcp-tester/tests/report_compat.rs` (test / golden stdout)

**Analog:** `tests/v1_lists_golden.rs` for the normalization machinery; the writer seam already
exists.

**The writer seam that makes byte capture possible** (`crates/mcp-tester/src/report.rs:238-255`) —
it was added for precisely this reason, so the golden test captures into a `Vec<u8>`:

```rust
    /// Writer-seam helper: render the report into any `std::io::Write` sink.
    ///
    /// Phase 78 Plan 04 (Codex MEDIUM): the existing `print` path wrote
    /// directly to stdout via `println!`, which made it impossible for tests
    /// to assert the printed bytes. This helper accepts any writer so tests
    /// can capture into `Vec<u8>` and assert on the content.
    pub fn print_to_writer<W: Write>(
        &self,
        format: OutputFormat,
        w: &mut W,
    ) -> std::io::Result<()> {
        match format {
            OutputFormat::Pretty => self.print_pretty(w),
            OutputFormat::Json => self.print_json(w),
            ...
        }
    }
```

**⚠ THREE measured non-determinism sources the planner MUST budget for.** A naive
"assert_eq!(captured, GOLDEN)" will be flaky:

1. **`print_pretty` groups by category into a `std::collections::HashMap`** and iterates it
   (`report.rs:262-282`). `HashMap` iteration order is randomized per process — exactly the
   hazard `tests/v1_lists_golden.rs:49-58` documents for `tools/list`. A pretty golden is
   byte-stable ONLY with a single category, or after sorting, or with a normalization pass.
2. **`print_json` serializes `TestReport` whole** (`report.rs:460-463`), and `TestReport` carries
   `duration: Duration` and `timestamp: DateTime<Utc>` (`report.rs:153-158`), plus `TestResult.duration`
   (`:74-81`). All per-run. → use the `DynamicField` width-preserving substitution from
   `v1_lists_golden.rs:97-127` on `duration`/`timestamp`.
3. **`colored` emits ANSI escapes** in `print_pretty`/`print_verbose` (`report.rs:257-259`,
   `:294-298`) and the crate's tty detection differs between a terminal and a captured `Vec<u8>`.
   Pin it explicitly rather than relying on the default.

Also: `print_test_result_pretty` (`report.rs:294-320`) prints a duration column only when
`duration.as_millis() > 100` — a *conditional* format that changes width. Make the fixture's
durations deterministic (`Duration::from_secs(0)`, as `TestReport::from_error` at `:191-203` does)
rather than normalizing after the fact.

---

### `crates/mcp-tester/tests/dual_run.rs` (test / live socket)

**Analog:** `crates/mcp-tester/tests/transport_conformance_integration.rs` — the ONLY existing
live-socket test in this crate, and its header states the deliberate design choice:

```rust
//! We use a hand-rolled `tokio::net::TcpListener` stub instead of an
//! in-process `pmcp` `streamable_http_server` because:
//! 1. The plan permits this fallback ("PROVE the wiring end-to-end against
//!    SOME real HTTP server, not specifically the pmcp one").
//! 2. We need to deliberately produce the regression response shape
//!    (`200 + application/json + non-SSE body`) — easier with canned bytes
//!    than with the pmcp server (which is correct by construction).

async fn spawn_stub_server(handler: Handler) -> (SocketAddr, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind 127.0.0.1:0");
    let addr = listener.local_addr().expect("local_addr");
    let join = tokio::spawn(async move { /* accept loop; one request per conn */ });
    (addr, join)
}
```

⚠ **For 117 the trade-off inverts.** D-05's auto-detect must distinguish DUAL / V1 / V2, and only a
REAL pmcp server produces the genuine `-32601`/`400 Unsupported protocol version` signatures the
detector classifies on (RESEARCH § Q4.3 table). `mcp-tester` already depends on
`pmcp = { path = "../../", features = ["streamable-http", "oauth"] }`
(`crates/mcp-tester/Cargo.toml:21`), so an in-process `StreamableHttpServer` is available.
Use `spawn_stub_server` only for the *negative* cases a real server cannot produce.

---

### `crates/pmcp-agent/tests/agent_v2_e2e.rs` (test / live socket)

**In-crate analog for the socket harness:** `crates/pmcp-agent/tests/http_sources_mock.rs:14-60`:

```rust
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Spawn a one-shot mock HTTP server that captures the first request, optionally
/// delays, then replies with `status` + `resp_body` (JSON).
async fn spawn_mock(status: u16, resp_body: String, delay: Option<Duration>) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    ...
}
```

**In-crate analog for shared test modules:** `crates/pmcp-agent/tests/common/duplex.rs`, pulled in
with `#[path]` by four test binaries (`real_loop_sampling.rs:13`, `adapter_agent_as_server.rs:17`,
`e2e_package_to_adapter.rs:12`, `sampling_source.rs:15`):

```rust
#[path = "common/duplex.rs"]
mod duplex;
```

**Feature gating for a native-only test** — `invoker_task_augmented.rs:14`:

```rust
#![cfg(not(target_arch = "wasm32"))]
```

⚠ The era-probe path needs `url-connector`, so the gate is
`#![cfg(all(feature = "url-connector", not(target_arch = "wasm32")))]`.

**Mock `ConnectorClient` for the non-socket half** — `invoker_task_augmented.rs:30-60` is the
established shape (a `Behavior` enum + `AtomicUsize` counters), and it is what to use for anything
that does not genuinely need a socket.

**The unreachable-host case needs no server at all** — point the factory at a closed loopback port.

---

### `crates/pmcp-agent/src/invoker/factory.rs` (modified — the CLNT-03 change)

**Current code, in full** (`:125-146`). The unconditional `initialize` at `:141` is the entire bug:

```rust
        async fn client_for(
            &self,
            endpoint: &str,
        ) -> Result<Arc<dyn ConnectorClient>, InvokerError> {
            let url = url::Url::parse(endpoint)
                .map_err(|e| InvokerError::Config(format!("invalid endpoint URL: {e}")))?;
            // T-108-05-05: only http(s) endpoints are dispatched (mirrors the
            // 108-04 scheme policy) — never a `file://`/`data:`/etc. scheme.
            match url.scheme() {
                "http" | "https" => {},
                other => return Err(InvokerError::UnsupportedScheme(other.to_string())),
            }
            let config = StreamableHttpTransportConfigBuilder::new(url).build();
            let transport = StreamableHttpTransport::new(config);
            let mut client = Client::new(transport);
            client
                .initialize(ClientCapabilities::default())
                .await
                .map_err(|e| InvokerError::Transport(e.to_string()))?;
            Ok(Arc::new(UrlConnectorClient { client }))
        }
```

**Pattern to copy for the v2 attempt:** `examples/s48_v2_mrtr_client.rs:96-107` is the canonical
era-pinned client construction in this repo:

```rust
fn v2_client(
    url: &Url,
    handler: Option<ScriptedElicitation>,
) -> pmcp::Result<Client<StreamableHttpTransport>> {
    let transport = StreamableHttpTransport::new(
        StreamableHttpTransportConfigBuilder::new(url.clone()).build(),
    );
    let builder = ClientBuilder::new(transport)
        .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))?;
    Ok(match handler {
        Some(handler) => builder.on_elicitation(handler).build(),
        None => builder.build(),
    })
}
```

**⚠ A-D08 — the lock the plan must NOT violate.** `src/client/mod.rs:871-878` and `:892`, verbatim:

```rust
    /// Ask a v2 server for its capability projection (`server/discover`).
    ///
    /// v2 has no `initialize`, so this is how a client learns what the server
    /// supports. It is EXPLICIT: pmcp never calls it implicitly, and never uses
    /// it to CHOOSE an era (Phase-113 D-08 forbids exactly that auto-probe).
    /// Populating capabilities from a call the USER made is a different thing
    /// from probing to decide which protocol to speak — do not "restore" the
    /// latter.
    pub async fn server_discover(
        &mut self,
    ) -> Result<crate::types::protocol::ServerDiscoverResult> {
        self.require_v2(crate::types::protocol::SERVER_DISCOVER_METHOD)?;
```

and `require_v2` (`src/client/mod.rs:707-721`) — it fails LOCALLY, with no round trip:

```rust
    /// Fail fast and LOCALLY when a v2-only method is called on a v1 connection.
    fn require_v2(&self, method: &str) -> Result<()> {
        if self.is_v2() { return Ok(()); }
        Err(Error::InvalidState(format!(
            "{method} requires the 2026-07-28 era — select it with \
             ClientBuilder::with_protocol_version"
        )))
    }
```

**If a new failure discriminator is needed** (RESEARCH § Q4.3 says the reachability rule avoids it,
but if the planner disagrees): use the marker-const pattern at `src/error/mod.rs:114-131`, never a
new `Error` variant:

```rust
/// The stable programmatic identity of [`Error::mrtr_round_limit_exceeded`].
///
/// Carried in the error's `data.pmcpError`. It is the discriminator
/// [`Error::is_mrtr_round_limit_exceeded`] matches on, so it is part of the
/// crate's compatibility surface: **do not change this string**.
pub const MRTR_ROUND_LIMIT_MARKER: &str = "MrtrRoundLimitExceeded";

/// The stable programmatic identity of [`Error::retired_on_v2`].
pub const RETIRED_ON_V2_MARKER: &str = "RetiredOnV2";
```

---

### `crates/pmcp-agent/src/trace.rs` (modified — the D-08 payload)

**The additive-field precedent is INSIDE the struct being changed** (`:32-43`) — `initial_state`
already carries the exact attribute pair a new `negotiated_version` needs:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectTrace {
    /// Optional pre-seeded state the store loads (drives resume). `None` = fresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_state: Option<RunState>,
    /// Ordered completion results, one returned per `create_message` call.
    pub completions: Vec<CreateMessageResultWithTools>,
    /// Ordered tool-batch results, one returned per `invoke_batch` call.
    #[serde(default)]
    pub tool_batches: Vec<Vec<ToolCallResult>>,
}

impl EffectTrace {
    /// Build a trace from completions and tool batches (fresh initial state).
    #[must_use]
    pub fn new(
        completions: Vec<CreateMessageResultWithTools>,
        tool_batches: Vec<Vec<ToolCallResult>>,
    ) -> Self {
        Self { initial_state: None, completions, tool_batches }
    }
}
```

→ add `with_era(..)` / `with_negotiated_version(..)` as a `#[must_use]` consuming builder rather
than widening `new`'s arity (RESEARCH § Q4.4).

**`ReplayInvoker` and its deterministic exhaustion path** (`:158-197`) — the mismatch behaviour must
match this determinism discipline, and `ReplaySource`'s exhaustion error (`:150-155`) is the model
for "fail deterministically, do not panic":

```rust
/// A [`ToolInvoker`] that returns recorded tool batches from an [`EffectTrace`].
///
/// Each `invoke_batch` call returns the next recorded batch; an exhausted trace
/// returns an empty batch (deterministic).
#[derive(Debug)]
pub struct ReplayInvoker {
    batches: Vec<Vec<ToolCallResult>>,
    cursor: AtomicUsize,
}

impl ReplayInvoker {
    /// Build a replay invoker from the tool batches recorded in `trace`.
    #[must_use]
    pub fn from_trace(trace: &EffectTrace) -> Self {
        Self::new(trace.tool_batches.clone())
    }
}
```

**Backward-compat + property proof** — `crates/pmcp-agent/tests/replay_safety.rs:135-152` shows the
fixture-based test to extend (a pre-117 fixture with no era field must still deserialize):

```rust
#[test]
fn golden_end_turn_completes_in_one_step() {
    let trace: EffectTrace =
        serde_json::from_str(include_str!("fixtures/golden_trace_end_turn.json"))
            .expect("valid end-turn fixture");
    let decisions = run_once(&trace, &replay_config());
    assert_eq!(decisions.outcome, Some(OutcomeTag::Completed));
    ...
    // Deterministic across runs.
    assert_eq!(decisions, run_once(&trace, &replay_config()));
}
```

The two existing fixtures — `crates/pmcp-agent/tests/fixtures/golden_trace_end_turn.json` and
`golden_trace_tool_loop.json` — **are already era-less pre-117 traces.** They ARE the
backward-compatibility test; do not regenerate them.

---

### `examples/s49_v2_agent_client.rs` (example / runnable)

**Analog:** `examples/s47_v2_stateless_mrtr.rs` (296 lines, SERVER) + `examples/s48_v2_mrtr_client.rs`
(236 lines, CLIENT). Both read. The house shape:

**Header contract** (`s48:1-41`) — run command, paired-process instruction, numbered
"What this demonstrates" list, and an explicit exit-code contract:

```rust
//! Example: a 2026-07-28 CLIENT that fulfils multi-round-trip elicitation
//! automatically.
//!
//! Start the paired SERVER first:
//! ```bash
//! cargo run --example s47_v2_stateless_mrtr --features full
//! ```
//!
//! Then run this client with:
//! ```bash
//! cargo run --example s48_v2_mrtr_client --features full
//! ```
//!
//! It takes the server address as `argv[1]` and defaults to `127.0.0.1:8147`,
//! which is where `s47` binds when it is given no address of its own. This is a
//! one-shot script: it exits 0 when every demonstration behaved as documented,
//! and non-zero otherwise.
```

**Main body: numbered demos + a banner** (`s48:110-131`):

```rust
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::args().nth(1).unwrap_or_else(|| DEFAULT_ADDR.to_string());
    let url = Url::parse(&format!("http://{addr}/"))?;
    println!("=============================================================");
    println!("  v2 (2026-07-28) MRTR CLIENT  ->  http://{addr}");
    println!("=============================================================");
    demo_automatic_fulfilment(&url).await?;
    demo_unfulfilled_is_returned(&url).await?;
    demo_undeclared_capability(&url).await?;
    println!("  All three demonstrations behaved as documented.");
    Ok(())
}
```

**Declaration** (`Cargo.toml:616-645`) — copy the block AND the numbering note. Note `s49` is
already used twice (`s49_sampling_host` at `:584`, `s49_v2_subscriptions_client` at `:643`); cargo
example NAMES are unique so `s49_v2_agent_client` is legal, but the plan must record the collision
the way `:620-625` already does:

```toml
[[example]]
name = "s48_v2_mrtr_client"
path = "examples/s48_v2_mrtr_client.rs"
required-features = ["streamable-http", "http-client"]
```

**⚠ Placement decision the planner must make explicitly.** Two viable homes, both with precedent:

| Home | Precedent | Cost |
|---|---|---|
| `examples/s49_v2_agent_client.rs` (root) | matches VALIDATION.md's command `cargo run --example s49_v2_agent_client --features "full"`; root `[dev-dependencies]` **already** path-deps three workspace members (`Cargo.toml:198-201`: `pmcp-macros`, `pmcp-code-mode`, `pmcp-code-mode-derive`) | adds `pmcp-agent = { path = "crates/pmcp-agent", features = ["url-connector"] }` as a 4th root dev-dep. Dev-dep cycles are legal in cargo and this repo already relies on that |
| `crates/pmcp-agent/examples/s49_v2_agent_client.rs` | `crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs` (auto-discovered, run as `cargo run -p pmcp-agent --example …`) | needs an explicit `[[example]]` block with `required-features = ["url-connector"]` — `pmcp-agent/Cargo.toml` currently has NO `[[example]]` section, so s50 is auto-discovered and unfeatured |

Note `make test-examples` (`Makefile:254-266`) globs **`examples/*.rs` only** — an example under
`crates/pmcp-agent/examples/` is NOT built by `make test-all`.

---

### Fuzz target (feature-list / baseline parser)

**Analog:** `fuzz/fuzz_targets/dcr_response_parser.rs` (44 lines) — the shortest complete
parser-fuzz target, and it carries the CLAUDE.md invocation line in its header:

```rust
//! Fuzz target for `pmcp::client::oauth::DcrResponse` JSON parser.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run dcr_response_parser`.
//!
//! Invariant: `serde_json::from_slice::<DcrResponse>` must never panic on
//! arbitrary bytes. Error paths are acceptable; panics are not.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(response) = serde_json::from_slice::<pmcp::client::oauth::DcrResponse>(data) else {
        return;
    };
    ...
    assert!(
        text.contains(application_type),
        "DcrResponse::application_type() returned `{application_type}`, which does not appear \
         anywhere in the input bytes"
    );
});
```

**Manifest wiring** (`fuzz/Cargo.toml:223-228`) — one `[[bin]]` per target:

```toml
[[bin]]
name = "fuzz_schema_draft_pin"
path = "fuzz_targets/fuzz_schema_draft_pin.rs"
test = false
doc = false
bench = false
```

**`mcp-tester` is ALREADY a fuzz dependency** (`fuzz/Cargo.toml:53-54`), used by
`fuzz/fuzz_targets/app_widget_scanner.rs` — so a baseline-parser target can call
`mcp_tester::era_diff::…` with zero manifest churn beyond the `[[bin]]` block:

```toml
[dependencies.mcp-tester]
path = "../crates/mcp-tester"
```

⚠ The `pmcp` fuzz dep is `default-features = false` with an explicit feature list
(`fuzz/Cargo.toml:31-43`) whose comments explain each entry. If the target needs `v1-compat`
symbols, add it there WITH a written rationale — that file's convention is that every feature
entry is justified in prose.

---

### `docs/v1-sunset-policy.md` (docs / normative prose)

**Siblings:** `docs/protocol-compatibility.md`, `docs/MIGRATION.md`, `docs/JSON_RPC_COMPATIBILITY.md`
— top-level normative `docs/` documents (as opposed to `docs/design/`, which holds design docs).
`docs/protocol-compatibility.md:1-10` shows the house shape (title, `## Overview`, status tables):

```markdown
# Protocol Compatibility Report

## Overview

This document tracks the compatibility between PMCP (Rust SDK) and the official TypeScript MCP SDK v1.17.2+.

## Compatibility Status
```

**⚠ `docs/*.md` is gated by NOTHING.** No link checker, no mdbook build over `docs/`. Verified:
`make doc-check` is rustdoc-only (`Makefile:426-430`) and `make book` builds `pmcp-book/`, not
`docs/`. The enforcement therefore has to live in the **rustdoc half** — which is why the
`Makefile:429` edit is load-bearing:

```makefile
.PHONY: doc-check
doc-check:
	@echo "$(BLUE)Checking rustdoc warnings (zero-tolerance)...$(NC)"
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps \
		--features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket
	@echo "$(GREEN)✓ Zero rustdoc warnings$(NC)"
```

`make doc-check` IS blocking: `ci.yml:230-231` runs it inside the `quality-gate` job, which IS in
`gate.needs` (`ci.yml:443`).

---

## Shared Patterns

### Pattern S1 — The blocking-CI-job wiring (THREE edits, not one)

**Source:** `.github/workflows/ci.yml:441-462`
**Apply to:** the new `v1-severance` job

```yaml
  gate:
    runs-on: ubuntu-latest
    needs: [test, quality-gate, purity-check, pmcp-agent-targets, wasm32-purity]
    if: always()
    steps:
      - name: Evaluate required checks
        env:
          TEST_RESULT: ${{ needs.test.result }}
          QG_RESULT: ${{ needs.quality-gate.result }}
          PURITY_RESULT: ${{ needs.purity-check.result }}
          AGENT_TARGETS_RESULT: ${{ needs.pmcp-agent-targets.result }}
          WASM32_RESULT: ${{ needs.wasm32-purity.result }}
        run: |
          if [[ "$TEST_RESULT" != "success" ]] || \
             [[ "$QG_RESULT" != "success" ]] || \
             [[ "$PURITY_RESULT" != "success" ]] || \
             [[ "$AGENT_TARGETS_RESULT" != "success" ]] || \
             [[ "$WASM32_RESULT" != "success" ]]; then
            echo "Required checks failed: test=$TEST_RESULT, quality-gate=$QG_RESULT, purity-check=$PURITY_RESULT, pmcp-agent-targets=$AGENT_TARGETS_RESULT, wasm32-purity=$WASM32_RESULT"
            exit 1
          fi
          echo "All required checks passed."
```

**Three separate acceptance items:** `needs:` (`:443`), the `env:` block (`:447-452`), and BOTH the
`if` chain and its echo string (`:454-461`). Adding only `needs:` produces a job that is *awaited*
but whose result is *never checked*.

**Live proof of the trap:** `ci.yml:141-164` is a job named `feature-flags` ("Feature Flag
Verification", `run: make test-feature-flags`) that is **absent from `gate.needs`** — visible,
green-looking, non-blocking. Do not add the severance build there. Also note `make
test-feature-flags` (`Makefile:310-341`) checks **`pmcp-tasks` only** and touches zero root `pmcp`
features, so its name is misleading.

**Job template:** `ci.yml:311-345` (`purity-check`) — the smallest job in the file that IS in
`gate.needs`; copy its checkout / toolchain / per-job cache-key shape:

```yaml
  purity-check:
    name: Purity Gate
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v7

    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable

    - name: Cache cargo
      uses: actions/cache@v6
      with:
        path: |
          ~/.cargo/registry/index/
          ~/.cargo/registry/cache/
          ~/.cargo/git/db/
        key: ${{ runner.os }}-cargo-purity-${{ hashFiles('**/Cargo.lock') }}
        restore-keys: |
          ${{ runner.os }}-cargo-purity-

    - name: Run purity gate (per-crate, per-feature, fail-closed; Layers 1+2)
      run: make purity-check
```

`wasm32-purity` (`ci.yml:404-440`) is the closer analog for *rationale prose*: its ~20-line comment
block explains why the fence exists and what must not be "fixed" — the `v1-severance` job should
carry the same, naming `--all-features` / `--all-targets` / workspace-wide builds as the three
false greens.

### Pattern S2 — A dependency-free marker feature with a written scope note

**Source:** `Cargo.toml:236-243`
**Apply to:** the new `v1-compat` and `full-v2` entries

```toml
# Internal test/fuzz support surface. NOT stable API, and deliberately absent
# from BOTH `default` and `full` so `cargo public-api` never sees the seam on the
# shipped surface. Only `fuzz/` enables it (see fuzz/Cargo.toml).
fuzzing = []

# Testing support
test-helpers = []
# Conformance helpers (pmcp::testing) — folded into `full` so the quality gate
# compiles the module AND runs the tasks-lifecycle integration test. NOT in
# `default`, so lean release builds omit it.
testing = []
```

Current state to modify (`Cargo.toml:203-205`), confirming D-02's premise exactly:

```toml
[features]
default = ["logging"]
full = ["websocket", "http", "streamable-http", "sse", "validation", "resource-watcher", "rayon", "schema-generation", "jwt-auth", "composition", "mcp-apps", "http-client", "logging", "macros", "testing"]
```

### Pattern S3 — Live-socket harness: spawn / teardown / post

**Source:** `tests/common/v2.rs` (~850 lines, `pub` throughout)
**Apply to:** `tests/v1_byte_identity_after_cut.rs` (directly), and as the design reference for the
per-crate harnesses in `crates/pmcp-agent/tests/` and `crates/mcp-tester/tests/`.

```rust
/// Spawn `server` with an arbitrary config on an ephemeral loopback port.
pub async fn spawn_with(
    server: Server,
    config: StreamableHttpServerConfig,
) -> (SocketAddr, JoinHandle<()>) {
    spawn_shared_with(Arc::new(Mutex::new(server)), config).await
}

pub async fn spawn_shared_with(
    server: Arc<Mutex<Server>>,
    config: StreamableHttpServerConfig,
) -> (SocketAddr, JoinHandle<()>) {
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let http = StreamableHttpServer::with_config(addr, server, config);
    http.start().await.expect("server starts")
}

/// Shut a spawned server down in the order: drop sockets -> `abort()` -> `await`.
///
/// The order is the point. D-113-T recorded an intermittent nextest `LEAK` ...
pub async fn teardown<S: Send>(handle: JoinHandle<()>, sockets: S) {
    drop(sockets);
    handle.abort();
    let _ = handle.await;
}

/// Upper bound on any single stream read or poll in the subscription suites.
///
/// A hung stream must FAIL the test, not hang it.
pub const FRAME_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
```

`tests/common/mod.rs` (7 lines) explains why this lives where it does:

```rust
//! Shared test support for the Phase-113 integration test files.
//!
//! Files under `tests/common/` are NOT compiled as their own test binaries, so
//! this is the correct home for helpers that several `tests/*.rs` files share.
//! Consume it with `mod common;` from a test binary.

pub mod v2;
```

### Pattern S4 — Additive-only public API (marker const, never a new enum variant)

**Source:** `src/error/mod.rs:114-131`
**Apply to:** any new failure discriminator in `pmcp` core (`Error` is a plain `thiserror` enum with
NO `#[non_exhaustive]`, 116 D-03). Excerpt in the factory.rs section above.

**Corollary for `mcp-tester`:** `TestResult` (`report.rs:74-81`) and `TestCategory` (`report.rs:55-71`)
are the same class of surface — struct-literal-constructed and exhaustively matched by `cargo-pmcp`.
`post_deploy_report.rs`'s "new top-level struct" is the sanctioned escape hatch.

### Pattern S5 — Non-vacuity guards are mandatory, and named

**Sources:** `tests/v2_bounded_reads_tripwire.rs:185`, `tests/phase115_contract_bindings.rs:432-438,:701-712`,
`crates/pmcp-cfn-renderer/tests/semantic_golden.rs:47-50`.
**Apply to:** every new test in this phase that derives its own scope or parses a file.

Three forms in use, all acceptable:
- `assert!(!files.is_empty(), "... every check in this file would pass vacuously")`
- `assert!(names.len() >= 10, "FAILURE MODE: ...\nWHAT TO DO: fix the reader, not the assertion.")`
- `assert!(checked >= 1, "no goldens found — run scripts/generate-cfn-goldens.sh")`

The `FAILURE MODE: … / WHAT TO DO: …` two-line message form
(`tests/phase115_contract_bindings.rs`) is the newest and the most actionable — prefer it.

---

## No Analog Found

> These are the planning-relevant gaps. Each changes the risk profile of an item.

| Item | Role | Data flow | Finding |
|---|---|---|---|
| `#[cfg_attr(feature = …, path = …)] mod v1;` paired-module selection | module split | n/a | **DOES NOT EXIST anywhere in this repo.** Grep over `src/`, `crates/`, `tests/`, `examples/`, `cargo-pmcp/` finds `#[path]` used only **unconditionally**: one production site (`src/server/wasm_core.rs:213`, `#[path = "wasm_core_tests.rs"]` on a `cfg(test)` module) and five test sites (`crates/pmcp-agent/tests/common/duplex.rs` consumers, `tests/property_tests.rs:11`, `tests/structured_tool_output.rs:35`, `crates/pmcp-cfn-renderer/examples/normalize_json.rs:15`). Every `cfg_attr` in `src/` (21 sites) is `derive(JsonSchema)` or `allow(dead_code)` — **zero** are `path =`. **RESEARCH § Q2.5's central mechanism is a first for this codebase.** Two concrete consequences: (a) `tests/v2_tasks_tripwires.rs:568-600` (`declared_module_file`) already parses `#[path = "…"]` off `mod` declarations and is unit-tested against the `wasm_core_tests` form (`:1998`) — a **`cfg_attr`-wrapped** `#[path]` is a shape it has never seen, so the planner must check whether that tripwire's scanner still resolves the module; (b) there is no in-repo example of rustdoc/clippy behaviour on a `cfg_attr(path)` pair, so `make doc-check` behaviour on `v1_session_off.rs` is unmeasured. |
| ZST stand-in selected by a **cargo feature** | module (null twin) | n/a | **PARTIAL precedent only.** The nearest thing is target-gated, not feature-gated, and is an *inline* module rather than a file swap — `src/server/mod.rs:224-234`: `/// wasm32 stand-in for the native cancellation module. … #[cfg(target_arch = "wasm32")] pub mod cancellation { #[derive(Debug, Clone, Default)] pub struct RequestHandlerExtra; }`. The doc-comment discipline ("so handler signatures stay identical across targets") transfers directly to `V1State`; the *mechanism* does not. |
| A `toml` dependency in `mcp-tester` | config | file-I/O | **`crates/mcp-tester/Cargo.toml:20-40` has NO `toml` dependency.** It has `serde_yaml = "0.9"` (`:26`), which `src/scenario.rs:234-249` already uses for checked-in `scenarios/*.yaml`. RESEARCH § Q5.3 recommends TOML for `era-deltas.toml`; that is a **new dependency on a published 0.7.0 crate**, which needs a stated justification (the CLAUDE.md package-legitimacy discipline). Root `pmcp` has `toml = "1.0"` at `Cargo.toml:76` but that does not reach a sibling crate. Two options with in-repo precedent: add `toml = "1"` (matching root's pin), or use YAML and inherit `scenario.rs`'s existing loader shape. **The planner must decide explicitly — RESEARCH assumed TOML was free and it is not.** |
| A `pmcp-agent` test that drives a real `StreamableHttpServer` | test (live socket) | request-response | **None exists.** `crates/pmcp-agent/tests/` has one socket test (`http_sources_mock.rs`, a raw `TcpListener` HTTP/1.1 mock for the completion sources) and no pmcp-server spawn anywhere. The root harness `tests/common/v2.rs` is in a **different crate's** test tree and cannot be imported. Mitigations that DO exist: `crates/pmcp-agent/Cargo.toml:53` dev-deps `pmcp = { path = "../..", features = ["full"] }`, so `StreamableHttpServer::with_config(...).start()` is available; and `crates/pmcp-agent/tests/common/duplex.rs` proves the per-crate `#[path = "common/…"]` shared-module convention. **Budget a new `crates/pmcp-agent/tests/common/v2_server.rs` in the plan** — it is real work, not a copy. |
| Any CI job that runs `mcp-tester` against a live server | CI | — | **None.** Confirms CONTEXT.md A-CI. `.github/workflows/mcp-tester-validation.yml:59-62` stubs the binary to `echo`, and that workflow is not in `ci.yml`'s `gate.needs`. → CI is not a consumer of the report shape; the only consumers are the six in-repo library linkers. |
| Byte-identical golden over `mcp-tester` **pretty** output | test (golden) | — | **No precedent, and a measured obstacle.** `report.rs:262-282` groups into a `std::collections::HashMap<String, Vec<&TestResult>>` and iterates it, so category order is randomized per process; `colored` (`report.rs:257`) emits ANSI conditionally; `report.rs:305-309` prints a duration column only above 100 ms. The JSON side is tractable via `report.rs:460-463` + the `DynamicField` normalizer, but `duration` and `timestamp` (`report.rs:153-158`) must be substituted. **Plan for a single-category fixture with `Duration::from_secs(0)` rather than a post-hoc normalizer.** |
| A test that parses the `Makefile`'s `doc-check` feature list | test | file-I/O | **None.** `tests/phase115_contract_bindings.rs` parses YAML contracts; nothing in `tests/` reads the `Makefile`. RESEARCH § Q3.4's "second derived tripwire" (assert `doc-check`'s list ⊇ `full` minus a delta) would be the first, and the `Makefile:429` line is a single backslash-continued shell string — a text scan of it is exactly the shape `tests/v2_schema_tripwires.rs:26-42` warns about. **Low-confidence item; consider deferring it and keeping the one-line `Makefile` edit as a plain acceptance item.** |

---

## Metadata

**Analog search scope:** `tests/`, `tests/common/`, `src/server/`, `src/shared/`, `src/client/`,
`src/error/`, `examples/`, `fuzz/`, `crates/pmcp-agent/`, `crates/mcp-tester/`,
`crates/pmcp-cfn-renderer/`, `cargo-pmcp/src/commands/test/`, `.github/workflows/ci.yml`,
`Makefile`, `Cargo.toml`, `docs/`
**Files read this session:** 33
**Pattern extraction date:** 2026-08-07
