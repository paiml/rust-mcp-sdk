//! Exportable, wire-level conformance runner (TEAM-06, D-17/D-19).
//!
//! Replays versioned **fixture format rev 2** cases against a live MCP server
//! reached through a [`ConformanceTarget`] abstraction — either an in-memory
//! [`pmcp::Client`] over a [`crate::DuplexTransport`] OR (behind the `http`
//! feature) a [`pmcp::Client`] over an HTTP endpoint — so the platform can
//! import this runner and point it at its own in-process OR remote servers.
//!
//! # `rev 2` is the fixture FORMAT, not the MCP era
//!
//! `rev 2` is the FIXTURE FORMAT revision. It has nothing to do with MCP era v2
//! (`2026-07-28`); throughout this crate a bare `v2` means the ERA. The on-disk
//! field is still spelled `schema_version: "2"` and is deliberately unchanged —
//! renaming it would be a 33-file data diff for a naming win (Phase 118 D-08).
//!
//! # What "conformant" means here
//!
//! For every server the runner proves **advertised == enforced** at the wire:
//!
//! - `tools/list` cases assert the advertised tool-name set is **EXACTLY** the
//!   expected set (equality, not subset) AND each tool's `inputSchema` equals
//!   the fixture's expected schema (per-tool schema equality).
//! - `tool_call` cases send fixture `_meta` via the low-level `_meta`-forwarding
//!   client API ([`pmcp::Client::call_tool_with_meta`] /
//!   [`pmcp::Client::call_tool_with_task_and_meta`]) so guard state reaches the
//!   server, subset/predicate-match the response, run capture/substitution for
//!   stateful scenarios, and assert related-task **semantically** via
//!   [`pmcp::types::CallToolResult::related_task`] (never a bare string match).
//! - `error` cases assert an error outcome carrying the expected numeric code.
//!
//! # Determinism
//!
//! Independent cases run against **fresh** target instances (built by the caller
//! supplied factory), so per-instance deterministic id/clock seams (mem-001…,
//! appr-001…) replay exactly. Stateful sequences (add→get, write→read,
//! ask→resolve) declare an explicit `scenario` + `order` and run against a
//! single shared instance with capture/substitution.

#![allow(clippy::module_name_repetitions)]

use std::collections::BTreeMap;
use std::path::Path;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Map, Value};

use pmcp::types::protocol::RequestMeta;
use pmcp::types::{CallToolResult, ToolInfo};

// ===========================================================================
// Fixture format rev 2
// ===========================================================================

/// The kind of a fixture case (format rev 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureKind {
    /// Assert the advertised tool surface (exact name set + per-tool schema).
    ToolsList,
    /// Issue a single `tools/call` and assert the outcome/response.
    ToolCall,
}

/// Expected outcome of a `tool_call` case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The call must succeed (`isError == false`).
    Success,
    /// The call must fail with a protocol error carrying a numeric code.
    Error,
}

/// How to compare the fixture's expected `response` against the actual one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    /// Deep structural equality (ignoring `_note*` keys).
    Exact,
    /// Recursive subset: every expected key/element must be present + matching.
    #[default]
    Subset,
    /// Subset semantics plus predicate/wildcard tokens (`"*"`, `"@nonempty"`,
    /// `"@string"`, `"@number"`, `"@bool"`) for generated-field tolerance.
    Predicate,
}

/// Deterministic injection hints. The runner carries these; the harness that
/// BUILDS the target consumes them (e.g. a sequential id seam). Present so
/// fixtures are self-describing and replayable.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Determinism {
    /// RNG seed for a server that uses randomness.
    #[serde(default)]
    pub seed: Option<u64>,
    /// Fixed wall-clock (epoch millis) for a server that stamps time.
    #[serde(default)]
    pub clock: Option<u64>,
    /// Starting id ordinal for a sequential id seam.
    #[serde(default)]
    pub id_seed: Option<u64>,
}

/// The `tools/call` request payload of a fixture.
#[derive(Debug, Clone, Deserialize)]
pub struct RequestSpec {
    /// Tool name to invoke.
    pub name: String,
    /// Tool arguments (may contain `${var}` substitution tokens).
    #[serde(default)]
    pub arguments: Value,
    /// Namespaced request `_meta` (guard state; may contain `${var}` tokens).
    #[serde(rename = "_meta", default)]
    pub meta: Map<String, Value>,
    /// Send with task augmentation (`call_tool_with_task_and_meta`).
    #[serde(default)]
    pub task: bool,
}

/// The expectation block of a fixture.
#[derive(Debug, Clone, Deserialize)]
pub struct Expect {
    /// Required outcome (success/error). Optional for `tools_list` cases.
    #[serde(default)]
    pub outcome: Option<Outcome>,
    /// How to compare `response`.
    #[serde(default, rename = "match")]
    pub match_mode: MatchMode,
    /// Expected response (subset/exact/predicate depending on `match_mode`).
    #[serde(default)]
    pub response: Value,
    /// For `tools_list` cases: the EXACT expected per-tool input-schema map
    /// (tool name → `inputSchema`). Its key set is the exact advertised set.
    #[serde(default, rename = "tools_list_schema")]
    pub tools_list_schema: Option<BTreeMap<String, Value>>,
}

/// A single fixture case (format rev 2).
#[derive(Debug, Clone, Deserialize)]
pub struct Fixture {
    /// Must be `"2"`.
    pub schema_version: String,
    /// Case discriminant.
    pub kind: FixtureKind,
    /// Stable identifier for reporting.
    pub case_id: String,
    /// Which reference server this case targets.
    pub server: String,
    /// Scenario id for a stateful ordered sequence (independent when absent).
    #[serde(default)]
    pub scenario: Option<String>,
    /// Order within a scenario (ascending). Ignored for independent cases.
    #[serde(default)]
    pub order: Option<u32>,
    /// Deterministic injection hints (consumed by the target-building harness).
    #[serde(default)]
    pub determinism: Determinism,
    /// The `tools/call` request (required for `tool_call`).
    #[serde(default)]
    pub request: Option<RequestSpec>,
    /// Variables to extract from the response for later substitution.
    /// Map of `var name → JSON selector` (e.g. `"$.content[0].text"`).
    #[serde(default)]
    pub capture: BTreeMap<String, String>,
    /// Placeholder → capture-var renames for `${placeholder}` tokens. When a
    /// token is absent here it resolves directly against the capture store.
    #[serde(default)]
    pub substitute: BTreeMap<String, String>,
    /// The expectation.
    pub expect: Expect,
}

// ===========================================================================
// ConformanceTarget abstraction (in-memory + HTTP)
// ===========================================================================

/// A protocol-level error surfaced by a `tools/call` (numeric code preserved).
#[derive(Debug, Clone)]
pub struct CallError {
    /// The JSON-RPC numeric error code, if the client surfaced one.
    pub code: Option<i32>,
    /// The error message.
    pub message: String,
}

/// A live server the runner can drive: initialize, list tools, and call tools
/// with arbitrary `_meta`. Implemented for an in-memory client over
/// [`crate::DuplexTransport`] and (behind `http`) an HTTP client, so the same
/// fixtures prove conformance in-process and over the wire (D-19).
#[async_trait]
pub trait ConformanceTarget: Send {
    /// Perform the MCP initialize handshake.
    async fn initialize(&mut self) -> Result<(), String>;

    /// List the advertised tools.
    async fn list_tools(&mut self) -> Result<Vec<ToolInfo>, String>;

    /// Call a tool, forwarding `meta` as request `_meta`. `task` selects the
    /// task-augmented low-level API. Returns the tool result on success or a
    /// [`CallError`] (with the numeric code) on a protocol error.
    async fn call(
        &mut self,
        name: &str,
        args: Value,
        meta: RequestMeta,
        task: bool,
    ) -> Result<CallToolResult, CallError>;
}

/// A [`ConformanceTarget`] backed by a [`pmcp::Client`] over any transport.
///
/// Construct with [`ClientTarget::in_memory`] (spawns a [`pmcp::Server`] over a
/// [`crate::DuplexTransport`] pair) or, behind the `http` feature,
/// [`ClientTarget::http`] (connects to a URL).
pub struct ClientTarget<T: pmcp::shared::Transport> {
    client: pmcp::Client<T>,
    _server_task: Option<tokio::task::JoinHandle<()>>,
}

impl ClientTarget<crate::DuplexTransport> {
    /// Build an in-memory target: spawn `server` on one half of a duplex pair
    /// and connect a client to the other.
    #[must_use]
    pub fn in_memory(server: pmcp::Server) -> Self {
        let (client_t, server_t) = crate::DuplexTransport::pair();
        let task = tokio::spawn(async move {
            let _ = server.run(server_t).await;
        });
        Self {
            client: pmcp::Client::new(client_t),
            _server_task: Some(task),
        }
    }
}

#[cfg(feature = "http")]
impl ClientTarget<pmcp::HttpTransport> {
    /// Connect an HTTP target to a running server `base_url` (D-19). The
    /// platform points the runner at its own endpoint with this constructor.
    ///
    /// # Errors
    /// Returns an error string if the transport cannot be constructed.
    pub fn http(base_url: url::Url) -> Result<Self, String> {
        let transport = pmcp::HttpTransport::with_url(base_url).map_err(|e| e.to_string())?;
        Ok(Self {
            client: pmcp::Client::new(transport),
            _server_task: None,
        })
    }
}

#[async_trait]
impl<T> ConformanceTarget for ClientTarget<T>
where
    T: pmcp::shared::Transport + Send + Sync + 'static,
{
    async fn initialize(&mut self) -> Result<(), String> {
        self.client
            .initialize(pmcp::types::ClientCapabilities::default())
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    async fn list_tools(&mut self) -> Result<Vec<ToolInfo>, String> {
        self.client
            .list_tools(None)
            .await
            .map(|r| r.tools)
            .map_err(|e| e.to_string())
    }

    async fn call(
        &mut self,
        name: &str,
        args: Value,
        meta: RequestMeta,
        task: bool,
    ) -> Result<CallToolResult, CallError> {
        let to_err = |e: pmcp::Error| CallError {
            code: e.error_code().map(|c| c.0),
            message: e.to_string(),
        };
        if task {
            match self
                .client
                .call_tool_with_task_and_meta(name.to_string(), args, meta)
                .await
                .map_err(to_err)?
            {
                pmcp::ToolCallResponse::Result(r) => Ok(r),
                pmcp::ToolCallResponse::Task(t) => Err(CallError {
                    code: None,
                    message: format!("unexpected async task response: {}", t.task_id),
                }),
            }
        } else {
            self.client
                .call_tool_with_meta(name.to_string(), args, meta)
                .await
                .map_err(to_err)
        }
    }
}

// ===========================================================================
// Report
// ===========================================================================

/// The result of a single fixture case.
#[derive(Debug, Clone)]
pub struct CaseResult {
    /// The fixture `case_id`.
    pub case_id: String,
    /// Whether the case conformed.
    pub passed: bool,
    /// Failure detail (present iff `!passed`).
    pub detail: Option<String>,
}

/// A structured conformance report (preferred over fail-fast assertions).
#[derive(Debug, Clone, Default)]
pub struct ConformanceReport {
    /// Count of conformant cases.
    pub passed: usize,
    /// Count of non-conformant cases.
    pub failed: usize,
    /// Per-case results, in execution order.
    pub cases: Vec<CaseResult>,
}

impl ConformanceReport {
    fn record(&mut self, case_id: String, outcome: Result<(), String>) {
        match outcome {
            Ok(()) => {
                self.passed += 1;
                self.cases.push(CaseResult {
                    case_id,
                    passed: true,
                    detail: None,
                });
            },
            Err(detail) => {
                self.failed += 1;
                self.cases.push(CaseResult {
                    case_id,
                    passed: false,
                    detail: Some(detail),
                });
            },
        }
    }
}

/// Panic with a readable diff if any case failed (dependency-light; does not
/// pull `pretty_assertions` into the library build).
///
/// # Panics
/// Panics when `report.failed > 0`, listing every failed case + detail.
pub fn assert_conformant(report: &ConformanceReport) {
    if report.failed == 0 {
        return;
    }
    let mut msg = format!(
        "conformance FAILED: {} passed, {} failed\n",
        report.passed, report.failed
    );
    for case in report.cases.iter().filter(|c| !c.passed) {
        msg.push_str("  ✗ ");
        msg.push_str(&case.case_id);
        msg.push('\n');
        if let Some(detail) = &case.detail {
            for line in detail.lines() {
                msg.push_str("      ");
                msg.push_str(line);
                msg.push('\n');
            }
        }
    }
    panic!("{msg}");
}

// ===========================================================================
// run_fixtures
// ===========================================================================

/// Replay every `*.json` fixture under `fixtures_dir` against fresh targets
/// built by `make_target`.
///
/// Independent cases each get a FRESH target (so per-instance deterministic
/// seams replay exactly); cases sharing a `scenario` run in `order` against a
/// single shared target with capture/substitution. Returns a structured
/// [`ConformanceReport`] — call [`assert_conformant`] to turn failures into a
/// test panic.
///
/// Malformed fixtures (bad `schema_version`/`kind`/JSON) are recorded as FAILED
/// cases, never panics, so the harness catches them.
pub async fn run_fixtures<T, F, Fut>(mut make_target: F, fixtures_dir: &Path) -> ConformanceReport
where
    T: ConformanceTarget,
    F: FnMut() -> Fut,
    Fut: core::future::Future<Output = T>,
{
    let mut report = ConformanceReport::default();

    let (fixtures, load_errors) = load_fixtures(fixtures_dir);
    for (path, err) in load_errors {
        report.record(format!("<load:{}>", path), Err(err));
    }

    // Partition into independent cases and ordered scenario groups.
    let mut independent: Vec<Fixture> = Vec::new();
    let mut scenarios: BTreeMap<String, Vec<Fixture>> = BTreeMap::new();
    for fx in fixtures {
        match &fx.scenario {
            Some(id) => scenarios.entry(id.clone()).or_default().push(fx),
            None => independent.push(fx),
        }
    }
    // Deterministic file/case ordering (Concern: fs iteration order).
    independent.sort_by(|a, b| a.case_id.cmp(&b.case_id));

    for fx in independent {
        let mut target = make_target().await;
        let mut store: BTreeMap<String, Value> = BTreeMap::new();
        let outcome = run_one(&mut target, &fx, &mut store).await;
        report.record(fx.case_id.clone(), outcome);
    }

    for (_id, mut group) in scenarios {
        group.sort_by(|a, b| a.order.cmp(&b.order).then(a.case_id.cmp(&b.case_id)));
        let mut target = make_target().await;
        let mut store: BTreeMap<String, Value> = BTreeMap::new();
        let mut shared_init = target.initialize().await;
        for fx in group {
            let outcome = if let Err(e) = &shared_init {
                Err(format!("scenario initialize failed: {e}"))
            } else {
                run_case_body(&mut target, &fx, &mut store).await
            };
            // Only surface the init error on the first case.
            shared_init = Ok(());
            report.record(fx.case_id.clone(), outcome);
        }
    }

    report
}

/// Run an independent case against a freshly built (un-initialized) target.
async fn run_one<T: ConformanceTarget>(
    target: &mut T,
    fx: &Fixture,
    store: &mut BTreeMap<String, Value>,
) -> Result<(), String> {
    target
        .initialize()
        .await
        .map_err(|e| format!("initialize failed: {e}"))?;
    run_case_body(target, fx, store).await
}

/// Validate schema + dispatch a single case (assumes target is initialized).
async fn run_case_body<T: ConformanceTarget>(
    target: &mut T,
    fx: &Fixture,
    store: &mut BTreeMap<String, Value>,
) -> Result<(), String> {
    if fx.schema_version != "2" {
        return Err(format!(
            "unsupported schema_version {:?} (runner requires \"2\")",
            fx.schema_version
        ));
    }
    match fx.kind {
        FixtureKind::ToolsList => run_tools_list(target, fx).await,
        FixtureKind::ToolCall => run_tool_call(target, fx, store).await,
    }
}

/// Assert EXACT advertised surface + per-tool input-schema equality.
async fn run_tools_list<T: ConformanceTarget>(target: &mut T, fx: &Fixture) -> Result<(), String> {
    let expected = fx
        .expect
        .tools_list_schema
        .as_ref()
        .ok_or_else(|| "tools_list case missing expect.tools_list_schema".to_string())?;

    let tools = target
        .list_tools()
        .await
        .map_err(|e| format!("list_tools failed: {e}"))?;

    // Exact name-set equality (not subset).
    let mut actual_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
    actual_names.sort();
    let mut expected_names: Vec<String> = expected.keys().cloned().collect();
    expected_names.sort();
    if actual_names != expected_names {
        let extra: Vec<&String> = actual_names
            .iter()
            .filter(|n| !expected.contains_key(*n))
            .collect();
        let missing: Vec<&String> = expected_names
            .iter()
            .filter(|n| !tools.iter().any(|t| &t.name == *n))
            .collect();
        return Err(format!(
            "advertised tool set mismatch\n  extra (advertised, not expected): {extra:?}\n  missing (expected, not advertised): {missing:?}"
        ));
    }

    // Per-tool input-schema equality.
    for tool in &tools {
        let want = &expected[&tool.name];
        if &tool.input_schema != want {
            return Err(format!(
                "input schema drift for tool `{}`\n  expected: {}\n  actual:   {}",
                tool.name, want, tool.input_schema
            ));
        }
    }
    Ok(())
}

/// Issue a `tools/call`, match the response, run capture, assert related-task.
async fn run_tool_call<T: ConformanceTarget>(
    target: &mut T,
    fx: &Fixture,
    store: &mut BTreeMap<String, Value>,
) -> Result<(), String> {
    let req = fx
        .request
        .as_ref()
        .ok_or_else(|| "tool_call case missing request".to_string())?;

    // Apply substitution from prior captures into args + _meta.
    let args = substitute(&req.arguments, &fx.substitute, store);
    let meta = build_meta(&req.meta, &fx.substitute, store);

    let outcome = fx.expect.outcome.unwrap_or(Outcome::Success);
    let result = target.call(&req.name, args, meta, req.task).await;

    match (outcome, result) {
        (Outcome::Error, Err(err)) => match_error(&fx.expect, &err),
        (Outcome::Error, Ok(res)) => Err(format!(
            "expected an error outcome but the call SUCCEEDED (guard not enforced): {:?}",
            res.content
        )),
        (Outcome::Success, Err(err)) => Err(format!(
            "expected success but the call errored: code={:?} message={}",
            err.code, err.message
        )),
        (Outcome::Success, Ok(res)) => {
            check_success(&fx.expect, &res)?;
            run_capture(&fx.capture, &res, store)
        },
    }
}

/// Match an error outcome: numeric code equality + optional message match.
fn match_error(expect: &Expect, err: &CallError) -> Result<(), String> {
    let Some(error_obj) = expect.response.get("error") else {
        // No specific error shape required — any error conforms.
        return Ok(());
    };
    if let Some(expected_code) = error_obj.get("code").and_then(Value::as_i64) {
        match err.code {
            Some(actual) if i64::from(actual) == expected_code => {},
            other => {
                return Err(format!(
                    "error code mismatch: expected {expected_code}, got {other:?} (message: {})",
                    err.message
                ))
            },
        }
    }
    if let Some(expected_msg) = error_obj.get("message").and_then(Value::as_str) {
        // Error messages are asserted by SUBSTRING containment: the guard's
        // semantic identity ("self-call rejected", "cycle", …) must appear in
        // the wire message. The high-level Server flattens the numeric code to
        // -32603, so the message is where the guard identity lives.
        if !err.message.contains(expected_msg) {
            return Err(format!(
                "error message mismatch: expected to contain {expected_msg:?}, got {:?}",
                err.message
            ));
        }
    }
    Ok(())
}

/// Match a success response + the semantic related-task assertion.
fn check_success(expect: &Expect, res: &CallToolResult) -> Result<(), String> {
    let actual = serde_json::to_value(res)
        .map_err(|e| format!("could not serialize CallToolResult: {e}"))?;
    match_value(&expect.response, &actual, expect.match_mode)?;

    // Semantic related-task assertion: when the fixture's expected _meta carries
    // a related task, assert it via CallToolResult::related_task() (under
    // RELATED_TASK_META_KEY), NOT a raw string compare.
    if expect
        .response
        .get("_meta")
        .and_then(|m| m.get(pmcp::types::tasks::RELATED_TASK_META_KEY))
        .is_some()
    {
        let related = res.related_task().ok_or_else(|| {
            "expected a related task under RELATED_TASK_META_KEY, found none".to_string()
        })?;
        if related.task_id.is_empty() {
            return Err("related task present but taskId is empty".to_string());
        }
    }
    Ok(())
}

/// Extract capture variables from the response into the shared store.
fn run_capture(
    capture: &BTreeMap<String, String>,
    res: &CallToolResult,
    store: &mut BTreeMap<String, Value>,
) -> Result<(), String> {
    if capture.is_empty() {
        return Ok(());
    }
    let root = serde_json::to_value(res)
        .map_err(|e| format!("could not serialize CallToolResult for capture: {e}"))?;
    for (var, selector) in capture {
        let value = select(&root, selector)
            .ok_or_else(|| format!("capture `{var}`: selector `{selector}` matched nothing"))?;
        store.insert(var.clone(), value);
    }
    Ok(())
}

// ===========================================================================
// Value matching (subset / exact / predicate + wildcards)
// ===========================================================================

/// Recursively match `expected` against `actual` per `mode`. Keys beginning
/// with `_note` are advisory and ignored.
fn match_value(expected: &Value, actual: &Value, mode: MatchMode) -> Result<(), String> {
    // Predicate/wildcard tokens (only under Predicate mode).
    if mode == MatchMode::Predicate {
        if let Value::String(tok) = expected {
            if let Some(res) = eval_predicate(tok, actual) {
                return res;
            }
        }
    }
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => match_object(e, a, actual, mode),
        (Value::Array(e), Value::Array(a)) => match_array(e, a, mode),
        _ => match_scalar(expected, actual),
    }
}

/// Object arm of [`match_value`]: every expected key (except advisory `_note*`)
/// must be present and match; under [`MatchMode::Exact`] no extra keys allowed.
fn match_object(
    e: &Map<String, Value>,
    a: &Map<String, Value>,
    actual: &Value,
    mode: MatchMode,
) -> Result<(), String> {
    for (k, ev) in e {
        if k.starts_with("_note") {
            continue;
        }
        let av = a
            .get(k)
            .ok_or_else(|| format!("missing key `{k}` in {actual}"))?;
        match_value(ev, av, mode)?;
    }
    if mode == MatchMode::Exact {
        for k in a.keys() {
            if !e.contains_key(k) {
                return Err(format!("unexpected extra key `{k}` (exact match)"));
            }
        }
    }
    Ok(())
}

/// Array arm of [`match_value`]: same length, element-wise match (index-tagged).
fn match_array(e: &[Value], a: &[Value], mode: MatchMode) -> Result<(), String> {
    if e.len() != a.len() {
        return Err(format!(
            "array length mismatch: expected {}, got {}",
            e.len(),
            a.len()
        ));
    }
    for (i, (ev, av)) in e.iter().zip(a.iter()).enumerate() {
        match_value(ev, av, mode).map_err(|err| format!("[{i}]: {err}"))?;
    }
    Ok(())
}

/// Scalar arm of [`match_value`]: plain equality.
fn match_scalar(expected: &Value, actual: &Value) -> Result<(), String> {
    if expected == actual {
        Ok(())
    } else {
        Err(format!("value mismatch: expected {expected}, got {actual}"))
    }
}

/// Evaluate a predicate token against `actual`. Returns `None` when `tok` is not
/// a predicate (a literal string), else `Some(Ok/Err)`.
fn eval_predicate(tok: &str, actual: &Value) -> Option<Result<(), String>> {
    let ok = |b: bool, what: &str| {
        Some(if b {
            Ok(())
        } else {
            Err(format!("predicate `{what}` failed for {actual}"))
        })
    };
    if let Some(needle) = tok.strip_prefix("@contains:") {
        // Substring containment against the actual string (or its rendering).
        let hay = actual
            .as_str()
            .map_or_else(|| actual.to_string(), std::string::ToString::to_string);
        return Some(if hay.contains(needle) {
            Ok(())
        } else {
            Err(format!(
                "predicate `@contains:{needle}` failed for {actual}"
            ))
        });
    }
    match tok {
        "*" => Some(if actual.is_null() {
            Err("wildcard `*` matched null".to_string())
        } else {
            Ok(())
        }),
        "@nonempty" => ok(
            actual.as_str().is_some_and(|s| !s.is_empty())
                || actual.as_array().is_some_and(|a| !a.is_empty()),
            "@nonempty",
        ),
        "@string" => ok(actual.is_string(), "@string"),
        "@number" => ok(actual.is_number(), "@number"),
        "@bool" => ok(actual.is_boolean(), "@bool"),
        _ => None,
    }
}

// ===========================================================================
// Substitution + minimal JSON selector
// ===========================================================================

/// Resolve a `${token}` reference against the capture store, honoring an
/// explicit `substitute` rename map.
fn resolve_token(
    token: &str,
    substitute: &BTreeMap<String, String>,
    store: &BTreeMap<String, Value>,
) -> Option<Value> {
    let var = substitute.get(token).map_or(token, String::as_str);
    store.get(var).cloned()
}

/// Recursively substitute `${var}` tokens in `value`. A string equal to
/// `"${var}"` becomes the captured Value verbatim; an embedded token becomes
/// its string rendering.
fn substitute(
    value: &Value,
    subst: &BTreeMap<String, String>,
    store: &BTreeMap<String, Value>,
) -> Value {
    match value {
        Value::String(s) => substitute_string(s, subst, store),
        Value::Array(a) => Value::Array(a.iter().map(|v| substitute(v, subst, store)).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, v)| (k.clone(), substitute(v, subst, store)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn substitute_string(
    s: &str,
    subst: &BTreeMap<String, String>,
    store: &BTreeMap<String, Value>,
) -> Value {
    // Exact `${var}` → captured value verbatim (preserves type).
    if let Some(token) = s.strip_prefix("${").and_then(|r| r.strip_suffix('}')) {
        if !token.contains('{') && !token.contains('}') {
            if let Some(v) = resolve_token(token, subst, store) {
                return v;
            }
        }
    }
    // Embedded tokens → string rendering.
    let mut out = s.to_string();
    while let Some(start) = out.find("${") {
        let Some(end_rel) = out[start..].find('}') else {
            break;
        };
        let end = start + end_rel;
        let token = &out[start + 2..end];
        let replacement = resolve_token(token, subst, store)
            .map(|v| match v {
                Value::String(s) => s,
                other => other.to_string(),
            })
            .unwrap_or_default();
        out.replace_range(start..=end, &replacement);
    }
    Value::String(out)
}

/// Build the outgoing `RequestMeta` from the fixture `_meta` after substitution.
fn build_meta(
    meta: &Map<String, Value>,
    subst: &BTreeMap<String, String>,
    store: &BTreeMap<String, Value>,
) -> RequestMeta {
    let mut rm = RequestMeta::new();
    for (k, v) in meta {
        rm = rm.with_meta(k.clone(), substitute(v, subst, store));
    }
    rm
}

/// Minimal JSON selector: `$.a.b`, `$.content[0].text`, `$.items[2]`.
///
/// Returns an owned value (cloned). When a step navigates INTO a string that is
/// itself valid JSON (as the reference servers wrap plain values into
/// `content[0].text`), the string is auto-parsed so `$.content[0].text.id`
/// reaches the embedded field. Returns `None` when any step misses.
fn select(root: &Value, selector: &str) -> Option<Value> {
    let path = selector.strip_prefix('$').unwrap_or(selector);
    let path = path.strip_prefix('.').unwrap_or(path);
    let mut cur = root.clone();
    for raw in path.split('.') {
        if raw.is_empty() {
            continue;
        }
        cur = descend(&cur, raw)?;
    }
    Some(cur)
}

/// Descend one `name[idx][idx]...` path segment: an optional object key followed
/// by any number of `[idx]` array-index groups. Returns `None` on any miss.
fn descend(value: &Value, raw: &str) -> Option<Value> {
    // Split `name[idx]` into a key then any number of `[idx]` groups.
    let (name, mut rest) = match raw.find('[') {
        Some(b) => (&raw[..b], &raw[b..]),
        None => (raw, ""),
    };
    let mut cur = if name.is_empty() {
        value.clone()
    } else {
        step_into(value, name)?
    };
    while rest.starts_with('[') {
        let close = rest.find(']')?;
        let idx: usize = rest[1..close].parse().ok()?;
        cur = index_into(&cur, idx)?;
        rest = &rest[close + 1..];
    }
    Some(cur)
}

/// Navigate into `value` by object key, auto-parsing a JSON-carrying string.
fn step_into(value: &Value, key: &str) -> Option<Value> {
    if let Some(v) = value.get(key) {
        return Some(v.clone());
    }
    if let Value::String(s) = value {
        if let Ok(parsed) = serde_json::from_str::<Value>(s) {
            return parsed.get(key).cloned();
        }
    }
    None
}

/// Navigate into `value` by array index, auto-parsing a JSON-carrying string.
fn index_into(value: &Value, idx: usize) -> Option<Value> {
    if let Some(v) = value.get(idx) {
        return Some(v.clone());
    }
    if let Value::String(s) = value {
        if let Ok(parsed) = serde_json::from_str::<Value>(s) {
            return parsed.get(idx).cloned();
        }
    }
    None
}

// ===========================================================================
// Fixture loading
// ===========================================================================

/// Load + parse all `*.json` fixtures under `dir`. Returns parsed fixtures and
/// per-file load errors (recorded as failed cases by [`run_fixtures`]).
fn load_fixtures(dir: &Path) -> (Vec<Fixture>, Vec<(String, String)>) {
    let mut fixtures = Vec::new();
    let mut errors = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            errors.push((dir.display().to_string(), format!("read_dir failed: {e}")));
            return (fixtures, errors);
        },
    };
    let mut paths: Vec<std::path::PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();

    for path in paths {
        let display = path.display().to_string();
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Fixture>(&text) {
                Ok(fx) => fixtures.push(fx),
                Err(e) => errors.push((display, format!("malformed fixture: {e}"))),
            },
            Err(e) => errors.push((display, format!("read failed: {e}"))),
        }
    }
    (fixtures, errors)
}

// ===========================================================================
// Negative harness tests: prove the runner FAILS on drift.
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use pmcp::{RequestHandlerExtra, Server, ToolHandler, ToolInfo as PToolInfo};
    use serde_json::json;
    use std::sync::Arc;

    /// A trivial tool: echoes, or errors, depending on `fail`.
    struct EchoTool {
        name: String,
        schema: Value,
        fail: bool,
    }

    #[async_trait]
    impl ToolHandler for EchoTool {
        async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
            if self.fail {
                return Err(pmcp::Error::validation("intentional guard rejection"));
            }
            // Return a PLAIN value (not a CallToolResult shape) so the server's
            // double-wrap tripwire does not fire; the server wraps this into a
            // CallToolResult text envelope on the way out.
            Ok(json!({ "echoed": true }))
        }

        fn metadata(&self) -> Option<PToolInfo> {
            Some(PToolInfo::new(
                self.name.clone(),
                Some("echo".to_string()),
                self.schema.clone(),
            ))
        }
    }

    fn server_with(tools: Vec<EchoTool>) -> Server {
        let mut b = Server::builder().name("neg").version("0.0.0");
        for t in tools {
            let name = t.name.clone();
            b = b.tool_arc(&name, Arc::new(t) as Arc<dyn ToolHandler>);
        }
        b.build().unwrap()
    }

    fn one_tool_schema() -> Value {
        json!({ "type": "object", "properties": { "x": { "type": "string" } } })
    }

    fn write_fixture(dir: &std::path::Path, name: &str, fx: Value) {
        std::fs::write(dir.join(name), serde_json::to_vec_pretty(&fx).unwrap()).unwrap();
    }

    /// Build a fresh in-memory target factory over a server produced by `make`.
    async fn run<F>(dir: &std::path::Path, make: F) -> ConformanceReport
    where
        F: Fn() -> Server,
    {
        run_fixtures(|| async { ClientTarget::in_memory(make()) }, dir).await
    }

    #[tokio::test]
    async fn fails_on_extra_advertised_tool() {
        let dir = tempfile::tempdir().unwrap();
        // Fixture expects ONLY `echo`; server advertises `echo` + `extra`.
        write_fixture(
            dir.path(),
            "list.json",
            json!({
                "schema_version": "2", "kind": "tools_list",
                "case_id": "neg.extra-tool", "server": "neg",
                "expect": { "tools_list_schema": { "echo": one_tool_schema() } }
            }),
        );
        let report = run(dir.path(), || {
            server_with(vec![
                EchoTool {
                    name: "echo".into(),
                    schema: one_tool_schema(),
                    fail: false,
                },
                EchoTool {
                    name: "extra".into(),
                    schema: one_tool_schema(),
                    fail: false,
                },
            ])
        })
        .await;
        assert!(report.failed >= 1, "extra tool must fail: {report:?}");
    }

    #[tokio::test]
    async fn fails_on_schema_drift() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture(
            dir.path(),
            "list.json",
            json!({
                "schema_version": "2", "kind": "tools_list",
                "case_id": "neg.schema-drift", "server": "neg",
                "expect": { "tools_list_schema": {
                    "echo": { "type": "object", "properties": { "DIFFERENT": { "type": "number" } } }
                } }
            }),
        );
        let report = run(dir.path(), || {
            server_with(vec![EchoTool {
                name: "echo".into(),
                schema: one_tool_schema(),
                fail: false,
            }])
        })
        .await;
        assert!(report.failed >= 1, "schema drift must fail: {report:?}");
    }

    #[tokio::test]
    async fn fails_on_missing_guard() {
        let dir = tempfile::tempdir().unwrap();
        // Fixture expects an ERROR, but the server's tool succeeds (guard absent).
        write_fixture(
            dir.path(),
            "call.json",
            json!({
                "schema_version": "2", "kind": "tool_call",
                "case_id": "neg.missing-guard", "server": "neg",
                "request": { "name": "echo", "arguments": {} },
                "expect": { "outcome": "error", "match": "subset",
                    "response": { "error": { "code": -32603 } } }
            }),
        );
        let report = run(dir.path(), || {
            server_with(vec![EchoTool {
                name: "echo".into(),
                schema: one_tool_schema(),
                fail: false,
            }])
        })
        .await;
        assert!(report.failed >= 1, "missing guard must fail: {report:?}");
    }

    #[tokio::test]
    async fn passes_when_guard_enforced() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture(
            dir.path(),
            "call.json",
            json!({
                "schema_version": "2", "kind": "tool_call",
                "case_id": "neg.guard-ok", "server": "neg",
                "request": { "name": "echo", "arguments": {} },
                "expect": { "outcome": "error", "match": "subset",
                    "response": { "error": { "code": -32603 } } }
            }),
        );
        let report = run(dir.path(), || {
            server_with(vec![EchoTool {
                name: "echo".into(),
                schema: one_tool_schema(),
                fail: true, // enforces => errors as expected
            }])
        })
        .await;
        assert_eq!(
            report.failed, 0,
            "enforced guard should conform: {report:?}"
        );
    }

    #[tokio::test]
    async fn fails_on_malformed_fixture() {
        let dir = tempfile::tempdir().unwrap();
        // Wrong schema_version + garbage file both must be caught.
        write_fixture(
            dir.path(),
            "old.json",
            json!({
                "schema_version": "1", "kind": "tools_list",
                "case_id": "neg.old-schema", "server": "neg",
                "expect": { "tools_list_schema": { "echo": one_tool_schema() } }
            }),
        );
        std::fs::write(dir.path().join("garbage.json"), b"{ not json ").unwrap();
        let report = run(dir.path(), || {
            server_with(vec![EchoTool {
                name: "echo".into(),
                schema: one_tool_schema(),
                fail: false,
            }])
        })
        .await;
        assert!(
            report.failed >= 2,
            "both malformed fixtures must fail: {report:?}"
        );
    }

    #[tokio::test]
    async fn passes_on_exact_conformant_surface() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture(
            dir.path(),
            "list.json",
            json!({
                "schema_version": "2", "kind": "tools_list",
                "case_id": "neg.exact-ok", "server": "neg",
                "expect": { "tools_list_schema": { "echo": one_tool_schema() } }
            }),
        );
        let report = run(dir.path(), || {
            server_with(vec![EchoTool {
                name: "echo".into(),
                schema: one_tool_schema(),
                fail: false,
            }])
        })
        .await;
        assert_eq!(
            report.failed, 0,
            "conformant surface should pass: {report:?}"
        );
        assert_conformant(&report);
    }

    #[test]
    fn select_navigates_objects_and_arrays() {
        let v = json!({ "content": [{ "id": "mem-001" }, { "id": "mem-002" }] });
        assert_eq!(select(&v, "$.content[0].id").unwrap(), json!("mem-001"));
        assert_eq!(select(&v, "$.content[1].id").unwrap(), json!("mem-002"));
        assert!(select(&v, "$.content[9].id").is_none());
    }

    #[test]
    fn select_auto_parses_json_in_text() {
        // The reference servers wrap plain values into content[0].text as a
        // serialized JSON string; the selector transparently descends into it.
        let v = json!({ "content": [{ "type": "text", "text": "{\"id\":\"appr-001\"}" }] });
        assert_eq!(
            select(&v, "$.content[0].text.id").unwrap(),
            json!("appr-001")
        );
    }

    #[test]
    fn substitution_preserves_type_and_embeds() {
        let mut store = BTreeMap::new();
        store.insert("approval_id".to_string(), json!("appr-001"));
        let subst = BTreeMap::new();
        let args = json!({ "approvalId": "${approval_id}", "note": "id=${approval_id}" });
        let out = substitute(&args, &subst, &store);
        assert_eq!(out["approvalId"], json!("appr-001"));
        assert_eq!(out["note"], json!("id=appr-001"));
    }
}
