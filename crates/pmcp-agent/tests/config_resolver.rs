//! AGNT-09: the `SlotResolver` seam (env-var + programmatic) resolves an
//! `AgentPackage`'s slots/endpoints, deviations warn-and-run (D-15), secrets are
//! never logged (ASVS V7), and `resolve_agent` composes a full
//! `ResolvedAgentConfig` end-to-end.
//!
//! Env-mutating tests set + restore a scoped guard and MUST run under
//! `--test-threads=1` (process env is global).

#![cfg(not(target_arch = "wasm32"))]

use pmcp_package::reference::{ComponentRef, ComponentType};
use pmcp_package::slot::{ConfigSlot, SlotType};
use pmcp_package::AgentPackage;

use pmcp_agent::config::{
    resolve_agent, EnvVarResolver, ProgrammaticBuilder, RedactedSecret, ResolveError,
    ResolvedValue, SlotResolver,
};

/// RAII guard that restores an env var to its prior value on drop.
struct EnvGuard {
    key: String,
    prior: Option<String>,
}

impl EnvGuard {
    fn set(key: &str, value: &str) -> Self {
        let prior = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self {
            key: key.to_string(),
            prior,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.prior {
            Some(v) => std::env::set_var(&self.key, v),
            None => std::env::remove_var(&self.key),
        }
    }
}

fn llm_slot(name: &str, tested: &str) -> ConfigSlot {
    ConfigSlot::new(SlotType::LlmProvider {
        name: name.to_string(),
        tested_value: tested.to_string(),
    })
}

fn secret_slot(name: &str) -> ConfigSlot {
    ConfigSlot::new(SlotType::Secret {
        name: name.to_string(),
    })
}

fn sample_agent() -> AgentPackage {
    AgentPackage {
        name: "triage-agent".to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
        instructions: "You triage incoming support tickets.".to_string(),
        llm: llm_slot("primary-llm", "anthropic"),
        max_tokens: 4096,
        max_iterations: 25,
        connectors: vec![ComponentRef::Range {
            name: "london-tube".to_string(),
            range: semver::VersionReq::parse("^1.2").unwrap(),
            component_type: ComponentType::Server,
        }],
        tool_selection: Some(serde_json::json!({ "london-tube": ["get_status", "get_line"] })),
        input_schema: Some(serde_json::json!({ "type": "object" })),
        output_schema: Some(serde_json::json!({ "type": "object" })),
        importance: Some("HIGH".to_string()),
        finalizer_role: None,
        budget_defaults: vec![],
    }
}

#[tokio::test]
async fn env_resolver_reads_behavior_value_from_env() {
    let _guard = EnvGuard::set("PRIMARY_LLM", "openai");
    let resolver = EnvVarResolver::new();

    let resolved = resolver
        .resolve_slot(&llm_slot("primary-llm", "anthropic"))
        .await
        .expect("resolves");
    match resolved {
        ResolvedValue::Plain(v) => assert_eq!(v, "openai"),
        ResolvedValue::Secret(_) => panic!("llm slot must resolve to a plain value"),
    }
}

#[tokio::test]
async fn env_resolver_missing_required_secret_is_typed_error() {
    // Ensure the env var is absent for this secret.
    std::env::remove_var("ABSENT_API_KEY");
    let resolver = EnvVarResolver::new();

    let err = resolver
        .resolve_slot(&secret_slot("ABSENT_API_KEY"))
        .await
        .expect_err("missing secret must error");
    assert!(matches!(err, ResolveError::MissingSlot(name) if name == "ABSENT_API_KEY"));
}

#[tokio::test]
async fn deviation_from_tested_value_warns_but_resolves_ok() {
    // Programmatic override to a different provider than tested ("anthropic").
    let resolver = ProgrammaticBuilder::new().with_value("primary-llm", "openai");

    // D-15: a deviation must NOT fail — resolution returns Ok with the new value.
    let resolved = resolver
        .resolve_slot(&llm_slot("primary-llm", "anthropic"))
        .await
        .expect("deviation warns and runs — never errors");
    match resolved {
        ResolvedValue::Plain(v) => assert_eq!(v, "openai"),
        ResolvedValue::Secret(_) => panic!("expected plain value"),
    }
}

#[tokio::test]
async fn programmatic_endpoint_map_builds_from_connectors() {
    let resolver =
        ProgrammaticBuilder::new().with_endpoint("london-tube", "https://tube.example/mcp");

    let endpoint = resolver
        .resolve_endpoint("london-tube")
        .await
        .expect("endpoint");
    assert_eq!(endpoint, "https://tube.example/mcp");

    let missing = resolver
        .resolve_endpoint("unknown")
        .await
        .expect_err("missing");
    assert!(matches!(missing, ResolveError::MissingEndpoint(n) if n == "unknown"));
}

#[tokio::test]
async fn secret_value_is_absent_from_debug_output() {
    let resolver = ProgrammaticBuilder::new().with_secret("LICHESS_API_KEY", "super-secret-token");

    let resolved = resolver
        .resolve_slot(&secret_slot("LICHESS_API_KEY"))
        .await
        .expect("resolves");
    let ResolvedValue::Secret(secret) = resolved else {
        panic!("secret slot must resolve to a redacted secret");
    };
    // The secret is readable only via expose(); Debug reveals nothing.
    assert_eq!(secret.expose(), "super-secret-token");
    let rendered = format!("{secret:?}");
    assert!(
        !rendered.contains("super-secret-token"),
        "Debug output must not contain the secret: {rendered}"
    );
    assert_eq!(rendered, "RedactedSecret(***)");
}

#[test]
fn redacted_secret_display_and_debug_never_leak() {
    let secret = RedactedSecret::new("top-secret");
    assert!(!format!("{secret:?}").contains("top-secret"));
}

#[tokio::test]
async fn resolve_agent_composes_full_config_end_to_end() {
    let pkg = sample_agent();
    let resolver = ProgrammaticBuilder::new()
        // Keep the tested provider so no deviation, exercising the happy path.
        .with_value("primary-llm", "anthropic")
        .with_endpoint("london-tube", "https://tube.example/mcp");

    let config = resolve_agent(&pkg, &resolver)
        .await
        .expect("resolve_agent composes a ResolvedAgentConfig");

    assert_eq!(config.instructions, "You triage incoming support tickets.");
    assert_eq!(config.model, "anthropic");
    assert_eq!(config.max_tokens, 4096);
    assert_eq!(config.max_iterations, 25);
    // Tool selection flattened in document order.
    assert_eq!(config.tools, vec!["get_status", "get_line"]);
    assert!(config.input_schema.is_some());
    assert!(config.output_schema.is_some());
    // The connector endpoint map is non-empty and carries the resolved endpoint.
    assert_eq!(
        config.endpoints.get("london-tube").map(String::as_str),
        Some("https://tube.example/mcp")
    );
}

#[tokio::test]
async fn resolve_agent_propagates_missing_endpoint() {
    let pkg = sample_agent();
    // No endpoint bound for the "london-tube" connector.
    let resolver = ProgrammaticBuilder::new().with_value("primary-llm", "anthropic");

    let err = resolve_agent(&pkg, &resolver)
        .await
        .expect_err("missing connector endpoint must error");
    assert!(matches!(err, ResolveError::MissingEndpoint(n) if n == "london-tube"));
}
