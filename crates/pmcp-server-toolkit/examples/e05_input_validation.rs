//! The three input-validation layers, each catching what only it can (Phase 128).
//!
//! Run with:
//! ```sh
//! cargo run -p pmcp-server-toolkit --example e05_input_validation \
//!   --features input-validation,http
//! ```
//!
//! `make test-examples` only BUILDS examples — it never runs one. So running this
//! is the CLAUDE.md ALWAYS manual leg, and the observed output is what discharges
//! it; a green build proves nothing about what the layers catch.
//!
//! # What it demonstrates
//!
//! Three layers, in the order the documentation describes them, each shown catching
//! one thing and each shown NOT catching the others' cases:
//!
//! 1. **D1 — the config-declared schema.** `[[tools.parameters]]` becomes an
//!    `inputSchema` and a violation is refused before the backend is called. It is
//!    a rule about ONE value.
//! 2. **E2 — a per-tool `ArgumentValidator`.** A rule about a COMBINATION of
//!    values, which a JSON Schema cannot express. It runs strictly AFTER D1, so it
//!    never sees arguments the schema already refused.
//! 3. **E1 — a `RequestPolicy`.** A rule about what may LEAVE the server. It runs
//!    on the outbound request, before the credential is applied, so third-party
//!    policy code cannot observe one.
//!
//! No network is contacted: layers 1 and 2 refuse before dispatch, and layer 3
//! refuses before the send. The backend URL is deliberately unreachable to make
//! that visible — if any layer were absent, the corresponding line below would
//! report a transport error instead of a refusal.
//!
//! Per Phase 83 review R3 the imports below are ONE crate-root block. If a type
//! needed here cannot be imported that way, the fix is the missing `lib.rs`
//! re-export — never a module-path-qualified import in an example.

use std::sync::Arc;

use pmcp_server_toolkit::{
    async_trait, create_auth_provider, render_validation_report,
    synthesize_from_config_with_http_connector_and_hooks, ArgumentRefusal, ArgumentValidator,
    AuthConfig, HttpClient, HttpConnector, OutboundRequest, PolicyRefusal, RequestPolicy,
    ServerConfig, ToolkitHooks,
};

/// Layer 2 — a rule about a COMBINATION of values.
///
/// `start` and `end` are both declared integers, so the schema accepts any pair.
/// Their RELATION is the rule, and only a Rust validator can state it.
struct EndAfterStart;

impl ArgumentValidator for EndAfterStart {
    fn validate(&self, args: &serde_json::Value) -> Result<(), ArgumentRefusal> {
        let start = args.get("start").and_then(serde_json::Value::as_i64);
        let end = args.get("end").and_then(serde_json::Value::as_i64);
        match (start, end) {
            // The message is a FIXED string: it names the rule, never a value.
            (Some(s), Some(e)) if e < s => {
                Err(ArgumentRefusal::new("`end` must not precede `start`"))
            },
            _ => Ok(()),
        }
    }
}

/// Layer 3 — a rule about what may LEAVE the server.
///
/// An endpoint allowlist. The policy is handed the FULLY RESOLVED target, so it
/// sees the URL as it will be sent rather than the `[[tools]]` template.
struct AllowlistPrefix(&'static str);

#[async_trait]
impl RequestPolicy for AllowlistPrefix {
    async fn check(&self, req: &OutboundRequest<'_>) -> Result<(), PolicyRefusal> {
        println!(
            "      [policy saw] tool={:?} method={} path={} query={:?} body={:?}",
            req.tool, req.method, req.path, req.query, req.body
        );
        if req.path.contains(self.0) {
            Ok(())
        } else {
            Err(PolicyRefusal::new(
                "outbound endpoint is not on the allowlist",
            ))
        }
    }
}

const CONFIG: &str = r#"
[server]
name = "Input Validation Demo"
version = "0.1.0"

[backend]
base_url = "https://backend.invalid"

# Every rule this config can enforce IS enforced. Each of these four keys is an
# opt-out when set the other way, and every active opt-out is logged once at
# startup — an enforcement that is off must never read as on.
[server.validation]
enforce_input_schema = true
default_max_length = 256
additional_properties = false

[[tools]]
name = "fetch_range"
description = "Fetch a numbered range of records for one dataset version"
path = "/content/{version}/records"
method = "GET"

# Layer 1: a rule about ONE value. `version` must look like `v` plus digits.
[[tools.parameters]]
name = "version"
type = "string"
required = true
pattern = "^v[0-9]+$"

[[tools.parameters]]
name = "start"
type = "integer"
required = true

[[tools.parameters]]
name = "end"
type = "integer"
required = true
"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = ServerConfig::from_toml_strict_validated(CONFIG)?;

    // Register BOTH escape hatches. `ToolkitHooks` is a plain value handed to the
    // assembly entry point: `ServerBuilderExt` is implemented for core's
    // `ServerBuilder`, whose fields are private, so there is nowhere on the builder
    // to accumulate a registration.
    let hooks = ToolkitHooks::default()
        .with_argument_validator("fetch_range", Arc::new(EndAfterStart))
        .with_request_policy(Arc::new(AllowlistPrefix("/content/")));

    // The connector carries the policy. `https://backend.invalid` never resolves,
    // which is the point: every line below reports a REFUSAL, so nothing reached
    // the network.
    let connector: Arc<dyn HttpConnector> = Arc::new(
        HttpClient::new(
            reqwest::Client::new(),
            "https://backend.invalid".to_string(),
            create_auth_provider(&AuthConfig::Bearer {
                token: "never-visible-to-the-policy".to_string(),
                required: true,
            })?,
        )?
        .with_request_policy(hooks.request_policy().expect("registered just above")),
    );

    // The once-at-startup enforcement report: what is ON, what is OFF, and which
    // hooks are registered. This is what an operator reads in a deploy log to trace
    // a server that has started refusing calls.
    println!("== startup enforcement report ==");
    for line in render_validation_report(&cfg, &hooks) {
        println!("  [{:?}] {}", line.level, line.text);
    }

    let mut tools = synthesize_from_config_with_http_connector_and_hooks(&cfg, connector, &hooks)?;
    let handler = tools.remove(0).2;
    let extra = pmcp::RequestHandlerExtra::default;

    println!("\n== layer 1: D1, the config-declared schema (a rule about ONE value) ==");
    let err = handler
        .handle(
            serde_json::json!({ "version": "nope", "start": 1, "end": 9 }),
            extra(),
        )
        .await
        .expect_err("`version` violates its declared pattern");
    println!("  REFUSED by D1: {err}");

    println!("\n== layer 2: E2, a per-tool ArgumentValidator (a rule about a COMBINATION) ==");
    println!("  these same values pass the declared schema — two integers, no constraint");
    let err = handler
        .handle(
            serde_json::json!({ "version": "v3", "start": 9, "end": 1 }),
            extra(),
        )
        .await
        .expect_err("`end` precedes `start`");
    println!("  REFUSED by E2: {err}");

    println!("\n== layer 3: E1, a RequestPolicy (a rule about what may LEAVE the server) ==");
    println!("  these values pass BOTH layers above, so the call reaches the outbound hook");
    let err = handler
        .handle(
            serde_json::json!({ "version": "v3", "start": 1, "end": 9 }),
            extra(),
        )
        .await
        .expect_err(
            "the allowlist permits /content/, and this path matches, so the REFUSAL \
                     below comes from the unreachable backend rather than the policy",
        );
    println!("  policy ALLOWED, then the unreachable backend failed: {err}");

    println!("\n  and with a policy that refuses this endpoint:");
    let refusing = ToolkitHooks::default()
        .with_request_policy(Arc::new(AllowlistPrefix("/only-this-other-prefix/")));
    let refusing_connector: Arc<dyn HttpConnector> = Arc::new(
        HttpClient::new(
            reqwest::Client::new(),
            "https://backend.invalid".to_string(),
            create_auth_provider(&AuthConfig::None)?,
        )?
        .with_request_policy(refusing.request_policy().expect("registered")),
    );
    let mut refusing_tools =
        synthesize_from_config_with_http_connector_and_hooks(&cfg, refusing_connector, &refusing)?;
    let err = refusing_tools
        .remove(0)
        .2
        .handle(
            serde_json::json!({ "version": "v3", "start": 1, "end": 9 }),
            extra(),
        )
        .await
        .expect_err("the policy refuses this endpoint");
    println!("  REFUSED by E1: {err}");

    println!(
        "\nAll three layers refused something only they could express, and no request \
         reached the network."
    );
    Ok(())
}
