//! Single-crate agent-package scaffold emitter (CLI-01, Phase 110-02).
//!
//! Mirrors [`crate::templates::workbook_server`]: one `generate` orchestrator
//! that calls one private `generate_<file>` fn per output file, each a single
//! raw `fs::write(dir.join("X"), <content>).context(...)`. There is NO template
//! engine — text emission is raw string literals (`format!` escapes literal
//! braces as `{{`/`}}`).
//!
//! It emits a runnable *agent* crate into a SINGLE directory:
//! - `Cargo.toml` — pins `pmcp-agent` (with `openai-compat`) plus the FULL
//!   dependency set the manifest-driven runner and `tests/pin.rs` compile
//!   against, and a `[dev-dependencies] toml` for the in-scaffold pin tripwire.
//! - `src/main.rs` — a runner that LOADS `agent.package.json` (the manifest is
//!   the source of truth — Codex 110-02 MEDIUM), resolves it via
//!   [`pmcp_agent::resolve_agent`] + [`pmcp_agent::EnvVarResolver`] into a
//!   [`pmcp_agent::ResolvedAgentConfig`], and drives [`pmcp_agent::AgentEngine`]
//!   against an OpenAI-compatible endpoint (Ollama by default).
//! - `agent.package.json` — an [`pmcp_package::AgentPackage`] serialized to
//!   pretty JSON. It is BUILT from the real struct here (not hand-written text),
//!   so it is guaranteed to round-trip through the schema the runner loads.
//! - `tests/pin.rs` — an in-scaffold version-pin tripwire (D-05): parses the
//!   scaffold's OWN `Cargo.toml` and asserts the `pmcp-agent` dep stays on the
//!   generated major.minor line, DERIVED from `PMCP_AGENT_VERSION`.
//!
//! Pin tripwires (D-05, Research Open-Q1 → ship every level):
//! 1. INTERNAL drift-guard — [`PMCP_AGENT_VERSION`] must equal the workspace
//!    `crates/pmcp-agent` `[package] version` (unit test below), so the emitted
//!    pin cannot silently drift from the released crate.
//! 2. IN-SCAFFOLD tripwire — the emitted `tests/pin.rs` asserts the generated
//!    project's own pin against its major.minor line.
//! 3. INTERNAL drift-guard for [`PMCP_PACKAGE_VERSION_REQ`] against the
//!    workspace `crates/pmcp-package` major.minor line (Phase 120). Level 1
//!    existed for `pmcp-agent` but nothing covered `pmcp-package`, so the
//!    emitted requirement sat on the superseded line through a version bump —
//!    invisible to `cargo build`, since the workspace resolves via its own
//!    manifests and never compiles what this template writes.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

use pmcp_package::{AgentPackage, ConfigSlot, SlotType};

/// The pinned `pmcp-agent` version the emitted `Cargo.toml` declares. A test
/// asserts this equals the workspace `crates/pmcp-agent` package version so the
/// hardcoded pin cannot silently drift from the released crate (D-05) —
/// mirroring the `workbook_server::PMCP_VERSION` drift guard.
const PMCP_AGENT_VERSION: &str = "0.4.0";

/// The `pmcp-package` version REQUIREMENT the emitted `Cargo.toml` declares —
/// a caret major.minor line, not a full version.
///
/// This is a THIRD version emitter, invisible to `cargo build`: the workspace
/// resolves green while every project scaffolded by `cargo pmcp agent new`
/// requests whatever this string says. Phase 120 found it still on `"0.1"`
/// after `pmcp-package` had gone 0.2.0, which would have shipped a scaffold
/// that fails to compile against the published crate. Naming it here means the
/// emitter below and its assertion test read the SAME constant, so the next
/// bump is one edit rather than two that can drift apart.
///
/// Phase 122 moved it to `"0.3"` alongside `pmcp-package`'s own 0.2.0 -> 0.3.0
/// bump, and measured the asymmetry that makes this constant the dangerous one:
/// reverting ONLY this line leaves `cargo build --workspace` green while
/// `emitted_package_requirement_matches_workspace_major_minor_line` goes red.
/// The compiler is not this emitter's tripwire; that test is.
const PMCP_PACKAGE_VERSION_REQ: &str = "0.4";

/// Emit the files of a single runnable agent crate into `dir`.
pub fn generate(dir: &Path, name: &str) -> Result<()> {
    generate_cargo_toml(dir, name)?;
    generate_main_rs(dir)?;
    generate_manifest(dir, name)?;
    generate_pin_test(dir)?;

    if std::env::var("PMCP_QUIET").is_err() {
        println!("  {} Generated agent package files", "✓".green());
    }
    Ok(())
}

fn generate_cargo_toml(dir: &Path, name: &str) -> Result<()> {
    // Enumerate the COMPLETE dependency set the runner + pin test compile against
    // (Codex 110-02 MEDIUM): pmcp-agent (with `openai-compat` for
    // OpenAiCompatSource), pmcp-package (the manifest type), tokio (the async
    // runtime), serde_json (manifest deserialize), anyhow (runner error flow),
    // and async-trait (the runner's minimal ToolInvoker impl). `[dev-dependencies]
    // toml` backs `tests/pin.rs`. Escape literal braces as `{{`/`}}`.
    let content = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
pmcp-agent = {{ version = "{PMCP_AGENT_VERSION}", features = ["openai-compat"] }}
pmcp-package = "{PMCP_PACKAGE_VERSION_REQ}"
tokio = {{ version = "1", features = ["full"] }}
serde_json = "1"
anyhow = "1"
async-trait = "0.1"

[dev-dependencies]
toml = "0.8"
"#,
    );

    fs::write(dir.join("Cargo.toml"), content).context("Failed to create Cargo.toml")?;
    Ok(())
}

/// The emitted `src/main.rs`: a manifest-driven runner. It LOADS
/// `agent.package.json` (source of truth), resolves it through the env-var slot
/// convention, and drives the `AgentEngine` against an OpenAI-compatible endpoint.
fn emitted_main_rs() -> &'static str {
    r#"//! Standalone agent runner generated by `cargo pmcp agent new`.
//!
//! The manifest (`agent.package.json`) is the source of truth: this runner LOADS
//! it, resolves its config slots from conventionally-named environment variables
//! (the `llm` slot falls back to its tested value when unset), and drives the
//! `AgentEngine` against an OpenAI-compatible endpoint (a local Ollama by
//! default). Point it at your model by editing the manifest / endpoint below and
//! running an OpenAI-compatible server.

use anyhow::{Context, Result};
use async_trait::async_trait;
use pmcp_agent::sources::{OpenAiCompatSource, SecretString};
use pmcp_agent::{
    resolve_agent, AgentEngine, EnvVarResolver, InMemoryStore, ToolCall, ToolCallResult,
    ToolInvoker,
};
use pmcp_package::AgentPackage;

/// A minimal invoker: this starter agent ships with no connectors, so any tool
/// call the model attempts returns an error the loop surfaces. Wire real
/// connectors by adding them to `agent.package.json` and swapping this for
/// `pmcp_agent::ClientToolInvoker`.
struct NoToolsInvoker;

#[async_trait]
impl ToolInvoker for NoToolsInvoker {
    async fn invoke(&self, call: ToolCall) -> ToolCallResult {
        ToolCallResult::error(
            call.id,
            format!("no connectors configured for tool `{}`", call.name),
        )
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // The manifest is the source of truth: load it, don't hardcode the config.
    let manifest = std::fs::read_to_string("agent.package.json")
        .context("read agent.package.json from the crate root")?;
    let pkg: AgentPackage =
        serde_json::from_str(&manifest).context("parse agent.package.json as an AgentPackage")?;

    // Resolve slots from conventionally-named env vars (the llm slot falls back
    // to its tested value when unset), producing a runnable config.
    let config = resolve_agent(&pkg, &EnvVarResolver::new())
        .await
        .context("resolve the agent package into a runnable config")?;

    // Drive the loop against an OpenAI-compatible endpoint (Ollama by default).
    let model = config.model.clone();
    let source = OpenAiCompatSource::new(
        "http://localhost:11434/v1",
        model,
        SecretString::new("ollama"),
    )
    .context("construct the OpenAI-compatible completion source")?;

    let engine = AgentEngine::new(source, NoToolsInvoker, InMemoryStore::new(), config);
    let outcome = engine.run("agent-run").await;
    println!("agent run outcome: {outcome:?}");
    Ok(())
}
"#
}

fn generate_main_rs(dir: &Path) -> Result<()> {
    fs::write(dir.join("src").join("main.rs"), emitted_main_rs())
        .context("Failed to create src/main.rs")?;
    Ok(())
}

/// Build the starter [`AgentPackage`] for the scaffold. Built from the real
/// struct (not hand-written JSON) so the emitted manifest is guaranteed to
/// round-trip through the schema the runner loads (Codex 110-02 MEDIUM). Also the
/// single source for `agent dev`'s built-in demo package.
pub(crate) fn starter_package(name: &str) -> AgentPackage {
    AgentPackage {
        name: name.to_string(),
        version: semver::Version::new(1, 0, 0),
        instructions: "You are a concise, helpful assistant. Use tools when helpful.".to_string(),
        llm: ConfigSlot::new(SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "llama3.2".to_string(),
        }),
        max_tokens: 100_000,
        max_iterations: 5,
        connectors: vec![],
        tool_selection: None,
        input_schema: None,
        output_schema: None,
        importance: None,
        finalizer_role: None,
        budget_defaults: vec![],
    }
}

fn generate_manifest(dir: &Path, name: &str) -> Result<()> {
    let pkg = starter_package(name);
    let json =
        serde_json::to_string_pretty(&pkg).context("serialize the starter AgentPackage to JSON")?;
    fs::write(dir.join("agent.package.json"), format!("{json}\n"))
        .context("Failed to create agent.package.json")?;
    Ok(())
}

/// The major.minor line of [`PMCP_AGENT_VERSION`], e.g. `0.2` for `0.2.0`.
///
/// DERIVED, never restated. Bumping `PMCP_AGENT_VERSION` from `0.1.0` to `0.2.0`
/// (plan 117-07) left the emitted tripwire asserting `0.1`, which would have
/// shipped a scaffold whose own `cargo test` failed on first run — the generator
/// unit tests never execute the emitted test, so nothing in this repo would have
/// caught it. Deriving removes the second copy that made that possible.
fn agent_pin_major_minor() -> String {
    let mut parts = PMCP_AGENT_VERSION.split('.');
    let major = parts.next().unwrap_or("0");
    let minor = parts.next().unwrap_or("0");
    format!("{major}.{minor}")
}

/// The emitted `tests/pin.rs`: an in-scaffold version-pin tripwire (D-05). It
/// parses the scaffold's OWN `Cargo.toml` (via the `toml` dev-dep) and asserts
/// the `pmcp-agent` dependency stays on the generated major.minor line.
fn emitted_pin_test() -> String {
    // The emitted file's own `{version}` / `{EXPECTED_LINE}` are real format args
    // in the SCAFFOLD's assert!, so they are escaped `{{` / `}}` here; only
    // `{line}` is interpolated by this template.
    let line = agent_pin_major_minor();
    format!(
        r#"//! Version-pin tripwire (D-05): the scaffolded `pmcp-agent` dependency must
//! stay pinned to the major.minor line this project was generated against. If a
//! future `cargo pmcp agent new` bumps the pin, update the expectation here
//! deliberately — a silent drift is a defect.

/// The major.minor line this project was generated against.
const EXPECTED_LINE: &str = "{line}";

#[test]
fn pmcp_agent_pin_matches_generated_major_minor() {{
    let manifest = include_str!("../Cargo.toml");
    let parsed: toml::Value = toml::from_str(manifest).expect("parse Cargo.toml");
    let dep = parsed
        .get("dependencies")
        .and_then(|d| d.get("pmcp-agent"))
        .expect("Cargo.toml has [dependencies] pmcp-agent");
    // The dependency is a table: {{ version = "0.1.0", features = [...] }}.
    let version = dep
        .get("version")
        .and_then(toml::Value::as_str)
        .expect("pmcp-agent dependency declares a version string");
    assert!(
        version.starts_with(EXPECTED_LINE),
        "scaffolded pmcp-agent pin `{{version}}` drifted from the generated `{{EXPECTED_LINE}}` line"
    );
}}
"#
    )
}

fn generate_pin_test(dir: &Path) -> Result<()> {
    let tests_dir = dir.join("tests");
    fs::create_dir_all(&tests_dir).context("Failed to create tests directory")?;
    fs::write(tests_dir.join("pin.rs"), emitted_pin_test())
        .context("Failed to create tests/pin.rs")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The workspace `crates/pmcp-agent/Cargo.toml` (for the version-drift guard).
    const AGENT_CARGO_TOML: &str = include_str!("../../../crates/pmcp-agent/Cargo.toml");

    /// The workspace `crates/pmcp-package/Cargo.toml` (for the version-drift
    /// guard on the scaffold's `pmcp-package` requirement).
    const PACKAGE_CARGO_TOML: &str = include_str!("../../../crates/pmcp-package/Cargo.toml");

    /// Read a crate's `[package] version` out of its manifest text.
    fn package_version(manifest: &str, crate_name: &str) -> String {
        let parsed: toml::Value = toml::from_str(manifest)
            .unwrap_or_else(|e| panic!("parse {crate_name} Cargo.toml: {e}"));
        parsed
            .get("package")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("{crate_name} Cargo.toml has [package] version"))
            .to_string()
    }

    #[test]
    fn emitted_package_requirement_matches_workspace_major_minor_line() {
        // Phase 120: the scaffold's `pmcp-package` requirement is a THIRD
        // version emitter that `cargo build` cannot see — the workspace
        // resolves green while every scaffolded project requests whatever this
        // constant says. It sat on "0.1" after pmcp-package went 0.2.0, which
        // would have shipped a scaffold that fails to compile against the
        // published crate. This guard is what makes that a red test rather
        // than a discovery four phases later.
        let version = package_version(PACKAGE_CARGO_TOML, "pmcp-package");
        let mut parts = version.split('.');
        let major = parts.next().unwrap_or("0");
        let minor = parts.next().unwrap_or("0");
        let expected = format!("{major}.{minor}");
        assert_eq!(
            PMCP_PACKAGE_VERSION_REQ, expected,
            "the scaffold's pmcp-package requirement `{PMCP_PACKAGE_VERSION_REQ}` drifted from \
             the workspace crate version `{version}` — bump PMCP_PACKAGE_VERSION_REQ in \
             templates/agent.rs"
        );
    }

    #[test]
    fn emitted_agent_version_matches_workspace_pin() {
        // D-05: the hardcoded PMCP_AGENT_VERSION must not drift from the workspace
        // `pmcp-agent` package version. Parse its Cargo.toml `[package] version`
        // and compare — mirroring the workbook_server PMCP_VERSION drift guard.
        let agent_version = package_version(AGENT_CARGO_TOML, "pmcp-agent");
        assert_eq!(
            PMCP_AGENT_VERSION, agent_version,
            "the scaffold's hardcoded pmcp-agent pin `{PMCP_AGENT_VERSION}` drifted from the \
             workspace pin `{agent_version}` — bump PMCP_AGENT_VERSION in templates/agent.rs"
        );
    }

    #[test]
    fn emitted_manifest_round_trips_as_agent_package() {
        // The emitted agent.package.json MUST deserialize back as an AgentPackage
        // (the runner LOADS it, so it cannot diverge from the schema). Render into
        // a scratch dir and read it back so the assertion exercises the REAL
        // emitter, not a copy of the literal.
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("src")).expect("src dir");
        generate_manifest(tmp.path(), "agent_round_trip_demo").expect("emit manifest");

        let json = std::fs::read_to_string(tmp.path().join("agent.package.json"))
            .expect("read emitted manifest");
        let pkg: AgentPackage =
            serde_json::from_str(&json).expect("emitted manifest deserializes as AgentPackage");
        assert_eq!(pkg.name, "agent_round_trip_demo");
        assert_eq!(pkg.llm.slot.tested_value(), Some("llama3.2"));
    }

    #[test]
    fn generate_emits_full_file_tree() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("src")).expect("src dir");
        generate(tmp.path(), "agent_tree_demo").expect("generate scaffold");

        for rel in [
            "Cargo.toml",
            "src/main.rs",
            "agent.package.json",
            "tests/pin.rs",
        ] {
            assert!(
                tmp.path().join(rel).is_file(),
                "scaffold did not emit expected file: {rel}"
            );
        }
    }

    #[test]
    fn emitted_cargo_toml_has_full_dependency_set() {
        let tmp = tempfile::tempdir().expect("tempdir");
        generate_cargo_toml(tmp.path(), "agent_deps_demo").expect("emit Cargo.toml");
        let cargo =
            std::fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read Cargo.toml");

        // Built from PMCP_AGENT_VERSION rather than restating it: a literal here
        // is a THIRD copy of the pin, and it made the D-05 drift-guard's failure
        // land twice — once as the guard firing (the useful signal) and once as
        // this test failing for a reason that has nothing to do with the
        // dependency SET it is named for. Only the guard should own the version.
        let agent_dep = format!(
            r#"pmcp-agent = {{ version = "{PMCP_AGENT_VERSION}", features = ["openai-compat"] }}"#
        );
        // Built from PMCP_PACKAGE_VERSION_REQ for the same reason: a literal
        // here is a second copy of the pin that can silently drift from the
        // emitter it is meant to be checking.
        let package_dep = format!(r#"pmcp-package = "{PMCP_PACKAGE_VERSION_REQ}""#);

        for token in [
            agent_dep.as_str(),
            package_dep.as_str(),
            r#"tokio = { version = "1", features = ["full"] }"#,
            "serde_json =",
            "anyhow =",
            "async-trait =",
            "[dev-dependencies]",
            "toml =",
        ] {
            assert!(
                cargo.contains(token),
                "emitted Cargo.toml missing dependency token: {token}\n{cargo}"
            );
        }
    }

    #[test]
    fn emitted_main_is_manifest_driven() {
        // The runner must LOAD the manifest and RESOLVE it (not hardcode config).
        let m = emitted_main_rs();
        for token in [
            "read_to_string(\"agent.package.json\")",
            "resolve_agent(",
            "EnvVarResolver::new()",
            "OpenAiCompatSource::new(",
            "AgentEngine::new(",
            "engine.run(",
        ] {
            assert!(
                m.contains(token),
                "emitted main.rs missing wiring token: {token}"
            );
        }
    }
}
