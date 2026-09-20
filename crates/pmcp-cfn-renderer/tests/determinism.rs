use pmcp_cfn_renderer::{render, ArtifactRef, RenderParams};

fn minimal_descriptor() -> pmcp_package::package::DeployDescriptor {
    // Smallest shape that satisfies `DeployDescriptor`'s `deny_unknown_fields`
    // + required-field set: [target] + [aws] + [server] (name,
    // timeout_seconds) + [auth] (enabled, provider) + [observability]
    // (log_retention_days, enable_xray, create_dashboard). Every other table
    // (`[metadata]`, `[composition]`, `[assets]`, `[iam]`, `[gcp]`,
    // `[layout]`, `[observability.alarms]`, `[auth.dcr]`, `[auth.groups]`,
    // `[auth.scopes]`) is `Option`/defaulted and can be omitted entirely.
    //
    // NOTE: the brief's original draft fixture put `error_threshold` directly
    // under `[observability]` and omitted `log_retention_days` /
    // `enable_xray` / `create_dashboard` — those three are REQUIRED fields on
    // `ObservabilitySection`, and `error_threshold` only exists on the
    // optional nested `[observability.alarms]` sub-table, so the original
    // draft fails to parse under `deny_unknown_fields`. This fixture matches
    // the real tracked `crates/pmcp-server/.pmcp/deploy.toml` shape, trimmed
    // to only the required fields.
    //
    // `[target].type = "pmcp-run"` (not the Task 1 draft's "aws-lambda"):
    // Task 3's `render` guards non-"pmcp-run" targets with
    // `RenderError::UnsupportedSection` (that stack shape needs the
    // not-yet-implemented `http_api` module) — this determinism test needs
    // `render` to actually succeed, so it uses the one target shape that
    // renders today.
    toml::from_str(
        r#"
        [target]
        type = "pmcp-run"
        version = "1.0.0"
        [aws]
        region = "us-east-1"
        [server]
        name = "det-test"
        timeout_seconds = 30
        [auth]
        enabled = false
        provider = "none"
        [observability]
        log_retention_days = 30
        enable_xray = false
        create_dashboard = false
        "#,
    )
    .expect("minimal descriptor parses")
}

fn params() -> RenderParams {
    RenderParams {
        artifact: ArtifactRef {
            s3_bucket: "pmcp-deploy-123456789012-us-east-1".into(),
            s3_key: "det-test/bootstrap.zip".into(),
            digest: None,
        },
        ..RenderParams::for_test("det-test-stack")
    }
}

#[test]
fn render_is_byte_deterministic() {
    let d = minimal_descriptor();
    let p = params();
    let a = render(&d, &p).unwrap().to_canonical_json();
    let b = render(&d, &p).unwrap().to_canonical_json();
    assert_eq!(a, b);
    // canonical: sorted keys — serde_json::Value round-trip of the string
    // must serialize back identically
    let v: serde_json::Value = serde_json::from_str(&a).unwrap();
    assert_eq!(serde_json::to_string_pretty(&v).unwrap(), a);
}
