//! OAPI-08 / D-04 — the london-tube **reference-parity** assertion.
//!
//! Reproduces the london-tube (TfL) reference instance and proves the Shape-A
//! binary serves the SAME tools + behavior as the pmcp-run reference, replayed
//! OFFLINE via `wiremock` (pure-Rust, no Docker, no live network in default CI).
//! This is the single most valuable parity target — it exercises the new
//! `api_key` query-parameter outgoing-auth path AND the `${TFL_APP_KEY}` secret
//! expansion path end-to-end.
//!
//! Tests here:
//!
//! 1. [`london_tube_fixture`] — fixture-validity: the vendored `london-tube.toml`
//!    parses via [`ServerConfig::from_toml_strict_validated`] and
//!    `london-tube-api.yaml` via [`OpenApiSchema::parse`], with the expected tool
//!    names present.
//! 2. `london_tube_parity_through_real_binary_path` — drives the REAL binary
//!    pipeline ([`run_serving`]) against a `wiremock` backend that REQUIRES the
//!    `app_key=dummy` query param (proving the api_key query-param auth AND that
//!    `${TFL_APP_KEY}` resolved to `dummy`, never the literal `${...}`), then
//!    replays `london-tube-scenarios.yaml` through `mcp-tester`, gating per-step.
//!    The fixture is copied VERBATIM and pointed at wiremock through the
//!    `${TFL_BASE_URL}` endpoint slot (PKG-03) — the same mechanism a real
//!    deployment uses — rather than by string-replacing a baked `base_url`.
//! 3. `parity_live_tfl` (`#[ignore]`) — the same scenarios against the REAL
//!    `https://api.tfl.gov.uk`, double-gated on `PMCP_OPENAPI_LIVE_TEST=1` +
//!    a real `TFL_APP_KEY`.
//!
//! Run the offline suite (single-threaded — ephemeral port + per-process env):
//! ```sh
//! cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1
//! ```

use std::time::Duration;

use mcp_tester::{ScenarioExecutor, ServerTester, TestScenario};
use pmcp_openapi_server::{run_serving, Args};
use pmcp_server_toolkit::http::OpenApiSchema;
use pmcp_server_toolkit::ServerConfig;
use wiremock::MockServer;

mod common;

use common::{
    assert_london_tube_code_mode_surface, assert_london_tube_config_slots, examples_dir,
    fixtures_dir, mount_london_tube, tfl_env_lock, DUMMY_APP_KEY,
};

/// P901-EXAMPLE smoke: the user-pointable `examples/london-tube.toml` (the config
/// a user runs via `pmcp-openapi-server --config <path>`) parses + validates
/// through the SAME strict entry point the binary uses, and carries the full
/// resources/prompts/annotations showcase surface. This proves the published
/// example config is well-formed without needing to boot a live server.
#[test]
fn pointable_example_config_parses_and_validates() {
    let config_text = std::fs::read_to_string(examples_dir().join("london-tube.toml"))
        .expect("read examples/london-tube.toml");
    let cfg = ServerConfig::from_toml_strict_validated(&config_text)
        .expect("pointable examples/london-tube.toml parses + validates");

    // Same three Code Mode context resources + start_code_mode prompt as the
    // enriched fixture (shared surface — asserted in one helper to prevent drift).
    assert_london_tube_code_mode_surface(&cfg, "pointable example");
    // No real credential committed — only the ${TFL_APP_KEY} placeholder.
    assert!(
        config_text.contains("${TFL_APP_KEY}"),
        "pointable example keeps the ${{TFL_APP_KEY}} placeholder (no real key)"
    );
    // PKG-03: the same three declared slots + the ${TFL_BASE_URL} endpoint
    // placeholder as the fixture — the two copies move together.
    assert_london_tube_config_slots(&cfg, &config_text, "pointable example");
}

/// Fixture-validity (Task 1): the vendored config + spec parse, and the expected
/// reference tool names are present. This is the offline gate that the vendored
/// fixtures are well-formed BEFORE the parity replay drives them.
#[test]
fn london_tube_fixture() {
    let dir = fixtures_dir();

    // (1) The config parses through the production strict+validated entry point.
    let config_text =
        std::fs::read_to_string(dir.join("london-tube.toml")).expect("read london-tube.toml");
    let cfg = ServerConfig::from_toml_strict_validated(&config_text)
        .expect("vendored london-tube.toml parses + validates");

    // The expected reference tool names are present (one single-call + one script).
    let tool_names: Vec<&str> = cfg.tools.iter().map(|t| t.name.as_str()).collect();
    assert!(
        tool_names.contains(&"get-tube-status"),
        "single-call tool get-tube-status present: {tool_names:?}"
    );
    assert!(
        tool_names.contains(&"disrupted-lines-with-detail"),
        "script tool disrupted-lines-with-detail present: {tool_names:?}"
    );

    // (1a/1b) The enriched showcase ships all three Code Mode context resources
    // and the start_code_mode prompt (shared surface — see helper).
    assert_london_tube_code_mode_surface(&cfg, "enriched fixture");

    // (1a') PKG-03: the three declared config slots + the ${TFL_BASE_URL}
    // endpoint placeholder. A future edit that drops the block fails HERE.
    assert_london_tube_config_slots(&cfg, &config_text, "enriched fixture");

    // (1c) cost_hint VALUE check: get-tube-status parses to the allowed "low" value
    // (not a mere presence check — proves the cost_hint enum string parsed).
    let status_tool = cfg
        .tools
        .iter()
        .find(|t| t.name == "get-tube-status")
        .expect("get-tube-status tool present");
    let cost_hint = status_tool
        .annotations
        .as_ref()
        .and_then(|a| a.cost_hint.as_deref());
    assert_eq!(
        cost_hint,
        Some("low"),
        "get-tube-status cost_hint must parse to the allowed \"low\" value (one of low|medium|high)"
    );

    // The api_key query-param backend auth (the D-04 path) is declared.
    let backend = cfg.backend.as_ref().expect("[backend] section present");
    assert!(
        matches!(
            &backend.auth,
            pmcp_server_toolkit::http::auth::AuthConfig::ApiKey { query_params, .. }
                if query_params.get("app_key").is_some_and(|v| v.contains("${TFL_APP_KEY}"))
        ),
        "[backend.auth] is api_key with an app_key=${{TFL_APP_KEY}} query param"
    );

    // The disrupted-lines tool is a SCRIPT tool (D-01 detection rule).
    let script_tool = cfg
        .tools
        .iter()
        .find(|t| t.name == "disrupted-lines-with-detail")
        .expect("script tool present");
    assert!(
        script_tool.is_script_tool(),
        "disrupted-lines-with-detail is a script tool"
    );

    // (2) The minimal OpenAPI spec parses and surfaces the operations the tools call.
    let spec_text = std::fs::read_to_string(dir.join("london-tube-api.yaml"))
        .expect("read london-tube-api.yaml");
    let schema = OpenApiSchema::parse(&spec_text).expect("vendored london-tube-api.yaml parses");
    assert!(
        schema
            .operation_for("/Line/Mode/tube/Status", "GET")
            .is_some(),
        "spec covers GET /Line/Mode/tube/Status (the get-tube-status tool)"
    );
    assert!(
        schema
            .operation_for("/Line/{lineId}/Disruption", "GET")
            .is_some(),
        "spec covers GET /Line/{{lineId}}/Disruption (the script tool's per-line call)"
    );
}

/// OAPI-08 / D-04 — the binding parity assertion, OFFLINE via wiremock.
///
/// Drives the REAL binary pipeline ([`run_serving`]) against a wiremock backend
/// that REQUIRES `app_key=dummy` and replays `london-tube-scenarios.yaml` through
/// `mcp-tester`, gating per-step. The wiremock `query_param("app_key", "dummy")`
/// matchers + the post-replay request inspection prove the api_key query-param
/// auth AND the `${TFL_APP_KEY}` → `app_key=dummy` secret expansion (never the
/// literal `${...}`).
// Why (clippy::await_holding_lock): holding the guard across the awaits is the
// POINT of `tfl_env_lock`, not an oversight. `TFL_BASE_URL`/`TFL_APP_KEY` are
// read once, at assembly time, inside the awaited `run_serving` — so the lock
// must still be held when that await completes, or a concurrent test's
// `set_var` could land in between and this "offline" replay would silently
// target live TfL. An async-aware mutex would not help: the hazard is the
// process-global environment, and these tests are run `--test-threads=1`
// precisely so the guard is uncontended. This lint predates Phase 121 (verified
// against the pre-lift file, which emits the identical warning) and is
// unreachable by `make lint`, which lints only the root `pmcp` package.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn london_tube_parity_through_real_binary_path() {
    // Serialize with parity_live_tfl — both mutate the same process-global
    // variables, and assembly-time resolution must see THIS test's values.
    let _env_lock = tfl_env_lock();

    // The fixture's `${TFL_APP_KEY}` resolves from the process env at auth
    // provider construction time — set it BEFORE spawning the server.
    std::env::set_var("TFL_APP_KEY", DUMMY_APP_KEY);

    // (1) Stand up the wiremock backend with app_key=dummy-gated matchers.
    let backend = MockServer::start().await;
    mount_london_tube(&backend, DUMMY_APP_KEY).await;

    // (2) Point the fixture at wiremock through the ENDPOINT SLOT (PKG-03), not
    // through string surgery over a baked literal. `base_url` is
    // `${TFL_BASE_URL}` in the committed fixture, so the config is copied
    // VERBATIM and the environment supplies the endpoint — exactly the
    // mechanism a real deployment uses. Both variables resolve at dispatch
    // time, so both must be set before `run_serving` assembles the server.
    std::env::set_var("TFL_BASE_URL", backend.uri());

    let tmp = tempfile::tempdir().expect("create tempdir");
    let config_path = tmp.path().join("london-tube.toml");
    std::fs::copy(fixtures_dir().join("london-tube.toml"), &config_path)
        .expect("copy london-tube.toml");

    // (3) The REAL binary path: programmatic Args → run_serving (NO --spec; the
    // reference instance ships none — D-03 curated path). Ephemeral loopback port.
    let args = Args {
        config: config_path,
        spec: None,
        http: "127.0.0.1:0".to_string(),
    };
    let (bound, handle) = tokio::time::timeout(Duration::from_secs(10), run_serving(&args))
        .await
        .expect("run_serving must not hang")
        .expect("REAL binary path must assemble + serve the london-tube config");

    // (4) Replay london-tube-scenarios.yaml via the mcp-tester library.
    let url = format!("http://{bound}");
    let mut tester = ServerTester::new(
        &url,
        Duration::from_secs(30),
        false,        // insecure
        None,         // api_key
        Some("http"), // force_transport
        None,         // http_middleware_chain
    )
    .expect("construct ServerTester for the spawned HTTP server");

    let mut initialized = false;
    for attempt in 0..20u32 {
        if matches!(
            tester.test_initialize().await.status,
            mcp_tester::report::TestStatus::Passed
        ) {
            initialized = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50 * u64::from(attempt + 1))).await;
    }
    assert!(initialized, "MCP initialize must succeed (readiness)");

    let scenario = TestScenario::from_file(fixtures_dir().join("london-tube-scenarios.yaml"))
        .expect("load the london-tube parity contract");
    let mut exec = ScenarioExecutor::new(&mut tester, true /* detailed */);
    let result = exec
        .execute(scenario)
        .await
        .expect("scenario execution must complete without a harness error");

    // (5) PER-STEP GATE: every parity step must pass its own assertions (tool
    // list parity + tool-output parity). We gate on each `step_results[i].success`
    // (the per-step truth) rather than the aggregate.
    let failed: Vec<_> = result
        .step_results
        .iter()
        .filter(|s| !s.success)
        .map(|s| (&s.step_name, &s.error))
        .collect();
    assert!(
        failed.is_empty(),
        "every london-tube parity step must pass — tool list + tool outputs must \
         match the reference scenarios. {}/{} completed; failed={failed:#?}",
        result.steps_completed,
        result.steps_total,
    );

    // (6) SECRET-EXPANSION PROOF (T-90-07-04): the backend RECEIVED `app_key=dummy`
    // (the matchers already enforced this — every served response REQUIRED it), and
    // NO recorded request URL carries the literal `${TFL_APP_KEY}` placeholder.
    let recorded = backend
        .received_requests()
        .await
        .expect("wiremock records requests");
    assert!(
        !recorded.is_empty(),
        "the parity replay must have hit the backend at least once (proving the \
         tool calls reached wiremock — not a no-op)"
    );
    for req in &recorded {
        let full = req.url.to_string();
        assert!(
            full.contains("app_key=dummy"),
            "every backend request carries the RESOLVED app_key=dummy (api_key \
             query-param auth, D-04): {full}"
        );
        assert!(
            !full.contains("%24%7BTFL_APP_KEY%7D") && !full.contains("${TFL_APP_KEY}"),
            "the literal ${{TFL_APP_KEY}} placeholder must NEVER reach the wire — \
             it must be RESOLVED to `dummy` (T-90-07-04): {full}"
        );
    }

    // (7) required=false behavior (Codex LOW): the api_key was SET (`dummy`), so it
    // IS present in every request (asserted above). The UNSET case — where an unset
    // `${TFL_APP_KEY}` makes a `required=false` api_key OMITTED rather than sent as
    // the literal placeholder — is covered by the toolkit unit test
    // `test_api_key_query_param_unset_ref_is_omitted` (auth.rs), the receiving end
    // of the same expansion path exercised here.

    // Bounded shutdown — no leaked spawned server.
    handle.abort();
}

/// OAPI-08 (live) — the SAME scenarios against the REAL `https://api.tfl.gov.uk`,
/// double-gated (`#[ignore]` + `PMCP_OPENAPI_LIVE_TEST=1` + a real `TFL_APP_KEY`).
///
/// Skips cleanly in credential-less / offline CI (the env early-return), and is
/// authentic for an operator who opts in with a real key. Mirrors the Phase
/// 84/86 double-gate (offline default, live env-gated).
///
/// Run with:
/// ```sh
/// PMCP_OPENAPI_LIVE_TEST=1 TFL_APP_KEY=<real-key> \
///   cargo test -p pmcp-openapi-server --test parity_replay parity_live_tfl \
///   -- --ignored --test-threads=1
/// ```
// Why (clippy::await_holding_lock): same rationale as the offline twin above —
// the guard must outlive the awaited `run_serving` assembly, and these tests
// are single-threaded by construction. Pre-existing; not introduced by the
// Phase 121 D-02 helper lift.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
#[ignore = "live network — requires PMCP_OPENAPI_LIVE_TEST=1 + a real TFL_APP_KEY"]
async fn parity_live_tfl() {
    // Serialize with the offline parity test — see TFL_ENV_LOCK. Taken BEFORE
    // the gates below so even the skip paths cannot interleave with a
    // concurrent offline run's env reads.
    let _env_lock = tfl_env_lock();

    // Double-gate: even when run with --ignored, bail unless explicitly enabled
    // AND a real key is present (never hit the live API by accident).
    if std::env::var("PMCP_OPENAPI_LIVE_TEST").ok().as_deref() != Some("1") {
        eprintln!("parity_live_tfl skipped: set PMCP_OPENAPI_LIVE_TEST=1 to enable");
        return;
    }
    let Ok(app_key) = std::env::var("TFL_APP_KEY") else {
        eprintln!("parity_live_tfl skipped: set TFL_APP_KEY to a real TfL key");
        return;
    };
    if app_key.trim().is_empty() {
        eprintln!("parity_live_tfl skipped: TFL_APP_KEY is empty");
        return;
    }

    // Use the vendored fixture VERBATIM. Its `base_url` is the
    // `${TFL_BASE_URL}` endpoint slot (PKG-03), so the LIVE path supplies the
    // real TfL endpoint through the environment — the same mechanism the
    // offline replay uses to supply wiremock's.
    std::env::set_var("TFL_BASE_URL", "https://api.tfl.gov.uk");

    let tmp = tempfile::tempdir().expect("create tempdir");
    let config_path = tmp.path().join("london-tube.toml");
    std::fs::copy(fixtures_dir().join("london-tube.toml"), &config_path)
        .expect("copy london-tube.toml");

    let args = Args {
        config: config_path,
        spec: None,
        http: "127.0.0.1:0".to_string(),
    };
    let (bound, handle) = run_serving(&args)
        .await
        .expect("REAL binary path must serve against the live TfL backend");

    let url = format!("http://{bound}");
    let mut tester = ServerTester::new(
        &url,
        Duration::from_secs(60),
        false,
        None,
        Some("http"),
        None,
    )
    .expect("construct ServerTester for the spawned HTTP server");
    assert!(
        matches!(
            tester.test_initialize().await.status,
            mcp_tester::report::TestStatus::Passed
        ),
        "MCP initialize must succeed against the live-backed server"
    );

    let scenario = TestScenario::from_file(fixtures_dir().join("london-tube-scenarios.yaml"))
        .expect("load the london-tube parity contract");
    let mut exec = ScenarioExecutor::new(&mut tester, true);
    let result = exec
        .execute(scenario)
        .await
        .expect("live scenario execution must complete without a harness error");

    // Tool list parity is deterministic; tool-OUTPUT value assertions
    // ("Victoria" / "Severe delays") may legitimately vary against the live API
    // (real-time status), so gate only on the capability-discovery steps here.
    let discovery_failed: Vec<_> = result
        .step_results
        .iter()
        .filter(|s| s.step_name.starts_with("List ") || s.step_name.starts_with("Tools include"))
        .filter(|s| !s.success)
        .map(|s| (&s.step_name, &s.error))
        .collect();
    assert!(
        discovery_failed.is_empty(),
        "live parity: the tool surface must match the reference. failed={discovery_failed:#?}"
    );

    handle.abort();
}
