//! Phase 79 Wave 1: post-deploy verification schema types.
//!
//! Defines the `[post_deploy_tests]` block that operators add to
//! `.pmcp/deploy.toml` plus the typed [`TestOutcome`] enum that Plan 79-03's
//! verifier consumes (data sourced from the Wave-0
//! `mcp_tester::PostDeployReport` JSON contract).
//!
//! ## Why this module exists
//!
//! Cost-coach's Failure Mode C (proven 2026-04-23) was: widget bundle deployed
//! correctly to Lambda, but the JS SDK was misconfigured (missing `onteardown`
//! handler) — runtime broken, deploy reported "successful". Wave 3's
//! orchestrator probes the live endpoint via the existing
//! `cargo pmcp test {check, conformance, apps}` subcommands and consumes their
//! `--format=json` output (Wave-0 `PostDeployReport`) to render a typed
//! verdict.
//!
//! ## Phase 76 IamConfig precedent (mirrored here)
//!
//! `Option::is_none` skip on `DeployConfig::post_deploy_tests` preserves
//! byte-identity for files lacking the `[post_deploy_tests]` section.
//!
//! ## Revision-3 supersessions
//!
//! - **HIGH-G2 (rollback hard-reject):** [`OnFailure`] has variants `{Fail,
//!   Warn}` ONLY. The string `"rollback"` is hard-rejected by both the
//!   custom [`Deserialize`] impl AND the [`std::str::FromStr`] impl with the
//!   verbatim error in [`ROLLBACK_REJECT_MESSAGE`]. Operators who explicitly
//!   configure rollback must change to `"fail"` or `"warn"` — the previously-
//!   planned reserve-but-warn-then-fallback was a UX trap (operators assume
//!   rollback happened and ignore the broken-but-live state). Future phase
//!   that verifies `DeployTarget::rollback()` impls will add the variant +
//!   migration note.
//! - **HIGH-C2 (auth handled by child):** [`InfraErrorKind`] has variants
//!   `{Subprocess, Timeout, AuthOrNetwork}` ONLY — no `AuthMissing` variant.
//!   Subprocesses inherit the parent's env via Tokio Command default and
//!   self-resolve auth via the existing `AuthMethod::None` path that already
//!   covers Phase 74 cache + automatic refresh.

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};

/// Config for the post-deploy verification lifecycle (Phase 79).
///
/// Fields default to the values documented in `79-CONTEXT.md` "Post-deploy
/// verification". The whole block is wrapped in
/// `Option<PostDeployTestsConfig>` on `DeployConfig` so files lacking the
/// section round-trip byte-identically.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostDeployTestsConfig {
    /// Whether post-deploy verification runs at all. Default `true`.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Which checks to run, in order. Default
    /// `["connectivity", "conformance", "apps"]`.
    #[serde(default = "default_checks")]
    pub checks: Vec<String>,

    /// Which apps-validation strict mode to use. Default
    /// [`AppsMode::ClaudeDesktop`].
    #[serde(default)]
    pub apps_mode: AppsMode,

    /// What to do when a verification check fails. Default [`OnFailure::Fail`].
    #[serde(default)]
    pub on_failure: OnFailure,

    /// Per-test subprocess timeout (seconds). Default 60.
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,

    /// Pre-test wait after Lambda hot-swap to let pooled connections drain
    /// (milliseconds). Default 2000.
    #[serde(default = "default_warmup_grace_ms")]
    pub warmup_grace_ms: u64,
}

fn default_enabled() -> bool {
    true
}

fn default_checks() -> Vec<String> {
    vec![
        "connectivity".to_string(),
        "conformance".to_string(),
        "apps".to_string(),
    ]
}

fn default_timeout_seconds() -> u64 {
    60
}

fn default_warmup_grace_ms() -> u64 {
    2000
}

impl Default for PostDeployTestsConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            checks: default_checks(),
            apps_mode: AppsMode::default(),
            on_failure: OnFailure::default(),
            timeout_seconds: default_timeout_seconds(),
            warmup_grace_ms: default_warmup_grace_ms(),
        }
    }
}

/// Apps-validation strict mode. The `--apps-mode=` flag on
/// `cargo pmcp test apps` and `cargo pmcp deploy` accepts the same three
/// kebab-case strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AppsMode {
    /// Generic apps validation — no host-specific signals required.
    Standard,
    /// ChatGPT host signals (e.g. structured-content shape).
    Chatgpt,
    /// Claude Desktop host signals — `onteardown` handler, widget meta tags,
    /// etc. Default per `79-CONTEXT.md`. Phase 78 promoted this from
    /// placeholder to a real strict mode.
    #[default]
    ClaudeDesktop,
}

/// Failure-handling policy.
///
/// REVISION 3 (cross-AI review HIGH-G2 supersession): the `Rollback` variant
/// is REMOVED. Both serde deserialization and `FromStr` reject `"rollback"`
/// with the actionable error in [`ROLLBACK_REJECT_MESSAGE`]. The future phase
/// that verifies `DeployTarget::rollback()` impls will add the variant + a
/// migration note to CHANGELOG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OnFailure {
    /// CLI exits nonzero. **The new (broken) Lambda revision STAYS LIVE.**
    ///
    /// CI/CD pipelines that interpret nonzero exit as "auto-rollback me" will
    /// misread this — the deploy DID succeed at the infrastructure level; only
    /// the post-deploy verification failed. To roll back, run
    /// `cargo pmcp deploy rollback --target <target>` manually (note: rollback
    /// implementations across all 4 targets are currently stubs — see Phase 79
    /// deferred items).
    ///
    /// Plan 79-03 augments this with exit code 3 + GitHub Actions
    /// `::error::` annotation for CI/CD-friendly machine signals.
    #[default]
    Fail,

    /// CLI prints the failure banner and exits zero, whatever failed: a test
    /// verdict or an infrastructure error (spawn failure, timeout, unreachable
    /// endpoint). The (potentially broken) revision stays live; pipeline
    /// continues. (Before 0.28.0 an infrastructure error still exited 2.)
    Warn,
}

/// Verbatim hard-reject error for `on_failure="rollback"`. Used by both the
/// custom [`Deserialize`] impl AND the [`std::str::FromStr`] impl so config +
/// CLI share one rejection path. REVISION 3 supersession per CONTEXT.md
/// HIGH-G2.
pub const ROLLBACK_REJECT_MESSAGE: &str =
    "on_failure='rollback' is not yet implemented in this version of cargo-pmcp. \
     Change to 'fail' (default) or 'warn'. Auto-rollback support will land in a \
     future phase that verifies the existing DeployTarget::rollback() trait implementations.";

/// Custom `Deserialize` to hard-reject `"rollback"` at config validation time
/// per HIGH-G2. The default `#[derive(Deserialize)]` would accept any of the
/// listed variants — this impl narrows to `{fail, warn}` and emits the
/// actionable error for `"rollback"`.
impl<'de> Deserialize<'de> for OnFailure {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "fail" => Ok(OnFailure::Fail),
            "warn" => Ok(OnFailure::Warn),
            "rollback" => Err(D::Error::custom(ROLLBACK_REJECT_MESSAGE)),
            other => Err(D::Error::custom(format!(
                "invalid on_failure='{other}' — valid values are 'fail' or 'warn'"
            ))),
        }
    }
}

impl std::str::FromStr for OnFailure {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fail" => Ok(OnFailure::Fail),
            "warn" => Ok(OnFailure::Warn),
            "rollback" => Err(ROLLBACK_REJECT_MESSAGE.to_string()),
            other => Err(format!(
                "invalid on_failure='{other}' — valid values are 'fail' or 'warn'"
            )),
        }
    }
}

/// Result of one subprocess test invocation. Distinguishes verdict-on-new-code
/// ([`Self::TestFailed`]) from infra flakiness ([`Self::InfraError`]) so CI/CD
/// can decide rollback without conflating the two.
///
/// REVISION 3: the [`TestOutcome::Passed::summary`] /
/// [`TestOutcome::TestFailed::summary`] / [`TestOutcome::TestFailed::recipes`]
/// fields are populated from the Wave-0 `mcp_tester::PostDeployReport`
/// (consumed by 79-03's `spawn_test_subprocess`), NOT from regex-parsing
/// pretty terminal output. This eliminates the F-2 / N-1 / N-3 brittleness
/// chain of revisions 1 + 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestOutcome {
    /// Subprocess exited 0. `summary` carries the pass-count from the JSON
    /// report when applicable; `None` otherwise (e.g., connectivity check is
    /// binary so no N/M counts apply).
    Passed {
        /// Optional pass-count metric.
        summary: Option<TestSummary>,
    },
    /// Subprocess exited 1 (verdict on the new code).
    TestFailed {
        /// Single-line label / detail (e.g. `"apps"`, `"conformance"`).
        label: String,
        /// Pass-count metric extracted from JSON `PostDeployReport.summary`.
        summary: Option<TestSummary>,
        /// Per-failure reproduction commands extracted from JSON
        /// `PostDeployReport.failures[].reproduce`.
        recipes: Vec<FailureRecipe>,
        /// Per-failure messages from JSON `PostDeployReport.failures[].message`
        /// (e.g. `... Request failed with status: 403 Forbidden`). The banner
        /// reads them for its hints (debug session
        /// `cargo-pmcp-deploy-targets`, findings #8 and #12).
        messages: Vec<String>,
    },
    /// Subprocess failed at the infrastructure level — child failed to spawn,
    /// timed out, or reported a network/auth error via the JSON outcome.
    InfraError(InfraErrorKind, String),
}

/// Pass/total count populated from the Wave-0 `mcp_tester::PostDeployReport.summary`.
///
/// We KEEP a local 2-bucket struct (passed/total) for the banner formatter —
/// the upstream `mcp_tester::TestSummary` carries 5 buckets but the banner
/// only renders passed/total. Wave 3's JSON consumer maps
/// `mcp_tester::TestSummary { passed, total, .. }` → this 2-bucket form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestSummary {
    /// Number of passed checks.
    pub passed: u32,
    /// Total number of checks.
    pub total: u32,
}

/// Reproduction command for a single failing tool. Populated from
/// `mcp_tester::FailureDetail.reproduce` (Wave 0 contract). Documentation-only
/// — never `eval`'d, never `Command::spawn`'d (T-79-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureRecipe {
    /// Verbatim copy-paste-able `cargo pmcp test ...` command line.
    pub command: String,
}

/// Infrastructure-error classification.
///
/// REVISION 3 (HIGH-C2 supersession): `AuthMissing` variant REMOVED.
/// Subprocesses inherit the parent's env via Tokio Command default and
/// self-resolve auth via the existing `AuthMethod::None` path at
/// `cargo-pmcp/src/commands/auth.rs:106-109`, which already supports Phase 74
/// cache + automatic refresh. The orchestrator no longer pre-checks auth
/// presence; a child 401 surfaces as the child's own non-zero exit (mapped
/// to [`Self::AuthOrNetwork`] if the JSON output exists, or [`Self::Subprocess`]
/// if not).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfraErrorKind {
    /// Child process failed to spawn or exited via signal.
    Subprocess,
    /// Subprocess exceeded `timeout_seconds`.
    Timeout,
    /// Child reported network/auth failure (exit code 2-ish OR JSON
    /// `outcome=infra-error`).
    AuthOrNetwork,
}

// ============================================================================
// Phase 79 Wave 3 (Plan 79-03): subprocess orchestrator implementation.
// ============================================================================
//
// The remainder of this file implements the imperative post-deploy verifier
// orchestrator that consumes the Wave-0 `mcp_tester::PostDeployReport` JSON
// contract via subprocess invocation. See `<objective>` of Plan 79-03 for the
// full design rationale.
//
// REVISION 3 supersessions applied:
// - HIGH-1: subprocess argv includes `--format=json`; stdout parsed via
//   `serde_json::from_str::<PostDeployReport>`. NO regex parsing.
// - HIGH-C2: subprocess inherits parent env via Tokio Command default. NO
//   `--api-key` argv. NO `MCP_API_KEY` injection by parent. Child resolves
//   auth via existing `AuthMethod::None` Phase 74 cache + refresh path.
// - HIGH-G2: `OnFailure::Rollback` is hard-rejected at parse time — orchestrator
//   matches exhaustively on `{Fail, Warn}`.
// - HIGH-2: distinct exit codes (`BrokenButLive=3`, `InfraError=2`); CI
//   annotation emitted to stderr when `CI=true`.

use mcp_tester::post_deploy_report::{
    PostDeployReport, TestCommand as JsonTestCommand, TestOutcome as JsonOutcome,
};
use std::time::Duration;
use std::time::Instant;
use tokio::process::Command;
use tokio::time::{sleep, timeout};

/// Resolve the executable path to spawn for `cargo pmcp test ...` subprocesses.
///
/// In production this is `std::env::current_exe()` (the running cargo-pmcp
/// binary itself). Under `cfg(test)` the value of the `PMCP_TEST_FIXTURE_EXE`
/// environment variable, when set, takes precedence — this is the integration-
/// test injection point that swaps in a deterministic mock binary at
/// `cargo-pmcp/tests/fixtures/mock_test_binary.rs`.
fn resolve_test_subprocess_exe() -> std::io::Result<std::path::PathBuf> {
    if cfg!(test) || std::env::var_os("PMCP_TEST_FIXTURE_EXE").is_some() {
        if let Some(p) = std::env::var_os("PMCP_TEST_FIXTURE_EXE") {
            return Ok(std::path::PathBuf::from(p));
        }
    }
    std::env::current_exe()
}

/// Subprocess invocation for `cargo pmcp test check`. Cog ≤6.
///
/// REVISION 3 HIGH-1: argv now includes `--format=json` (Plan 79-05 contract).
/// REVISION 3 HIGH-C2: NO `--api-key` arg; child inherits parent env and
/// resolves auth via the existing `AuthMethod::None` Phase 74 cache + refresh
/// path at `cargo-pmcp/src/commands/auth.rs:99-135`.
pub async fn run_check(url: &str, timeout_secs: u64) -> TestOutcome {
    spawn_test_subprocess(
        &["test", "check", url, "--format=json"],
        timeout_secs,
        "Connectivity",
    )
    .await
}

/// Subprocess invocation for `cargo pmcp test conformance`. Cog ≤6.
pub async fn run_conformance(url: &str, timeout_secs: u64) -> TestOutcome {
    spawn_test_subprocess(
        &["test", "conformance", url, "--format=json"],
        timeout_secs,
        "Conformance",
    )
    .await
}

/// Subprocess invocation for `cargo pmcp test apps`. Cog ≤8.
pub async fn run_apps(url: &str, mode: AppsMode, timeout_secs: u64) -> TestOutcome {
    let mode_arg = match mode {
        AppsMode::Standard => "standard",
        AppsMode::Chatgpt => "chatgpt",
        AppsMode::ClaudeDesktop => "claude-desktop",
    };
    spawn_test_subprocess(
        &["test", "apps", url, "--mode", mode_arg, "--format=json"],
        timeout_secs,
        "Apps validation",
    )
    .await
}

/// Shared spawn helper. Cog ≤15.
///
/// REVISION 3 HIGH-1: parses `PostDeployReport` from stdout via `serde_json`.
/// REVISION 3 HIGH-C2: NO `MCP_API_KEY` parent-side injection. Tokio Command
/// default inherits parent env unchanged.
async fn spawn_test_subprocess(
    args: &[&str],
    timeout_secs: u64,
    label: &'static str,
) -> TestOutcome {
    let exe = match resolve_test_subprocess_exe() {
        Ok(p) => p,
        Err(e) => {
            return TestOutcome::InfraError(
                InfraErrorKind::Subprocess,
                format!("Failed to resolve current executable: {e}"),
            );
        },
    };

    // REVISION 3 HIGH-C2: NO env injection. Child inherits parent's full env
    // (Tokio Command default — no env_clear() call). Child's existing
    // AuthMethod::None path resolves auth via Phase 74 cache + automatic refresh.
    let mut cmd = Command::new(&exe);
    cmd.args(args);
    cmd.stdout(std::process::Stdio::piped());
    // stderr inherits → live progress visible to the user.

    // 16 MiB cap on subprocess stdout — bounds memory if a misbehaving test
    // command floods stdout (e.g., debug logs, attacker-controlled argv).
    // The structured PostDeployReport JSON for typical conformance/apps runs
    // is well under 100 KB.
    const MAX_STDOUT_BYTES: u64 = 16 * 1024 * 1024;

    let fut = async {
        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take();
        let mut stdout_buf = Vec::with_capacity(64 * 1024);
        if let Some(s) = stdout {
            use tokio::io::AsyncReadExt;
            let _ = s.take(MAX_STDOUT_BYTES).read_to_end(&mut stdout_buf).await;
        }
        let status = child.wait().await?;
        let stdout_str = String::from_utf8_lossy(&stdout_buf).into_owned();
        Ok::<(std::process::ExitStatus, String), std::io::Error>((status, stdout_str))
    };

    match timeout(Duration::from_secs(timeout_secs), fut).await {
        Ok(Ok((status, stdout_buf))) => parse_subprocess_result(label, status, &stdout_buf),
        Ok(Err(e)) => TestOutcome::InfraError(
            InfraErrorKind::Subprocess,
            format!("{label} subprocess spawn failed: {e}"),
        ),
        Err(_) => TestOutcome::InfraError(
            InfraErrorKind::Timeout,
            format!("{label} exceeded {timeout_secs}s timeout"),
        ),
    }
}

/// REVISION 3 HIGH-1: typed JSON parser for the Plan 79-05 contract. Cog ≤12.
fn parse_subprocess_result(
    label: &'static str,
    status: std::process::ExitStatus,
    stdout_buf: &str,
) -> TestOutcome {
    // Try typed JSON parse. Malformed JSON → InfraError (don't crash the verifier).
    let report = match serde_json::from_str::<PostDeployReport>(stdout_buf) {
        Ok(r) => r,
        Err(e) => {
            return TestOutcome::InfraError(
                InfraErrorKind::Subprocess,
                format!("{label} subprocess produced unparseable JSON: {e}"),
            );
        },
    };

    match report.outcome {
        JsonOutcome::Passed => {
            let summary = report.summary.map(map_upstream_summary);
            TestOutcome::Passed { summary }
        },
        JsonOutcome::TestFailed => {
            let summary = report.summary.map(map_upstream_summary);
            let recipes: Vec<FailureRecipe> = report
                .failures
                .iter()
                .map(|f| FailureRecipe {
                    command: f.reproduce.clone(),
                })
                .collect();
            let messages = report.failures.iter().map(|f| f.message.clone()).collect();
            TestOutcome::TestFailed {
                label: label.to_string(),
                summary,
                recipes,
                messages,
            }
        },
        JsonOutcome::InfraError => {
            let msg = report
                .failures
                .first()
                .map(|f| f.message.clone())
                .unwrap_or_else(|| format!("{label} reported infra-error"));
            let kind = if status.code() == Some(2) {
                InfraErrorKind::AuthOrNetwork
            } else {
                InfraErrorKind::Subprocess
            };
            TestOutcome::InfraError(kind, msg)
        },
    }
}

/// Map upstream 5-bucket `mcp_tester::TestSummary` → local 2-bucket form.
/// Cog ≤2.
fn map_upstream_summary(s: mcp_tester::TestSummary) -> TestSummary {
    TestSummary {
        passed: u32::try_from(s.passed).unwrap_or(u32::MAX),
        total: u32::try_from(s.total).unwrap_or(u32::MAX),
    }
}

/// Wrap a single test invocation with a single retry-after-1s mitigation
/// for Lambda alias-swap pooled-connection stragglers (RESEARCH.md Pitfall 3).
/// Cog ≤6.
pub async fn run_with_single_retry<F, Fut>(invoke: F, label: &str, quiet: bool) -> TestOutcome
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = TestOutcome>,
{
    let first = invoke().await;
    match first {
        TestOutcome::Passed { .. } => first,
        TestOutcome::TestFailed { .. }
        | TestOutcome::InfraError(InfraErrorKind::AuthOrNetwork, _) => {
            sleep(Duration::from_millis(1000)).await;
            let second = invoke().await;
            if matches!(second, TestOutcome::Passed { .. }) && !quiet {
                eprintln!(
                    "  INFO: {label} first attempt failed; retry passed \
                     (consider increasing warmup_grace_ms)"
                );
            }
            second
        },
        other => other, // Don't retry timeouts or true infra failures
    }
}

/// Format the LIVE-but-broken failure banner per CONTEXT.md "Specifics" lines
/// 184-198.
///
/// REVISION 3 HIGH-1: consumes typed `PostDeployReport`-sourced summary/recipes
/// directly. Noun (`tests`/`widgets`) dispatched from the per-step
/// [`JsonTestCommand`] (sourced from `PostDeployReport.command`). Cog ≤10.
pub fn format_failure_banner_from_report(
    target_id: &str,
    outcomes: &[(String, JsonTestCommand, TestOutcome, Option<u64>)],
) -> String {
    let mut out = String::new();
    out.push_str("\nRunning post-deploy verification:\n");
    for (name, command, outcome, dur) in outcomes {
        format_one_step(&mut out, name, *command, outcome, *dur);
    }
    let any_failed = outcomes
        .iter()
        .any(|(_, _, o, _)| !matches!(o, TestOutcome::Passed { .. }));
    if any_failed {
        out.push_str("\n⚠ The deployed version IS LIVE and contains issues.\n");
        out.push_str(&format!(
            "  To roll back: cargo pmcp deploy rollback --target {target_id}\n"
        ));
    }
    out
}

/// Render one step line + any reproduce sub-lines into `out`. Cog ≤8.
fn format_one_step(
    out: &mut String,
    name: &str,
    command: JsonTestCommand,
    outcome: &TestOutcome,
    dur: Option<u64>,
) {
    let mark = match outcome {
        TestOutcome::Passed { .. } => "✓",
        _ => "✗",
    };
    // Noun dispatched from typed command (REVISION 3 HIGH-1).
    let noun = match command {
        JsonTestCommand::Apps => "widgets",
        _ => "tests",
    };
    let metric = render_step_metric(outcome, dur, noun);
    out.push_str(&format!("  {mark} {name:<22} {metric}\n"));
    if let TestOutcome::TestFailed { recipes, .. } = outcome {
        if recipes.is_empty() {
            out.push_str(&format!(
                "     reproduce: cargo pmcp test {} <URL>\n",
                name.to_lowercase().replace(" validation", "")
            ));
        } else {
            for recipe in recipes {
                // VERBATIM from PostDeployReport.failures[].reproduce — no reconstruction.
                out.push_str(&format!("     reproduce: {}\n", recipe.command));
            }
        }
    }
}

/// Render one step's metric annotation. Cog ≤6.
fn render_step_metric(outcome: &TestOutcome, dur: Option<u64>, noun: &'static str) -> String {
    match outcome {
        TestOutcome::Passed {
            summary: Some(s), ..
        } => format!("({}/{} {} passed)", s.passed, s.total, noun),
        TestOutcome::TestFailed {
            summary: Some(s), ..
        } => {
            let failed = s.total.saturating_sub(s.passed);
            format!("({}/{} {} failed)", failed, s.total, noun)
        },
        _ => dur.map(|d| format!("({d}ms)")).unwrap_or_default(),
    }
}

/// REVISION 3 HIGH-2 supersession: emit a GitHub Actions / GitLab-friendly
/// `::error::` annotation to stderr when running in CI. Auto-detects via
/// `CI` env var (set by GitHub Actions, GitLab, CircleCI, Travis, etc.).
/// AUGMENTS the loud banner — does NOT replace it. Cog ≤4.
pub fn emit_ci_annotation(target_id: &str, exit_code: i32) {
    if std::env::var("CI").is_err() {
        return;
    }
    let mut sink = std::io::stderr();
    let _ = write_ci_annotation(&mut sink, target_id, exit_code);
}

/// Internal writer-targeted CI annotation emitter (testable). Cog ≤2.
fn write_ci_annotation<W: std::io::Write>(
    sink: &mut W,
    target_id: &str,
    exit_code: i32,
) -> std::io::Result<()> {
    writeln!(
        sink,
        "::error::Deployment succeeded but post-deploy tests failed (exit code {exit_code}). \
         Lambda revision is LIVE. To roll back: cargo pmcp deploy rollback --target {target_id}"
    )
}

// ============================================================================
// Top-level orchestrator (Task 2 of Plan 79-03)
// ============================================================================

/// Result of the orchestrator. Exit-code mapping (see [`verdict_exit_code`]):
///
/// - [`OrchestrationFailure::BrokenButLive`] (test failed against live revision): exit 3
///   (REVISION 3 HIGH-2), `on_failure = fail` only
/// - [`OrchestrationFailure::InfraError`]: exit 2, `on_failure = fail` only
/// - `Ok(())`: exit 0 — every check passed, or `on_failure = warn`
///
/// REVISION 3 HIGH-2 rename: `TestFailed` → `BrokenButLive` to reflect that
/// post-deploy failures are inherently against a live revision (deploy already
/// reported success before this verifier ran). Pre-cutover failures take a
/// different exit-code path entirely (existing `anyhow::Error` from
/// `target.deploy()`).
#[derive(Debug)]
pub enum OrchestrationFailure {
    /// Post-deploy tests failed against the LIVE revision. CLI exits 3.
    BrokenButLive {
        /// CLI exit code (always 3 for this variant).
        exit_code: i32,
        /// Pre-formatted failure banner (Display proxies through this).
        banner: String,
    },
    /// Network / spawn / timeout failure. CLI exits 2.
    InfraError {
        /// CLI exit code (always 2 for this variant).
        exit_code: i32,
        /// Pre-formatted failure banner (Display proxies through this).
        banner: String,
    },
}

impl std::fmt::Display for OrchestrationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BrokenButLive { banner, .. } | Self::InfraError { banner, .. } => {
                f.write_str(banner)
            },
        }
    }
}

impl std::error::Error for OrchestrationFailure {}

impl OrchestrationFailure {
    /// CLI exit code for this failure mode.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::BrokenButLive { exit_code, .. } | Self::InfraError { exit_code, .. } => {
                *exit_code
            },
        }
    }
}

/// Internal lifecycle step descriptor. Holds both the human label (for the
/// banner) and the typed [`JsonTestCommand`] (for noun dispatch).
struct RunStep {
    label: String,
    kind: StepKind,
    json_command: JsonTestCommand,
}

enum StepKind {
    Check,
    Conformance,
    Apps,
}

/// Build the ordered run plan from config + widget detection. Cog ≤6.
fn build_run_plan(config: &PostDeployTestsConfig, widgets_present: bool) -> Vec<RunStep> {
    let mut plan = Vec::new();
    if config.checks.iter().any(|c| c == "connectivity") {
        plan.push(RunStep {
            label: "Connectivity".into(),
            kind: StepKind::Check,
            json_command: JsonTestCommand::Check,
        });
    }
    if config.checks.iter().any(|c| c == "conformance") {
        plan.push(RunStep {
            label: "Conformance".into(),
            kind: StepKind::Conformance,
            json_command: JsonTestCommand::Conformance,
        });
    }
    if widgets_present && config.checks.iter().any(|c| c == "apps") {
        plan.push(RunStep {
            label: "Apps validation".into(),
            kind: StepKind::Apps,
            json_command: JsonTestCommand::Apps,
        });
    }
    plan
}

/// Invoke a single lifecycle step (with retry-once mitigation). Cog ≤6.
async fn invoke_step(
    step: &RunStep,
    url: &str,
    config: &PostDeployTestsConfig,
    quiet: bool,
) -> TestOutcome {
    let label = step.label.clone();
    run_with_single_retry(
        || async {
            match step.kind {
                StepKind::Check => run_check(url, config.timeout_seconds).await,
                StepKind::Conformance => run_conformance(url, config.timeout_seconds).await,
                StepKind::Apps => run_apps(url, config.apps_mode, config.timeout_seconds).await,
            }
        },
        &label,
        quiet,
    )
    .await
}

/// One post-deploy step's result: label, typed command, outcome, duration.
pub type StepOutcome = (String, JsonTestCommand, TestOutcome, Option<u64>);

/// The CLI exit code the verification maps to, or `None` for exit 0.
///
/// | `on_failure` | all passed | a test failed | an infrastructure error |
/// |---|---|---|---|
/// | `fail` | 0 | 3 | 2 (wins over 3) |
/// | `warn` | 0 | 0 | 0 |
///
/// `warn` is consulted FIRST (debug session `cargo-pmcp-deploy-targets`, #14):
/// an infrastructure error in a check used to return 2 before the policy was
/// read, so `--on-test-failure warn` exited 2 although it is documented to
/// exit 0. The failure is still printed (see [`failure_banner`]).
#[must_use]
pub fn verdict_exit_code(outcomes: &[StepOutcome], on_failure: OnFailure) -> Option<i32> {
    if on_failure == OnFailure::Warn {
        return None;
    }
    let any_infra = outcomes
        .iter()
        .any(|(_, _, o, _)| matches!(o, TestOutcome::InfraError(..)));
    let any_test_failed = outcomes
        .iter()
        .any(|(_, _, o, _)| matches!(o, TestOutcome::TestFailed { .. }));
    if any_infra {
        Some(2)
    } else if any_test_failed {
        Some(3)
    } else {
        None
    }
}

/// The failure banner the orchestrator prints: the per-step report, hints for
/// failures it recognizes (a 403 behind a container target's ingress, a 404),
/// and under `warn` a note that the deploy exits 0 anyway.
#[must_use]
pub fn failure_banner(target_id: &str, outcomes: &[StepOutcome], on_failure: OnFailure) -> String {
    let mut banner = format_failure_banner_from_report(target_id, outcomes);
    if any_status(outcomes, "403") && is_container_target(target_id) {
        banner.push_str(&origin_policy_hint(target_id));
    }
    if any_status(outcomes, "404") && target_id != "pmcp-run" {
        banner.push_str(MCP_PATH_HINT);
    }
    if on_failure == OnFailure::Warn {
        banner.push_str(
            "  on_failure = warn: the failure is reported and the deploy continues (exit 0).\n",
        );
    }
    banner
}

/// Targets that run the server's own HTTP listener behind a platform ingress,
/// which forwards the public `Host` header to it.
fn is_container_target(target_id: &str) -> bool {
    matches!(target_id, "google-cloud-run" | "azure-container-apps")
}

/// The failure messages of every failed step.
fn failure_messages(outcomes: &[StepOutcome]) -> impl Iterator<Item = &str> {
    outcomes.iter().flat_map(|(_, _, outcome, _)| -> Vec<&str> {
        match outcome {
            TestOutcome::TestFailed { messages, .. } => {
                messages.iter().map(String::as_str).collect()
            },
            TestOutcome::InfraError(_, message) => vec![message.as_str()],
            TestOutcome::Passed { .. } => Vec::new(),
        }
    })
}

/// True when a failed step's message reports HTTP `status` (the client's
/// `Request failed with status: 403 Forbidden` form).
fn any_status(outcomes: &[StepOutcome], status: &str) -> bool {
    let needle = format!("status: {status}");
    failure_messages(outcomes).any(|message| message.contains(&needle))
}

/// Why a container target answers 403 (debug session
/// `cargo-pmcp-deploy-targets`, #12): the SDK's localhost-only origin default.
fn origin_policy_hint(target_id: &str) -> String {
    let mut hint = String::from(
        "\n  The endpoint answered 403 Forbidden. A pmcp server built with \
         pmcp::axum::router_with_config\n  and `allowed_origins: None` accepts only \
         localhost Host/Origin headers (DNS-rebinding\n  protection), so behind the \
         platform ingress every request is refused. Set\n  \
         `allowed_origins: Some(AllowedOrigins::any())` in RouterConfig (or \
         StreamableHttpServerConfig),\n  or an explicit list naming the service URL.\n",
    );
    if target_id == "google-cloud-run" {
        hint.push_str(
            "  Cloud Run also answers 403 to unauthenticated requests when \
             [server] allow_unauthenticated = false.\n",
        );
    }
    hint
}

/// Why an endpoint answers 404 (debug session `cargo-pmcp-deploy-targets`,
/// #8): the probe used a path the server does not serve.
const MCP_PATH_HINT: &str =
    "\n  The endpoint answered 404 Not Found. Set `mcp_path` under [server] in \
     .pmcp/deploy.toml to the\n  path your server serves MCP at (google-cloud-run probes \
     \"/mcp\" when it is unset; \"/\" for the root).\n";

/// Interpret the per-step outcomes into the final orchestrator verdict + emit
/// the SOLE failure-banner eprintln (F-6 mitigation). Cog ≤10.
///
/// REVISION 3 HIGH-2: emits CI annotation alongside the banner.
fn interpret_outcomes(
    target_id: &str,
    outcomes: &[StepOutcome],
    on_failure: OnFailure,
    quiet: bool,
) -> std::result::Result<(), OrchestrationFailure> {
    let any_failed = outcomes
        .iter()
        .any(|(_, _, o, _)| !matches!(o, TestOutcome::Passed { .. }));
    if !any_failed {
        return Ok(());
    }

    let banner = failure_banner(target_id, outcomes, on_failure);
    if !quiet {
        eprintln!("{banner}");
    }

    match verdict_exit_code(outcomes, on_failure) {
        None => Ok(()),
        Some(exit_code) => {
            // REVISION 3 HIGH-2: CI annotation alongside the banner.
            emit_ci_annotation(target_id, exit_code);
            Err(if exit_code == 3 {
                OrchestrationFailure::BrokenButLive { exit_code, banner }
            } else {
                OrchestrationFailure::InfraError { exit_code, banner }
            })
        },
    }
}

/// Top-level lifecycle: warmup → check → conformance → apps (apps optional).
/// Cog ≤10.
///
/// `widgets_present` controls whether the `apps` step runs when configured.
/// `quiet` suppresses the failure-banner eprintln (banner is still returned in
/// the [`OrchestrationFailure`] for callers that want to log it differently).
///
/// REVISION 3 HIGH-C2: no `auth_token` parameter. Subprocesses inherit env via
/// Tokio Command default and self-resolve auth via the existing
/// `AuthMethod::None` Phase 74 cache + refresh path.
pub async fn run_post_deploy_tests(
    url: &str,
    target_id: &str,
    widgets_present: bool,
    config: &PostDeployTestsConfig,
    quiet: bool,
) -> std::result::Result<(), OrchestrationFailure> {
    if !config.enabled {
        // Master plan locked decision #4: skip warmup too when disabled.
        return Ok(());
    }

    // Warmup grace.
    if config.warmup_grace_ms > 0 {
        sleep(Duration::from_millis(config.warmup_grace_ms)).await;
    }

    let mut outcomes: Vec<StepOutcome> = Vec::new();

    let runs = build_run_plan(config, widgets_present);
    for step in runs {
        let started = Instant::now();
        let outcome = invoke_step(&step, url, config, quiet).await;
        let dur = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        outcomes.push((step.label, step.json_command, outcome, Some(dur)));
    }

    interpret_outcomes(target_id, &outcomes, config.on_failure, quiet)
}

#[cfg(test)]
mod tests {
    //! Unit tests covering `<behavior>` Tests 2.1..2.10 of Plan 79-01.
    use super::*;
    use std::str::FromStr;

    /// Test 2.1 (round_trip_no_post_deploy_tests_byte_identical): default
    /// `PostDeployTestsConfig` is the documented defaults. Byte-identity for
    /// the `Option::is_none` skip is exercised in `tests/post_deploy_tests_config.rs`.
    #[test]
    fn defaults_round_trip_through_toml() {
        let cfg = PostDeployTestsConfig::default();
        let serialized = toml::to_string(&cfg).expect("serializes");
        let reparsed: PostDeployTestsConfig = toml::from_str(&serialized).expect("re-parses");
        assert_eq!(reparsed.enabled, cfg.enabled);
        assert_eq!(reparsed.checks, cfg.checks);
        assert_eq!(reparsed.apps_mode, cfg.apps_mode);
        assert_eq!(reparsed.on_failure, cfg.on_failure);
        assert_eq!(reparsed.timeout_seconds, cfg.timeout_seconds);
        assert_eq!(reparsed.warmup_grace_ms, cfg.warmup_grace_ms);
    }

    /// Test 2.2 (parses_explicit_post_deploy_tests_block): operator-shaped
    /// TOML hits every field with documented values.
    #[test]
    fn parses_explicit_post_deploy_tests_block() {
        let toml_str = r#"
enabled = true
checks = ["connectivity", "conformance", "apps"]
apps_mode = "claude-desktop"
on_failure = "fail"
timeout_seconds = 60
warmup_grace_ms = 2000
"#;
        let parsed: PostDeployTestsConfig = toml::from_str(toml_str).expect("parses");
        assert!(parsed.enabled);
        assert_eq!(
            parsed.checks,
            vec![
                "connectivity".to_string(),
                "conformance".to_string(),
                "apps".to_string(),
            ]
        );
        assert_eq!(parsed.apps_mode, AppsMode::ClaudeDesktop);
        assert_eq!(parsed.on_failure, OnFailure::Fail);
        assert_eq!(parsed.timeout_seconds, 60);
        assert_eq!(parsed.warmup_grace_ms, 2000);
    }

    /// Test 2.3 (defaults_match_context_md): the default values are exactly
    /// what `79-CONTEXT.md` documents.
    #[test]
    fn defaults_match_context_md() {
        let cfg = PostDeployTestsConfig::default();
        assert!(cfg.enabled);
        assert_eq!(
            cfg.checks,
            vec![
                "connectivity".to_string(),
                "conformance".to_string(),
                "apps".to_string(),
            ]
        );
        assert_eq!(cfg.apps_mode, AppsMode::ClaudeDesktop);
        assert_eq!(cfg.on_failure, OnFailure::Fail);
        assert_eq!(cfg.timeout_seconds, 60);
        assert_eq!(cfg.warmup_grace_ms, 2000);
    }

    /// Test 2.4 (rollback_hard_rejected — REVISION 3 supersession): given
    /// `on_failure = "rollback"`, deserialization returns `Err` whose Display
    /// contains the verbatim ROLLBACK_REJECT_MESSAGE.
    #[test]
    fn rollback_hard_rejected() {
        let toml_str = r#"on_failure = "rollback""#;
        let err = toml::from_str::<PostDeployTestsConfig>(toml_str)
            .expect_err("rollback must be hard-rejected");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("not yet implemented"),
            "expected hard-reject message, got: {msg}"
        );
        assert!(
            msg.contains("'fail'") && msg.contains("'warn'"),
            "expected actionable migration hints in error, got: {msg}"
        );
    }

    /// Test 2.5 (apps_mode_enum_values): all three documented values parse,
    /// and the default is ClaudeDesktop.
    #[test]
    fn apps_mode_enum_values() {
        #[derive(serde::Deserialize)]
        struct W {
            apps_mode: AppsMode,
        }
        let standard: W = toml::from_str(r#"apps_mode = "standard""#).expect("standard");
        assert_eq!(standard.apps_mode, AppsMode::Standard);
        let chatgpt: W = toml::from_str(r#"apps_mode = "chatgpt""#).expect("chatgpt");
        assert_eq!(chatgpt.apps_mode, AppsMode::Chatgpt);
        let cd: W = toml::from_str(r#"apps_mode = "claude-desktop""#).expect("claude-desktop");
        assert_eq!(cd.apps_mode, AppsMode::ClaudeDesktop);
        assert_eq!(AppsMode::default(), AppsMode::ClaudeDesktop);
    }

    /// Test 2.6 (only_two_on_failure_variants_exist): exhaustive match locks
    /// the variant set. If a third variant is added without updating this
    /// test, compilation fails.
    #[test]
    fn only_two_on_failure_variants_exist() {
        let f = OnFailure::Fail;
        let w = OnFailure::Warn;
        // Exhaustive match — adding a third variant forces a compile error here.
        let label = |v: OnFailure| match v {
            OnFailure::Fail => "fail",
            OnFailure::Warn => "warn",
        };
        assert_eq!(label(f), "fail");
        assert_eq!(label(w), "warn");
    }

    /// Test 2.7 (test_outcome_test_failed_struct_payload_constructs):
    /// `TestFailed { label, summary, recipes, messages }` constructs and a
    /// match on it binds every field.
    #[test]
    fn test_outcome_test_failed_struct_payload_constructs() {
        let outcome = TestOutcome::TestFailed {
            label: "apps".to_string(),
            summary: Some(TestSummary {
                passed: 7,
                total: 8,
            }),
            recipes: vec![FailureRecipe {
                command: "cargo pmcp test apps --url http://x --mode claude-desktop --tool foo"
                    .to_string(),
            }],
            messages: vec!["widget foo has no onteardown".to_string()],
        };
        match &outcome {
            TestOutcome::TestFailed {
                label,
                summary,
                recipes,
                messages,
            } => {
                assert_eq!(messages, &["widget foo has no onteardown"]);
                assert_eq!(label, "apps");
                assert_eq!(summary.unwrap().passed, 7);
                assert_eq!(summary.unwrap().total, 8);
                assert_eq!(recipes.len(), 1);
                assert!(recipes[0].command.contains("--mode claude-desktop"));
                assert!(recipes[0].command.contains("--tool foo"));
            },
            other => panic!("expected TestFailed, got {other:?}"),
        }
    }

    /// Test 2.8 (infra_error_kind_variants — REVISION 3): `InfraErrorKind`
    /// has EXACTLY `{Subprocess, Timeout, AuthOrNetwork}`. No `AuthMissing`
    /// variant. Exhaustive match locks the set.
    #[test]
    fn infra_error_kind_variants() {
        let s = InfraErrorKind::Subprocess;
        let t = InfraErrorKind::Timeout;
        let a = InfraErrorKind::AuthOrNetwork;
        let label = |v: InfraErrorKind| match v {
            InfraErrorKind::Subprocess => "subprocess",
            InfraErrorKind::Timeout => "timeout",
            InfraErrorKind::AuthOrNetwork => "auth-or-network",
        };
        assert_eq!(label(s), "subprocess");
        assert_eq!(label(t), "timeout");
        assert_eq!(label(a), "auth-or-network");
    }

    /// Test 2.9 (test_outcome_passed_struct_payload_constructs): both the
    /// `Some(summary)` and `None` variants of `Passed` construct and bind.
    #[test]
    fn test_outcome_passed_struct_payload_constructs() {
        let with_summary = TestOutcome::Passed {
            summary: Some(TestSummary {
                passed: 8,
                total: 8,
            }),
        };
        match &with_summary {
            TestOutcome::Passed { summary: Some(s) } => {
                assert_eq!(s.passed, 8);
                assert_eq!(s.total, 8);
            },
            other => panic!("expected Passed{{Some}}, got {other:?}"),
        }

        let connectivity = TestOutcome::Passed { summary: None };
        match &connectivity {
            TestOutcome::Passed { summary: None } => {},
            other => panic!("expected Passed{{None}}, got {other:?}"),
        }
    }

    // ---------- Debug session cargo-pmcp-deploy-targets, PR-D ----------

    fn step(outcome: TestOutcome) -> StepOutcome {
        (
            "Connectivity".to_string(),
            JsonTestCommand::Check,
            outcome,
            Some(10),
        )
    }

    fn passed() -> TestOutcome {
        TestOutcome::Passed { summary: None }
    }

    fn test_failed(message: &str) -> TestOutcome {
        TestOutcome::TestFailed {
            label: "Connectivity".to_string(),
            summary: None,
            recipes: vec![],
            messages: vec![message.to_string()],
        }
    }

    fn infra(message: &str) -> TestOutcome {
        TestOutcome::InfraError(InfraErrorKind::AuthOrNetwork, message.to_string())
    }

    /// #14: the exit-code table, every `on_failure` x outcome mix. `warn`
    /// always exits 0 (the field report: Connectivity and Conformance
    /// test-failed, Apps infra-error, `--on-test-failure warn` -> exit 2).
    #[test]
    fn exit_code_matrix() {
        /// `(name, outcomes, exit code under fail, exit code under warn)`.
        type Case = (&'static str, Vec<TestOutcome>, Option<i32>, Option<i32>);
        let cases: [Case; 6] = [
            ("no steps", vec![], None, None),
            ("all passed", vec![passed(), passed()], None, None),
            (
                "a test failed",
                vec![passed(), test_failed("x")],
                Some(3),
                None,
            ),
            ("infra error", vec![infra("x"), passed()], Some(2), None),
            (
                "the 017/019 shape",
                vec![test_failed("x"), test_failed("y"), infra("z")],
                Some(2),
                None,
            ),
            ("infra only", vec![infra("x")], Some(2), None),
        ];
        for (name, outcomes, fail, warn) in cases {
            let steps: Vec<StepOutcome> = outcomes.into_iter().map(step).collect();
            assert_eq!(
                verdict_exit_code(&steps, OnFailure::Fail),
                fail,
                "fail: {name}"
            );
            assert_eq!(
                verdict_exit_code(&steps, OnFailure::Warn),
                warn,
                "warn: {name}"
            );
        }
    }

    /// #14 through the orchestrator's own verdict path (quiet, so nothing is
    /// printed): `warn` returns Ok for an infrastructure error.
    #[test]
    fn warn_turns_an_infrastructure_error_into_success() {
        let steps = vec![step(test_failed("x")), step(infra("y"))];
        assert!(interpret_outcomes("google-cloud-run", &steps, OnFailure::Warn, true).is_ok());
        let failure = interpret_outcomes("google-cloud-run", &steps, OnFailure::Fail, true)
            .expect_err("fail keeps its code");
        assert_eq!(failure.exit_code(), 2);
    }

    /// The failure is still reported under `warn`, and the banner says the
    /// deploy exits 0 anyway.
    #[test]
    fn the_warn_banner_reports_the_failure_and_the_exit_code() {
        let steps = vec![step(infra("connection refused"))];
        let banner = failure_banner("google-cloud-run", &steps, OnFailure::Warn);
        assert!(banner.contains("✗ Connectivity"), "{banner}");
        assert!(banner.contains("IS LIVE"), "{banner}");
        assert!(banner.contains("on_failure = warn"), "{banner}");
        assert!(banner.contains("exit 0"), "{banner}");
        let banner = failure_banner("google-cloud-run", &steps, OnFailure::Fail);
        assert!(!banner.contains("on_failure = warn"), "{banner}");
    }

    const FORBIDDEN: &str =
        "Transport error: Request error: Request failed with status: 403 Forbidden";
    const NOT_FOUND: &str =
        "Transport error: Request error: Request failed with status: 404 Not Found";

    /// #12: a 403 behind a container target's ingress names the origin-policy
    /// fix (and Cloud Run's own authentication).
    #[test]
    fn a_403_on_a_container_target_names_the_origin_policy_fix() {
        for target in ["google-cloud-run", "azure-container-apps"] {
            let banner = failure_banner(target, &[step(test_failed(FORBIDDEN))], OnFailure::Fail);
            assert!(banner.contains("403"), "{target}: {banner}");
            assert!(
                banner.contains("AllowedOrigins::any()"),
                "{target}: {banner}"
            );
            assert!(banner.contains("allowed_origins"), "{target}: {banner}");
        }
        let banner = failure_banner(
            "google-cloud-run",
            &[step(test_failed(FORBIDDEN))],
            OnFailure::Fail,
        );
        assert!(banner.contains("allow_unauthenticated"), "{banner}");
        // An infra-error message carries the status too.
        let banner = failure_banner(
            "google-cloud-run",
            &[step(infra(FORBIDDEN))],
            OnFailure::Fail,
        );
        assert!(banner.contains("AllowedOrigins::any()"), "{banner}");
    }

    /// The origin hint is for container targets only (on a Lambda the SDK sees
    /// the loopback proxy's Host, so a 403 there is not the origin policy).
    #[test]
    fn a_403_elsewhere_and_other_failures_get_no_origin_hint() {
        let banner = failure_banner(
            "aws-lambda",
            &[step(test_failed(FORBIDDEN))],
            OnFailure::Fail,
        );
        assert!(!banner.contains("AllowedOrigins"), "{banner}");
        let banner = failure_banner(
            "google-cloud-run",
            &[step(test_failed("503 Service Unavailable"))],
            OnFailure::Fail,
        );
        assert!(!banner.contains("AllowedOrigins"), "{banner}");
        assert!(!banner.contains("mcp_path"), "{banner}");
    }

    /// #8: a 404 points at `[server] mcp_path` (not on pmcp-run, whose URL
    /// is the platform's).
    #[test]
    fn a_404_names_mcp_path() {
        for target in ["google-cloud-run", "aws-lambda", "azure-container-apps"] {
            let banner = failure_banner(target, &[step(test_failed(NOT_FOUND))], OnFailure::Warn);
            assert!(banner.contains("mcp_path"), "{target}: {banner}");
            assert!(banner.contains("\"/\""), "{target}: {banner}");
        }
        let banner = failure_banner("pmcp-run", &[step(test_failed(NOT_FOUND))], OnFailure::Fail);
        assert!(!banner.contains("mcp_path"), "{banner}");
    }

    fn any_outcome() -> impl proptest::strategy::Strategy<Value = TestOutcome> {
        use proptest::prelude::*;
        prop_oneof![
            Just(passed()),
            "[a-z0-9 ]{0,8}".prop_map(|m| test_failed(&m)),
            "[a-z0-9 ]{0,8}".prop_map(|m| infra(&m)),
        ]
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]

        /// #14: the verdict follows the documented table for any mix of step
        /// outcomes; `warn` never yields a nonzero exit.
        #[test]
        fn exit_code_follows_the_documented_table(
            outcomes in proptest::collection::vec(any_outcome(), 0..6),
            warn in proptest::bool::ANY,
        ) {
            let steps: Vec<StepOutcome> = outcomes.into_iter().map(step).collect();
            let on_failure = if warn { OnFailure::Warn } else { OnFailure::Fail };
            let infra = steps.iter().any(|(_, _, o, _)| matches!(o, TestOutcome::InfraError(..)));
            let failed = steps.iter().any(|(_, _, o, _)| matches!(o, TestOutcome::TestFailed { .. }));
            let expected = match on_failure {
                OnFailure::Warn => None,
                OnFailure::Fail if infra => Some(2),
                OnFailure::Fail if failed => Some(3),
                OnFailure::Fail => None,
            };
            proptest::prop_assert_eq!(verdict_exit_code(&steps, on_failure), expected);
            proptest::prop_assert_eq!(
                interpret_outcomes("google-cloud-run", &steps, on_failure, true)
                    .err()
                    .map(|f| f.exit_code()),
                expected
            );
        }
    }

    /// Test 2.10 (clap_str_parse_for_on_failure — REVISION 3): `FromStr`
    /// rejects `"rollback"` with the same actionable error as serde.
    #[test]
    fn clap_str_parse_for_on_failure() {
        assert_eq!(OnFailure::from_str("fail"), Ok(OnFailure::Fail));
        assert_eq!(OnFailure::from_str("warn"), Ok(OnFailure::Warn));
        let err = OnFailure::from_str("rollback").expect_err("rollback rejected");
        assert!(
            err.contains("not yet implemented"),
            "FromStr must use the same hard-reject message as serde: {err}"
        );
        let err = OnFailure::from_str("nonsense").expect_err("nonsense rejected");
        assert!(
            err.contains("'fail'") && err.contains("'warn'"),
            "unknown variants must list valid options: {err}"
        );
    }
}
