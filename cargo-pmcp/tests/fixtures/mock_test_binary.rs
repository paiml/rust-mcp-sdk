//! Phase 79 Wave 3 — deterministic mock binary for post-deploy verifier tests.
//!
//! This binary is invoked by the `post_deploy_orchestrator` integration test
//! via the `PMCP_TEST_FIXTURE_EXE` env-var injection point in
//! `cargo-pmcp/src/deployment/post_deploy_tests.rs::resolve_test_subprocess_exe`.
//!
//! Behaviour is controlled by environment variables (set by the test before
//! spawning the subprocess) so no Cargo features or args parsing is needed.
//!
//! Inputs (env):
//! - `MOCK_OUTCOME` — `"passed"` | `"test-failed"` | `"infra-error"` |
//!   `"malformed-json"` | `"empty"` | `"sleep-forever"` (default `"passed"`)
//! - `MOCK_COMMAND` — `"check"` | `"conformance"` | `"apps"` (default
//!   inferred from argv[1..] if recognisable, else `"check"`)
//! - `MOCK_SUMMARY_PASSED` — when set to a non-empty integer, populates the
//!   `summary.passed` field.
//! - `MOCK_SUMMARY_TOTAL` — pairs with `MOCK_SUMMARY_PASSED`.
//! - `MOCK_FAILURE_MESSAGE` — when set, populates `failures[0].message`.
//! - `MOCK_FAILURE_REPRODUCE` — when set, populates `failures[0].reproduce`.
//! - `MOCK_FAILURE_TOOL` — when set, populates `failures[0].tool`.
//! - `MOCK_EXIT_CODE` — explicit exit code override (otherwise derived from
//!   `MOCK_OUTCOME`: passed=0, test-failed=1, infra-error=2).
//! - `MOCK_ENV_DUMP_FILE` — when set, append every env var of the form
//!   `K=V\n` to the named file before exiting (used by env-inheritance test).
//! - `MOCK_ARGV_DUMP_FILE` — when set, append every argv element with `\n`
//!   delimiters to the named file before exiting (used by argv-shape test).
//!
//! Outputs:
//! - stdout: a single `PostDeployReport` JSON document (or malformed text /
//!   empty per `MOCK_OUTCOME`).
//! - exit code: per `MOCK_EXIT_CODE` or `MOCK_OUTCOME`.
//!
//! Test-only. Its `[[bin]]` entry in `cargo-pmcp/Cargo.toml` carries
//! `required-features = ["test-fixtures"]`, so a default build — and therefore
//! `cargo install cargo-pmcp` — skips it (#412: before 0.28.1 it was installed
//! into `~/.cargo/bin`). The `post_deploy_orchestrator` test, which resolves it
//! via `env!("CARGO_BIN_EXE_mock_test_binary")`, needs the same feature:
//! `cargo test -p cargo-pmcp --features test-fixtures --test post_deploy_orchestrator`.

use std::io::Write;

fn main() {
    // Optional dumps for env / argv shape tests.
    if let Ok(path) = std::env::var("MOCK_ENV_DUMP_FILE") {
        if let Ok(mut f) = std::fs::File::create(&path) {
            for (k, v) in std::env::vars() {
                let _ = writeln!(f, "{k}={v}");
            }
        }
    }
    if let Ok(path) = std::env::var("MOCK_ARGV_DUMP_FILE") {
        if let Ok(mut f) = std::fs::File::create(&path) {
            for arg in std::env::args() {
                let _ = writeln!(f, "{arg}");
            }
        }
    }

    let outcome = std::env::var("MOCK_OUTCOME").unwrap_or_else(|_| "passed".to_string());

    if outcome == "sleep-forever" {
        // Used by timeout test. Sleep ~30s — caller's timeout is much shorter.
        std::thread::sleep(std::time::Duration::from_secs(30));
        return;
    }
    if outcome == "malformed-json" {
        print!("{{not valid json");
        std::process::exit(1);
    }
    if outcome == "empty" {
        // Print nothing.
        std::process::exit(1);
    }

    let command = derive_command();
    let json = build_report_json(&outcome, &command);
    print!("{json}");

    let exit_code: i32 = std::env::var("MOCK_EXIT_CODE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(match outcome.as_str() {
            "passed" => 0,
            "test-failed" => 1,
            "infra-error" => 2,
            _ => 0,
        });
    std::process::exit(exit_code);
}

/// Derive the report's `command` field. Prefers `MOCK_COMMAND` env var; falls
/// back to argv pattern matching.
fn derive_command() -> String {
    if let Ok(v) = std::env::var("MOCK_COMMAND") {
        if !v.is_empty() {
            return v;
        }
    }
    let args: Vec<String> = std::env::args().collect();
    // argv shape: <exe> test {check|conformance|apps} <URL> ...
    if args.len() >= 3 && args[1] == "test" {
        match args[2].as_str() {
            "check" | "conformance" | "apps" => return args[2].clone(),
            _ => {},
        }
    }
    "check".to_string()
}

/// Construct a `PostDeployReport`-shaped JSON document. Hand-rolled (no serde
/// dep) to keep this binary's compile-time cheap and dep-free.
fn build_report_json(outcome: &str, command: &str) -> String {
    let url_arg = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "http://mock".to_string());

    let summary_part = build_summary_part();
    let failures_part = build_failures_part();
    let mode_part = if command == "apps" {
        let mode_arg = extract_mode_arg().unwrap_or_else(|| "claude-desktop".to_string());
        format!(",\n  \"mode\": \"{mode_arg}\"")
    } else {
        String::new()
    };

    format!(
        "{{\n  \"command\": \"{command}\",\n  \"url\": \"{url_arg}\"{mode_part},\n  \
         \"outcome\": \"{outcome}\"{summary_part}{failures_part},\n  \
         \"duration_ms\": 200,\n  \"schema_version\": \"1\"\n}}"
    )
}

fn build_summary_part() -> String {
    let passed = std::env::var("MOCK_SUMMARY_PASSED")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let total = std::env::var("MOCK_SUMMARY_TOTAL")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    match (passed, total) {
        (Some(p), Some(t)) => {
            let failed = t.saturating_sub(p);
            format!(
                ",\n  \"summary\": {{\"total\": {t}, \"passed\": {p}, \"failed\": {failed}, \
                 \"warnings\": 0, \"skipped\": 0}}"
            )
        },
        _ => String::new(),
    }
}

fn build_failures_part() -> String {
    let msg = std::env::var("MOCK_FAILURE_MESSAGE").ok();
    let reproduce = std::env::var("MOCK_FAILURE_REPRODUCE").ok();
    let tool = std::env::var("MOCK_FAILURE_TOOL").ok();
    if msg.is_none() && reproduce.is_none() {
        return ",\n  \"failures\": []".to_string();
    }
    let msg = msg.unwrap_or_default();
    let reproduce = reproduce.unwrap_or_default();
    let tool_field = match tool {
        Some(t) if !t.is_empty() => format!("\"tool\": \"{}\", ", json_escape(&t)),
        _ => String::new(),
    };
    format!(
        ",\n  \"failures\": [{{{tool_field}\"message\": \"{}\", \"reproduce\": \"{}\"}}]",
        json_escape(&msg),
        json_escape(&reproduce)
    )
}

fn extract_mode_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let pos = args.iter().position(|a| a == "--mode")?;
    args.get(pos + 1).cloned()
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
