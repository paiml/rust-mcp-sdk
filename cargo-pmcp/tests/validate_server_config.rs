//! Phase 128 Plan 07 — end-to-end coverage for `cargo pmcp validate config`
//! and for `validate deploy`'s D-07 lint extension.
//!
//! # This file IS reached by the gate
//!
//! `make test-cargo-pmcp-integration` names `validate_server_config` in BOTH its
//! `--test` selector list AND its `REQUIRED_TEST_BINARIES` string. Both entries are
//! required and they do different jobs: the first makes the binary RUN, the second
//! makes its absence (or a zero count) a FAILURE. The leg's own `ran -eq 0` guard
//! cannot substitute for the second — the other eight selected binaries keep the
//! summed total nonzero, so a silently-unrun binary here would not move it. That is
//! the same `running 0 tests` class this phase is repairing elsewhere, and it is why
//! `verb_help.rs` carries the same note.
//!
//! # Which stream each assertion reads
//!
//! Deliberate, and matching the commands' documented contract rather than assuming
//! one stream:
//!
//! - **WARNINGS are on stderr.** `validate_deploy` puts its IAM warnings there
//!   (`cargo-pmcp/src/commands/validate.rs:577`, via
//!   `crate::deployment::iam::emit_warnings`) and the new lint findings use the same
//!   `warning:` prefix on the same stream, so a reviewer piping stdout still sees
//!   them.
//! - **RESULTS and SUMMARIES are on stdout** — the banner, the `✓` result line, and
//!   the finding-count heading.
//!
//! Assertions check exit codes and the PRESENCE of expected content, never exact
//! formatting: `console::style` output and clap's help wrapping are both free to
//! change without this file being wrong.
//!
//! # These tests run the BINARY, not the handler
//!
//! `cargo_pmcp::commands` is not re-exported from `cargo-pmcp/src/lib.rs` (the lib
//! surface is intentionally minimal), so an integration test cannot call
//! `validate_server_config` directly. Driving the built binary is also what lets the
//! cwd-relative discovery path be exercised safely — a child process gets its own
//! working directory, where `std::env::set_current_dir` in a unit test would be
//! process-global and race with the `--lib` leg's parallel threads
//! (`Makefile:340` runs it WITHOUT `--test-threads=1`).

use assert_cmd::Command;
// `PredicateBooleanExt` is what supplies `.not()` on the `contains(..)` predicates
// used by the "must print nothing extra" assertions.
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

/// A minimal valid `[server]` header, prepended to every fixture.
const SERVER_HEADER: &str = r#"
[server]
name = "demo-server"
version = "0.1.0"
"#;

/// A `POST` tool's non-path string parameter is BODY position, where the D3 default
/// cap deliberately does not apply (free text must keep working) — so it is an
/// uncapped-string lint finding rather than an error.
const UNCAPPED_BODY_STRING: &str = r#"
[[tools]]
name = "post-comment"
description = "Post a comment."
path = "/issues/1/comments"
method = "POST"

[[tools.parameters]]
name = "body_text"
type = "string"
description = "Free text."
"#;

/// A parameter whose `pattern` does not compile as a Draft 2020-12 regex. This is a
/// HARD config error (SC-2), not a lint finding.
const NON_COMPILING_PATTERN: &str = r#"
[[tools]]
name = "lookup"
description = "Look something up."
path = "/things/{id}"
method = "GET"

[[tools.parameters]]
name = "id"
type = "string"
description = "Identifier."
pattern = "([unclosed"
"#;

/// A `[backend]` section, present in every single-call HTTP and OpenAPI config.
///
/// REGRESSION form of the `http`-feature requirement on the `pmcp-server-toolkit`
/// dependency (`cargo-pmcp/Cargo.toml`): `ServerConfig.backend` is
/// `#[cfg(feature = "http")]` and `ServerConfig` carries
/// `#[serde(deny_unknown_fields)]`, so a toolkit compiled WITHOUT `http` rejects
/// this fixture with `unknown field 'backend'`. Kept as a test rather than as a
/// one-off measurement, because a measurement taken against a SQL-only fixture
/// would have concluded `http` was unnecessary.
const BACKEND_SECTION: &str = r#"
[backend]
base_url = "https://example.invalid"
"#;

/// The deploy-document stanzas `.pmcp/deploy.toml` needs, kept in ONE place.
const DEPLOY_FIXTURE_HEADER: &str = r#"
[target]
type = "aws-lambda"
version = "1.0.0"

[aws]
region = "us-west-2"

[server]
name = "demo-server"
memory_mb = 512
timeout_seconds = 30

[environment]

[auth]
enabled = false

[observability]
log_retention_days = 30
enable_xray = false
create_dashboard = false

[[iam.tables]]
name = "demo-table"
actions = ["read"]
"#;

/// A fixture project: an optional toolkit `config.toml` at the root and an optional
/// `.pmcp/deploy.toml`.
fn project(server_config: Option<&str>, deploy_config: bool) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    if let Some(text) = server_config {
        std::fs::write(dir.path().join("config.toml"), text).expect("write config.toml");
    }
    if deploy_config {
        let pmcp_dir = dir.path().join(".pmcp");
        std::fs::create_dir_all(&pmcp_dir).expect("mkdir .pmcp");
        std::fs::write(pmcp_dir.join("deploy.toml"), DEPLOY_FIXTURE_HEADER)
            .expect("write deploy.toml");
    }
    dir
}

/// `cargo pmcp validate config --config <path>`.
fn validate_config_at(path: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-pmcp").expect("cargo-pmcp binary must be available");
    cmd.args(["validate", "config", "--config"]).arg(path);
    cmd
}

// -----------------------------------------------------------------------------
// Fixture shape 1 — an uncapped body-position string
// -----------------------------------------------------------------------------

/// The finding reaches STDERR (it is a warning) and the command still exits 0.
#[test]
fn uncapped_body_string_warns_on_stderr_and_exits_zero() {
    let dir = project(
        Some(&format!("{SERVER_HEADER}{UNCAPPED_BODY_STRING}")),
        false,
    );
    validate_config_at(&dir.path().join("config.toml"))
        .assert()
        .success()
        .stderr(contains("uncapped-string"))
        .stderr(contains("post-comment"))
        .stderr(contains("body_text"));
}

/// The SUMMARY reaches STDOUT — a reviewer reading only stdout still learns that
/// findings exist.
#[test]
fn uncapped_body_string_summary_is_on_stdout() {
    let dir = project(
        Some(&format!("{SERVER_HEADER}{UNCAPPED_BODY_STRING}")),
        false,
    );
    validate_config_at(&dir.path().join("config.toml"))
        .assert()
        .success()
        .stdout(contains("input-validation finding"));
}

/// An ACTIVE `[server.validation]` opt-out is reported, so a switched-off
/// enforcement can never read as switched on (T-128-31).
#[test]
fn active_opt_out_warns_on_stderr() {
    let toml_str = format!(
        "{SERVER_HEADER}\n[server.validation]\nenforce_input_schema = false\n{UNCAPPED_BODY_STRING}"
    );
    let dir = project(Some(&toml_str), false);
    validate_config_at(&dir.path().join("config.toml"))
        .assert()
        .success()
        .stderr(contains("opt-out-enforce-input-schema"));
}

// -----------------------------------------------------------------------------
// Fixture shape 2 — a non-compiling parameter pattern (HARD error)
// -----------------------------------------------------------------------------

#[test]
fn non_compiling_pattern_exits_non_zero_naming_the_parameter() {
    let dir = project(
        Some(&format!("{SERVER_HEADER}{NON_COMPILING_PATTERN}")),
        false,
    );
    validate_config_at(&dir.path().join("config.toml"))
        .assert()
        .failure()
        // Substance, not formatting: the error must name the tool, the offending
        // key, and the parameter. A bare `contains("id")` would pass on almost any
        // message, which is not an assertion.
        .stderr(contains("lookup"))
        .stderr(contains("pattern"))
        .stderr(contains("id"));
}

// -----------------------------------------------------------------------------
// Fixture shape 3 — zero tools
// -----------------------------------------------------------------------------

#[test]
fn zero_tools_config_reports_no_findings_on_stdout_and_exits_zero() {
    let dir = project(Some(SERVER_HEADER), false);
    validate_config_at(&dir.path().join("config.toml"))
        .assert()
        .success()
        .stdout(contains("no input-validation findings"));
}

// -----------------------------------------------------------------------------
// SC-3 — the toolkit-version banner (the operator's chosen resolution of the
// mixed-version case: print which toolkit linted, never hard-error on mismatch)
// -----------------------------------------------------------------------------

/// The banner is printed for a CLEAN config, which is the case that most needs it:
/// a bare `✓` would otherwise read as a guarantee about the deployed server.
///
/// Asserted on the message SHAPE, not on a version literal, so a toolkit bump
/// cannot make this test stale. `no input-validation findings` is co-asserted so a
/// banner printed on the wrong code path cannot satisfy it.
#[test]
fn clean_config_still_prints_the_toolkit_version_banner() {
    let dir = project(Some(SERVER_HEADER), false);
    validate_config_at(&dir.path().join("config.toml"))
        .assert()
        .success()
        .stdout(contains("no input-validation findings"))
        .stdout(contains("linted by pmcp-server-toolkit"))
        .stdout(contains(
            "not to whichever toolkit the deployed server runs",
        ));
}

/// And for a config WITH findings — the banner is unconditional, not a
/// clean-result-only decoration.
#[test]
fn config_with_findings_prints_the_toolkit_version_banner() {
    let dir = project(
        Some(&format!("{SERVER_HEADER}{UNCAPPED_BODY_STRING}")),
        false,
    );
    validate_config_at(&dir.path().join("config.toml"))
        .assert()
        .success()
        .stdout(contains("linted by pmcp-server-toolkit"))
        .stderr(contains("uncapped-string"));
}

/// `validate deploy`'s discovered-config path carries it too, for the same reason.
#[test]
fn validate_deploy_lint_findings_carry_the_toolkit_version_banner() {
    let dir = project(
        Some(&format!("{SERVER_HEADER}{UNCAPPED_BODY_STRING}")),
        true,
    );
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["validate", "deploy", "--server"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(contains("input-validation findings from config.toml"))
        .stdout(contains("linted by pmcp-server-toolkit"));
}

/// The banner must NOT appear when nothing was linted — a pure-IAM project stays
/// silent, which is the `DiscoveredConfigLint::Absent` contract. Without this
/// negative, "print it everywhere" would satisfy the three positives above.
#[test]
fn validate_deploy_prints_no_banner_when_there_is_no_toolkit_config() {
    let dir = project(None, true);
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["validate", "deploy", "--server"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(contains("IAM configuration valid"))
        .stdout(contains("linted by pmcp-server-toolkit").not());
}

// -----------------------------------------------------------------------------
// The `[backend]` parse regression — what `http` on the toolkit edge buys
// -----------------------------------------------------------------------------

#[test]
fn backend_section_parses_rather_than_failing_as_an_unknown_field() {
    let toml_str = format!("{SERVER_HEADER}{BACKEND_SECTION}{UNCAPPED_BODY_STRING}");
    let dir = project(Some(&toml_str), false);
    validate_config_at(&dir.path().join("config.toml"))
        .assert()
        .success()
        .stderr(contains("unknown field").not())
        .stderr(contains("uncapped-string"));
}

// -----------------------------------------------------------------------------
// Discovery — an explicit path, a `--server` directory, a missing file, and the
// process working directory (the branch a unit test may not touch)
// -----------------------------------------------------------------------------

#[test]
fn server_flag_discovers_the_config_at_the_directory_root() {
    let dir = project(
        Some(&format!("{SERVER_HEADER}{UNCAPPED_BODY_STRING}")),
        false,
    );
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["validate", "config", "--server"])
        .arg(dir.path())
        .assert()
        .success()
        .stderr(contains("uncapped-string"));
}

#[test]
fn working_directory_discovers_the_config_when_no_flag_is_given() {
    let dir = project(
        Some(&format!("{SERVER_HEADER}{UNCAPPED_BODY_STRING}")),
        false,
    );
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .current_dir(dir.path())
        .args(["validate", "config"])
        .assert()
        .success()
        .stderr(contains("uncapped-string"));
}

#[test]
fn missing_config_file_exits_non_zero_naming_the_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("nowhere").join("config.toml");
    validate_config_at(&missing)
        .assert()
        .failure()
        .stderr(contains(missing.display().to_string()));
}

// -----------------------------------------------------------------------------
// Fixture shapes 4 and 5 — `validate deploy` with, and without, a toolkit config
// -----------------------------------------------------------------------------

/// D-07's literal surface: BOTH documents present -> the IAM result AND the lint
/// findings, exit 0.
#[test]
fn validate_deploy_reports_lint_findings_when_both_documents_are_present() {
    let dir = project(
        Some(&format!("{SERVER_HEADER}{UNCAPPED_BODY_STRING}")),
        true,
    );
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["validate", "deploy", "--server"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(contains("IAM configuration valid"))
        .stdout(contains("input-validation findings from config.toml"))
        .stderr(contains("uncapped-string"));
}

/// Only a deploy document -> the IAM result and NOTHING about a toolkit config. A
/// pure-IAM project must not become noisy.
#[test]
fn validate_deploy_says_nothing_extra_without_a_toolkit_config() {
    let dir = project(None, true);
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["validate", "deploy", "--server"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(contains("IAM configuration valid"))
        .stdout(contains("config.toml").not())
        .stderr(contains("uncapped-string").not());
}

/// A DISCOVERED toolkit config that cannot be parsed is a warning, never an error:
/// `validate deploy`'s subject is the deploy document, and its documented guarantee
/// (a failing `validate deploy` guarantees a failing `deploy`) must not be widened.
#[test]
fn validate_deploy_warns_on_an_unreadable_toolkit_config_and_exits_zero() {
    let dir = project(Some("this is not = valid toml ["), true);
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["validate", "deploy", "--server"])
        .arg(dir.path())
        .assert()
        .success()
        .stderr(contains("config.toml"))
        .stderr(contains("could NOT be read"));
}
